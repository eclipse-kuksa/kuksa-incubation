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
use open1722::acf::custom::vss::{Data, Path, Vss};
use open1722::acf::custom::OpCode;
use open1722::acf::ntscf::Ntscf;
use open1722::Udp;

use crate::csv_reader::CsvRecord;
use crate::csv_reader::CsvValue;

/// Hard-coded IEEE 1722 stream ID used for all outgoing frames.
const STREAM_ID: u64 = 0xAABB_CCDD_EEFF_0001;
/// EtherType assigned to TSN / IEEE 1722 traffic.
const ETH_P_TSN: u16 = 0x22F0;
/// Number of bytes of the IEEE 1722 UDP encapsulation header.
const UDP_HEADER_LEN: usize = 4;

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

        let total = {
            let mut start = 0usize;

            if is_udp {
                let (udp_region, _rest) = buf.split_at_mut(UDP_HEADER_LEN);
                let mut udp = Udp::initialized(udp_region).map_err(open1722_err)?;
                udp.set_encapsulation_seq_no(*udp_seq);
                *udp_seq += 1;
                start = UDP_HEADER_LEN;
            }

            let (ntscf_region, vss_region) = buf[start..].split_at_mut(open1722::acf::ntscf::HEADER_LEN);
            let mut ntscf = Ntscf::initialized(ntscf_region).map_err(open1722_err)?;
            ntscf.set_sequence_num(*seq);
            ntscf.set_stream_id(STREAM_ID);
            *seq = seq.wrapping_add(1);

            let mut vss = Vss::initialized(vss_region).map_err(open1722_err)?;
            build_vss_frame(&mut vss, record)?;
            let vss_len = vss.message_length() as usize;
            ntscf.set_ntscf_data_length(vss_len as u16);

            start + open1722::acf::ntscf::HEADER_LEN + vss_len
        };

        let n = match &self.sock {
            Sock::Udp(udp) => udp.send(&buf[..total])?,
            Sock::Raw(fd) => unsafe {
                let n = libc::send(
                    *fd,
                    buf.as_ptr() as *const libc::c_void,
                    total,
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

/// Maps an `open1722::Error` to an `io::Error` for the socket-based API.
fn open1722_err(e: open1722::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, e)
}

/// Populates the VSS PDU header, path and data, and finalizes the length and
/// trailing pad. `vss` must wrap a mutable buffer large enough for the frame.
fn build_vss_frame(vss: &mut Vss<&mut [u8]>, record: &CsvRecord) -> io::Result<()> {
    let parsed = CsvValue::parse(&record.value, &record.datatype)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

    let (data, data_size) = make_vss_data(&record.datatype, &parsed);
    let op_code = if record.is_actuation() {
        OpCode::PublishTargetValue
    } else {
        OpCode::PublishCurrentValue
    };

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;

    vss.set_op_code(op_code);
    vss.set_message_timestamp(ts);
    vss.set_message_timestamp_valid(true);
    vss.set_path(Path::Interop(record.signal.as_bytes()))
        .map_err(open1722_err)?;
    vss.set_data(data).map_err(open1722_err)?;

    let path_size = 2u16 + record.signal.len() as u16;
    let payload_len = path_size + data_size;
    vss.set_payload_length(payload_len).map_err(open1722_err)?;

    Ok(())
}

/// Returns the VSS data payload and its wire size in bytes (excluding the
/// 2-byte length prefix that `set_data` adds for variable-length variants).
fn make_vss_data<'a>(dt: &str, val: &'a CsvValue) -> (Data<'a>, u16) {
    use CsvValue::*;
    match dt.to_uppercase().as_str() {
        "INT8" => (Data::I8(match val { Int32(n) => *n as i8, _ => 0 }), 1),
        "UINT8" => (Data::U8(match val { Uint32(n) => *n as u8, _ => 0 }), 1),
        "INT16" => (Data::I16(match val { Int32(n) => *n as i16, _ => 0 }), 2),
        "UINT16" => (Data::U16(match val { Uint32(n) => *n as u16, _ => 0 }), 2),
        "INT32" => (Data::I32(match val { Int32(n) => *n, _ => 0 }), 4),
        "UINT32" => (Data::U32(match val { Uint32(n) => *n, _ => 0 }), 4),
        "INT64" => (Data::I64(match val { Int64(n) => *n, _ => 0 }), 8),
        "UINT64" => (Data::U64(match val { Uint64(n) => *n, _ => 0 }), 8),
        "FLOAT" => (Data::F32(match val { Float(n) => *n, _ => 0.0 }), 4),
        "DOUBLE" => (Data::F64(match val { Double(n) => *n, _ => 0.0 }), 8),
        "BOOLEAN" => (Data::Bool(match val { Bool(b) => *b, _ => false }), 1),
        "STRING" => {
            let s = match val {
                String(s) => s.as_bytes(),
                _ => b"",
            };
            (Data::String(s), 2 + s.len() as u16)
        }
        _ => (Data::F32(0.0), 4),
    }
}
