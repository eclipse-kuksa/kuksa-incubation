/********************************************************************************
* Copyright (c) 2026 Contributors to the Eclipse Foundation
*
* This program and the accompanying materials are made available under the
* terms of the Apache License 2.0 which is available at
* http://www.apache.org/licenses/LICENSE-2.0
*
* SPDX-License-Identifier: Apache-2.0
********************************************************************************/

//! ACF-VSS frame listener — binds a UDP or raw AF_PACKET socket, receives
//! Ethernet frames carrying NTSCF/TSCF + ACF-VSS PDUs, and hands parsed
//! `ParsedVssMessage` values to the provider.

use std::io;
use std::net::UdpSocket;
use std::os::unix::io::RawFd;

use crate::open1722::ffi;
use crate::open1722_vss as vss;

/// EtherType assigned to TSN / IEEE 1722 traffic.
const ETH_P_TSN: u16 = 0x22F0;

/// Underlying transport — UDP for local loopback testing or raw AF_PACKET
/// for real Ethernet frames.
enum SocketKind {
    Udp(UdpSocket),
    Raw(RawFd),
}

/// Listens for ACF-VSS frames on either a raw AF_PACKET socket (ETH_P_TSN)
/// or a UDP socket, and parses them into `ParsedVssMessage` structs.
pub struct AcfVssListener {
    socket: SocketKind,
}

impl AcfVssListener {
    /// Create a listener on a UDP port (default IEEE 1722 UDP port: 17220).
    pub fn new_udp(port: u16) -> io::Result<Self> {
        let addr = format!("0.0.0.0:{port}");
        let udp_socket = UdpSocket::bind(&addr)?;
        Ok(Self {
            socket: SocketKind::Udp(udp_socket),
        })
    }

    /// Create a listener on a raw AF_PACKET socket for ETH_P_TSN (0x22F0).
    pub fn new_raw(ifname: &str, macaddr: [u8; 6]) -> io::Result<Self> {
        let fd = unsafe {
            let fd = libc::socket(
                libc::AF_PACKET,
                libc::SOCK_RAW | libc::SOCK_NONBLOCK,
                (ETH_P_TSN).to_be() as i32,
            );
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            fd
        };

        unsafe {
            let if_idx = libc::if_nametoindex(ifname.as_ptr() as *const _);
            if if_idx == 0 {
                libc::close(fd);
                return Err(io::Error::last_os_error());
            }

            let mut addr: libc::sockaddr_ll = std::mem::zeroed();
            addr.sll_family = libc::AF_PACKET as u16;
            addr.sll_protocol = ETH_P_TSN.to_be();
            addr.sll_ifindex = if_idx as i32;
            addr.sll_halen = libc::ETH_ALEN as u8;
            addr.sll_addr[..6].copy_from_slice(&macaddr);

            let res = libc::bind(
                fd,
                &addr as *const libc::sockaddr_ll as *const libc::sockaddr,
                std::mem::size_of::<libc::sockaddr_ll>() as u32,
            );
            if res < 0 {
                libc::close(fd);
                return Err(io::Error::last_os_error());
            }

            let val: libc::c_int = 1;
            libc::setsockopt(
                fd,
                libc::SOL_PACKET,
                libc::PACKET_ADD_MEMBERSHIP,
                &val as *const libc::c_int as *const libc::c_void,
                std::mem::size_of::<libc::c_int>() as u32,
            );
        }

        Ok(Self {
            socket: SocketKind::Raw(fd),
        })
    }

    /// Receive and parse one batch of ACF-VSS frames from the socket.
    /// Returns an empty `Vec` when no valid VSS messages are found in the buffer.
    pub fn recv_vss(&self) -> io::Result<Vec<vss::ParsedVssMessage>> {
        let mut buf = [0u8; 1500];
        let (n, has_udp) = match &self.socket {
            SocketKind::Udp(udp) => {
                let (n, _src) = udp.recv_from(&mut buf)?;
                (n, true)
            }
            SocketKind::Raw(fd) => {
                let n = unsafe {
                    libc::recv(
                        *fd,
                        buf.as_mut_ptr() as *mut libc::c_void,
                        buf.len(),
                        0,
                    )
                };
                if n <= 0 {
                    return Err(io::Error::last_os_error());
                }
                (n as usize, false)
            }
        };

        self.parse_buf(&buf[..n], has_udp)
    }

    /// Walk through a raw buffer, detect NTSCF/TSCF common headers, skip to
    /// the ACF-VSS payload, and parse each valid VSS PDU found.
    fn parse_buf(&self, buf: &[u8], has_udp: bool) -> io::Result<Vec<vss::ParsedVssMessage>> {
        let n = buf.len();
        let mut results = Vec::new();
        let mut offset = 0;

        if has_udp {
            if n < ffi::AVTP_UDP_HEADER_LEN {
                return Ok(results);
            }
            offset += ffi::AVTP_UDP_HEADER_LEN;
        }

        while offset + 4 <= n {
            let cf_ptr = &buf[offset] as *const u8 as *const ffi::Avtp_CommonHeader;

            // SAFETY: cf_ptr points into buf, which stays alive for the whole loop body
            let subtype = unsafe { ffi::open1722_ffi_common_header_get_subtype(cf_ptr) };

            let (cf_header_size, data_length) = if subtype == ffi::AVTP_SUBTYPE_TSCF {
                let tscf = cf_ptr as *const ffi::Avtp_Tscf;
                (
                    ffi::AVTP_TSCF_HEADER_LEN,
                    unsafe { ffi::open1722_ffi_tscf_get_stream_data_length(tscf) } as usize,
                )
            } else {
                let ntscf = cf_ptr as *const ffi::Avtp_Ntscf;
                (
                    ffi::AVTP_NTSCF_HEADER_LEN,
                    unsafe { ffi::open1722_ffi_ntscf_get_data_length(ntscf) } as usize,
                )
            };

            offset += cf_header_size;
            if offset + 4 > n {
                break;
            }

            let acf_ptr = &buf[offset] as *const u8 as *const ffi::Avtp_AcfCommon;
            let acf_type = unsafe { ffi::open1722_ffi_acf_common_get_acf_msg_type(acf_ptr) };

            if acf_type == ffi::AVTP_ACF_TYPE_VSS {
                let vss_pdu = acf_ptr as *const ffi::Avtp_Vss;
                if let Some(msg) = vss::parse_vss_frame(vss_pdu) {
                    results.push(msg);
                }
            }

            if data_length > 0 {
                offset += data_length;
            } else {
                break;
            }
        }

        Ok(results)
    }
}

// Raw AF_PACKET sockets must be closed explicitly; UDP sockets are managed
// by Rust's `UdpSocket` drop implementation.
impl Drop for AcfVssListener {
    fn drop(&mut self) {
        if let SocketKind::Raw(fd) = self.socket {
            unsafe { libc::close(fd) };
        }
    }
}

// SAFETY: The raw file descriptor is safe to send and share across threads
// since we never read from it concurrently.
unsafe impl Send for AcfVssListener {}
unsafe impl Sync for AcfVssListener {}
