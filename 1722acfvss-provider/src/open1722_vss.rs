/********************************************************************************
* Copyright (c) 2026 Contributors to the Eclipse Foundation
*
* This program and the accompanying materials are made available under the
* terms of the Apache License 2.0 which is available at
* http://www.apache.org/licenses/LICENSE-2.0
*
* SPDX-License-Identifier: Apache-2.0
********************************************************************************/

use crate::open1722::ffi;

#[derive(Debug)]
pub struct ParsedVssMessage {
    pub path: String,
    pub op_code: i32,
    pub datatype: i32,
    #[allow(dead_code)]
    pub timestamp: u64,
    pub value: VssValue,
}

#[derive(Debug)]
pub enum VssValue {
    Bool(bool),
    Int8(i8),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    Uint8(u8),
    Uint16(u16),
    Uint32(u32),
    Uint64(u64),
    Float(f32),
    Double(f64),
    String(String),
    Unknown,
}

/// Parses an ACF-VSS PDU frame and extracts the VSS path, op code, datatype,
/// timestamp and value. The acf_pdu must point to a valid ACF VSS PDU.
///
/// Path extraction is done in pure Rust because the C library's
/// `Avtp_Vss_GetVssPath` requires a pre-allocated buffer that we cannot
/// know the size of without calling `Avtp_Vss_CalcVssPathLength` first.
pub fn parse_vss_frame(acf_pdu: *const ffi::Avtp_Vss) -> Option<ParsedVssMessage> {
    if acf_pdu.is_null() {
        return None;
    }

    let op_code = unsafe { ffi::Avtp_Vss_GetOpCode(acf_pdu) };
    let datatype = unsafe { ffi::Avtp_Vss_GetDatatype(acf_pdu) };
    let timestamp = unsafe { ffi::Avtp_Vss_GetMsgTimestamp(acf_pdu) };
    let addr_mode = unsafe { ffi::Avtp_Vss_GetAddrMode(acf_pdu) };

    // The PDU layout after the 12-byte VSS fixed header:
    //   [path_length: 2 bytes BE] [path: path_length bytes]
    // Or in static-id mode:
    //   [static_id: 4 bytes]
    let pdu_bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(
            acf_pdu as *const u8,
            ffi::AVTP_VSS_FIXED_HEADER_LEN + 256,
        )
    };

    let path = if addr_mode == ffi::VSS_INTEROP_MODE {
        let path_data = &pdu_bytes[ffi::AVTP_VSS_FIXED_HEADER_LEN..];
        if path_data.len() < 2 {
            return None;
        }
        let path_len = u16::from_be_bytes([path_data[0], path_data[1]]) as usize;
        if path_data.len() < 2 + path_len {
            return None;
        }
        String::from_utf8_lossy(&path_data[2..2 + path_len]).to_string()
    } else {
        let path_data = &pdu_bytes[ffi::AVTP_VSS_FIXED_HEADER_LEN..];
        if path_data.len() < 4 {
            return None;
        }
        let id = u32::from_be_bytes([path_data[0], path_data[1], path_data[2], path_data[3]]);
        format!("{id}")
    };

    let mut data = ffi::VssData { data_uint64: 0 };
    unsafe { ffi::Avtp_Vss_GetVssData(acf_pdu, &mut data) };

    let value = match datatype {
        0 => VssValue::Uint8(unsafe { data.data_uint8 }),
        1 => VssValue::Int8(unsafe { data.data_int8 }),
        2 => VssValue::Uint16(unsafe { data.data_uint16 }),
        3 => VssValue::Int16(unsafe { data.data_int16 }),
        4 => VssValue::Uint32(unsafe { data.data_uint32 }),
        5 => VssValue::Int32(unsafe { data.data_int32 }),
        6 => VssValue::Uint64(unsafe { data.data_uint64 }),
        7 => VssValue::Int64(unsafe { data.data_int64 }),
        8 => VssValue::Bool(unsafe { data.data_bool != 0 }),
        9 => VssValue::Float(unsafe { data.data_float }),
        10 => VssValue::Double(unsafe { data.data_double }),
        11 => {
            let data_string = unsafe { &*data.data_string };
            if data_string.data.is_null() || data_string.data_length == 0 {
                VssValue::Unknown
            } else {
                unsafe {
                    let slice = std::slice::from_raw_parts(
                        data_string.data as *const u8,
                        data_string.data_length as usize,
                    );
                    VssValue::String(String::from_utf8_lossy(slice).to_string())
                }
            }
        }
        _ => VssValue::Unknown,
    };

    Some(ParsedVssMessage {
        path,
        op_code,
        datatype,
        timestamp,
        value,
    })
}
