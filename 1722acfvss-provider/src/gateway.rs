/********************************************************************************
* Copyright (c) 2026 Contributors to the Eclipse Foundation
*
* This program and the accompanying materials are made available under the
* terms of the Apache License 2.0 which is available at
* http://www.apache.org/licenses/LICENSE-2.0
*
* SPDX-License-Identifier: Apache-2.0
********************************************************************************/

//! ACF-VSS gateway — replays CSV signal logs as ACF-VSS frames over UDP or
//! raw AF_PACKET Ethernet.  Builds NTSCF + VSS PDUs in a stack buffer and
//! writes them directly to the chosen socket.

use std::io;
use std::net::UdpSocket;
use std::os::unix::io::RawFd;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use log::info;

use crate::csv_reader::CsvRecord;
use crate::open1722::ffi;

/// Hard-coded IEEE 1722 stream ID used for all outgoing frames.
const STREAM_ID: u64 = 0xAABB_CCDD_EEFF_0001;
/// EtherType assigned to TSN / IEEE 1722 traffic.
const ETH_P_TSN: u16 = 0x22F0;

/// Transport socket — either a connected UDP socket (for local testing) or a
/// raw AF_PACKET descriptor bound to a specific Ethernet interface.
enum Sock {
    Udp(UdpSocket),
    Raw(RawFd),
}

/// Sends pre-built ACF-VSS frames (NTSCF + VSS) over UDP or raw Ethernet.
pub struct Gateway {
    sock: Sock,
}

impl Gateway {
    /// Create a gateway that sends frames via UDP to `dest:port`.
    pub fn new_udp(dest: &str, port: u16) -> io::Result<Self> {
        let udp = UdpSocket::bind("0.0.0.0:0")?;
        udp.connect(format!("{dest}:{port}"))?;
        Ok(Self {
            sock: Sock::Udp(udp),
        })
    }

