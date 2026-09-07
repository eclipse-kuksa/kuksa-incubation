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

use open1722::acf::ntscf::Ntscf;
use open1722::acf::tscf::Tscf;
use open1722::{CommonHeader, Subtype};

use crate::open1722_vss as vss;

/// EtherType assigned to TSN / IEEE 1722 traffic.
const ETH_P_TSN: u16 = 0x22F0;

/// Number of bytes of the IEEE 1722 UDP encapsulation header that precedes
/// AVTP PDUs when carried over UDP/IPv4.
const UDP_HEADER_LEN: usize = 4;

/// COVESA VSS ACF message type (`AVTP_ACF_TYPE_VSS`).
///
/// This value is not present in the `open1722` crate's `AcfMsgType` enum,
/// because VSS is a custom (non-IEEE) format. We therefore read it as a raw
/// bit field below. See `parse_buf`.
///
/// The ACF message type occupies the 7 most-significant bits of the first
/// octet of the ACF common header, so the raw octet is shifted right by one
/// before comparing.
const ACF_TYPE_VSS: u8 = 0x42;

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
            if n < UDP_HEADER_LEN {
                return Ok(results);
            }
            offset += UDP_HEADER_LEN;
        }

        while offset + 4 <= n {
            // Peek at the AVTP common header to dispatch on the subtype.
            let Ok(common) = CommonHeader::new(&buf[offset..]) else {
                break;
            };
            let subtype = common.subtype_raw();

            let (header_size, data_length) = if subtype == Subtype::Tscf.as_u8() {
                let Ok(tscf) = Tscf::new(&buf[offset..]) else {
                    break;
                };
                (
                    open1722::acf::tscf::HEADER_LEN,
                    tscf.stream_data_length() as usize,
                )
            } else {
                let Ok(ntscf) = Ntscf::new(&buf[offset..]) else {
                    break;
                };
                (
                    open1722::acf::ntscf::HEADER_LEN,
                    ntscf.ntscf_data_length() as usize,
                )
            };

            offset += header_size;
            if offset + 4 > n {
                break;
            }

            // Detect a VSS ACF message. The ACF message type is the 7
            // most-significant bits of the first octet of the ACF common
            // header. VSS is a custom format (type 0x42) that is not part of
            // the `AcfMsgType` enum, so we read the raw bit field instead of
            // using a typed accessor.
            let acf_msg_type = buf[offset] >> 1;
            if acf_msg_type == ACF_TYPE_VSS
                && let Some(msg) = vss::parse_vss_frame(&buf[offset..])
            {
                results.push(msg);
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