    /// Create a gateway that sends raw L2 frames via `AF_PACKET` on `ifname`
    /// addressed to `macaddr`.  The socket is connected so `send()` can be
    /// used without specifying the destination each time.
    pub fn new_raw(ifname: &str, macaddr: [u8; 6]) -> io::Result<Self> {
        let fd = unsafe {
            let fd = libc::socket(
                libc::AF_PACKET,
                libc::SOCK_RAW,
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

            let res = libc::connect(
                fd,
                &addr as *const libc::sockaddr_ll as *const libc::sockaddr,
                std::mem::size_of::<libc::sockaddr_ll>() as u32,
            );
            if res < 0 {
                libc::close(fd);
                return Err(io::Error::last_os_error());
            }
        }

        Ok(Self {
            sock: Sock::Raw(fd),
        })
    }

    /// Build and send one ACF-VSS frame for a single CSV record.
    pub fn send_vss(&self, record: &CsvRecord, seq: &mut u8, udp_seq: &mut u32) -> io::Result<usize> {
        let is_udp = matches!(self.sock, Sock::Udp(_));
        let mut buf = [0u8; 1500];
        let mut off = 0usize;

        if is_udp {
            let seq_be = udp_seq.to_be_bytes();
            buf[off..off + 4].copy_from_slice(&seq_be);
            *udp_seq += 1;
            off += ffi::AVTP_UDP_HEADER_LEN;
        }

        let ntscf = &mut buf[off] as *mut u8 as *mut ffi::Avtp_Ntscf;
        unsafe {
            ffi::open1722_ffi_ntscf_init(ntscf);
            ffi::open1722_ffi_ntscf_set_sequence_num(ntscf, *seq);
            ffi::open1722_ffi_ntscf_set_stream_id(ntscf, STREAM_ID);
        }
        *seq = seq.wrapping_add(1);
        off += ffi::AVTP_NTSCF_HEADER_LEN;

        let vss = &mut buf[off] as *mut u8 as *mut ffi::Avtp_Vss;
        let vss_len = build_vss_frame(vss, record)?;
        off += vss_len;

        unsafe {
            ffi::open1722_ffi_ntscf_set_data_length(ntscf, vss_len as u16);
        }

        let n = match &self.sock {
            Sock::Udp(udp) => udp.send(&buf[..off])?,
            Sock::Raw(fd) => unsafe {
                let n = libc::send(
                    *fd,
                    buf.as_ptr() as *const libc::c_void,
                    off,
                    0,
                );
                if n < 0 {
                    return Err(io::Error::last_os_error());
                }
                n as usize
            },
        };

        Ok(n)
    }

    /// Send every CSV record in order, respecting per-row delays scaled by
    /// `delay_multiplier`.  When `infinite` is true the loop restarts after
    /// the last row.  Actuation rows are skipped unless `include_actuation`
    /// is set.
    pub fn replay(
        &self,
        records: &[CsvRecord],
        delay_multiplier: f64,
        include_actuation: bool,
        infinite: bool,
    ) -> io::Result<()> {
        let mut seq: u8 = 0;
        let mut udp_seq: u32 = 0;
        loop {
            for record in records {
                if record.is_actuation() && !include_actuation {
                    continue;
                }
                let n = self.send_vss(record, &mut seq, &mut udp_seq)?;
                info!(
                    "Sent {}: {} = {} ({} bytes)",
                    record.signal, record.value, record.datatype, n
                );
                if record.delay > 0.0 {
                    let d = Duration::from_secs_f64(record.delay * delay_multiplier);
                    std::thread::sleep(d);
                }
            }
            if !infinite {
                break;
            }
            info!("--- restarting CSV replay ---");
        }
        Ok(())
    }
}

// Raw AF_PACKET sockets are not automatically closed — we must explicitly
// close the file descriptor.  UDP sockets are managed by Rust's `UdpSocket`.
impl Drop for Gateway {
    fn drop(&mut self) {
        if let Sock::Raw(fd) = self.sock {
            unsafe { libc::close(fd) };
        }
    }
}

// SAFETY: The raw fd is only used from the thread that owns the `Gateway`,
// and `send()` on a connected AF_PACKET socket is thread-safe.
unsafe impl Send for Gateway {}
unsafe impl Sync for Gateway {}

/// Build one VSS PDU in the buffer pointed to by `vss` and return the
/// 4-byte-aligned total length (including padding) for `NTSCF_SetDataLength`.
fn build_vss_frame(vss: *mut ffi::Avtp_Vss, record: &CsvRecord) -> io::Result<usize> {
    let parsed = crate::csv_reader::CsvValue::parse(&record.value, &record.datatype)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

    let (vss_dt, data_size, data) = make_vss_data(&record.datatype, &parsed);
    let op_code: i32 = if record.is_actuation() { 1 } else { 0 };

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;

    let vss_path = ffi::make_vss_path_interop(&record.signal);

    unsafe {
        ffi::Avtp_Vss_Init(vss);
        ffi::Avtp_Vss_SetMsgTimestamp(vss, ts);
        ffi::Avtp_Vss_EnableMtv(vss);
        ffi::Avtp_Vss_SetAddrMode(vss, ffi::VSS_INTEROP_MODE);
        ffi::Avtp_Vss_SetOpCode(vss, op_code);
        ffi::Avtp_Vss_SetDatatype(vss, vss_dt);
        ffi::Avtp_Vss_SetVssPath(vss, &vss_path as *const ffi::VssPath as *mut ffi::VssPath);
        ffi::Avtp_Vss_SetVssData(vss, &data as *const ffi::VssData as *mut ffi::VssData);
    }

    let path_len = 2u16 + record.signal.len() as u16;
    let unpadded_len = ffi::AVTP_VSS_FIXED_HEADER_LEN as u16 + path_len + data_size;

    unsafe {
        ffi::Avtp_Vss_Pad(vss, unpadded_len);
        ffi::free_vss_path(vss_path);
    }

    if record.datatype.eq_ignore_ascii_case("STRING") {
        let ptr = unsafe { data.data_string };
        if !ptr.is_null() {
            unsafe { ffi::free_vss_data_string(ptr) };
        }
    }

    Ok((unpadded_len as usize).div_ceil(4) * 4)
}

/// Returns (vss_datatype_enum, wire_size_bytes, VssData union).
fn make_vss_data(dt: &str, val: &crate::csv_reader::CsvValue) -> (i32, u16, ffi::VssData) {
    use crate::csv_reader::CsvValue::*;
    match dt.to_uppercase().as_str() {
        "INT8" => (0x01, 1, ffi::VssData {
            data_int8: match val { Int32(n) => *n as i8, _ => 0 },
        }),
        "UINT8" => (0x00, 1, ffi::VssData {
            data_uint8: match val { Uint32(n) => *n as u8, _ => 0 },
        }),
        "INT16" => (0x03, 2, ffi::VssData {
            data_int16: match val { Int32(n) => *n as i16, _ => 0 },
        }),
        "UINT16" => (0x02, 2, ffi::VssData {
            data_uint16: match val { Uint32(n) => *n as u16, _ => 0 },
        }),
        "INT32" => (0x05, 4, ffi::VssData {
            data_int32: match val { Int32(n) => *n, _ => 0 },
        }),
        "UINT32" => (0x04, 4, ffi::VssData {
            data_uint32: match val { Uint32(n) => *n, _ => 0 },
        }),
        "INT64" => (0x07, 8, ffi::VssData {
            data_int64: match val { Int64(n) => *n, _ => 0 },
        }),
        "UINT64" => (0x06, 8, ffi::VssData {
            data_uint64: match val { Uint64(n) => *n, _ => 0 },
        }),
        "FLOAT" => (0x09, 4, ffi::VssData {
            data_float: match val { Float(n) => *n, _ => 0.0 },
        }),
        "DOUBLE" => (0x0A, 8, ffi::VssData {
            data_double: match val { Double(n) => *n, _ => 0.0 },
        }),
        "BOOLEAN" => (0x08, 1, ffi::VssData {
            data_bool: match val { Bool(b) => *b as u8, _ => 0 },
        }),
        "STRING" => {
            let s = match val { String(s) => s.as_str(), _ => "" };
            let (data, _ptr) = ffi::make_vss_data_string(s);
            (0x0B, 2 + s.len() as u16, data)
        }
        _ => (0x09, 4, ffi::VssData { data_float: 0.0 }),
    }
}
