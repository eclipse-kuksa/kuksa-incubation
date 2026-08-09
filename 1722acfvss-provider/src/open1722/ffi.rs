/********************************************************************************
* Copyright (c) 2026 Contributors to the Eclipse Foundation
*
* This program and the accompanying materials are made available under the
* terms of the Apache License 2.0 which is available at
* http://www.apache.org/licenses/LICENSE-2.0
*
* SPDX-License-Identifier: Apache-2.0
********************************************************************************/

use std::ffi::{c_char, CString};

pub const AVTP_TSCF_HEADER_LEN: usize = 24;
pub const AVTP_NTSCF_HEADER_LEN: usize = 12;
pub const AVTP_UDP_HEADER_LEN: usize = 4;

pub const AVTP_SUBTYPE_TSCF: u8 = 0x5;
pub const AVTP_ACF_TYPE_VSS: u8 = 0x42;

pub const VSS_INTEROP_MODE: i32 = 0;
pub const AVTP_VSS_FIXED_HEADER_LEN: usize = 12;

#[repr(C)]
pub struct Avtp_CommonHeader {
    pub header: [u8; 4],
    pub payload: [u8; 0],
}

#[repr(C)]
pub struct Avtp_Ntscf {
    pub header: [u8; AVTP_NTSCF_HEADER_LEN],
    pub payload: [u8; 0],
}

#[repr(C)]
pub struct Avtp_Tscf {
    pub header: [u8; AVTP_TSCF_HEADER_LEN],
    pub payload: [u8; 0],
}

#[repr(C)]
pub struct Avtp_AcfCommon {
    pub header: [u8; 4],
    pub payload: [u8; 0],
}

#[repr(C)]
pub struct Avtp_Vss {
    pub header: [u8; AVTP_VSS_FIXED_HEADER_LEN],
    pub payload: [u8; 0],
}

#[derive(Copy, Clone)]
#[repr(C)]
#[allow(dead_code)]
pub struct VssInteropPath {
    pub path_length: u16,
    pub path: *mut c_char,
}

#[derive(Copy, Clone)]
#[repr(C)]
#[allow(dead_code)]
pub union VssPath {
    pub vss_interop_path: VssInteropPath,
    pub vss_static_id_path: u32,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct VssDataString {
    pub data_length: u16,
    pub data: *mut c_char,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub union VssData {
    pub data_uint8: u8,
    pub data_int8: i8,
    pub data_uint16: u16,
    pub data_int16: i16,
    pub data_uint32: u32,
    pub data_int32: i32,
    pub data_uint64: u64,
    pub data_int64: i64,
    pub data_bool: u8,
    pub data_float: f32,
    pub data_double: f64,
    pub data_string: *mut VssDataString,
}

pub type AvtpAcfMsgType = u8;
pub type VssAddrMode = i32;
pub type VssOpCode = i32;
pub type VssDatatype = i32;

// --- Decoding functions (libopen1722custom.so) ---

unsafe extern "C" {
    pub fn Avtp_Vss_GetAddrMode(pdu: *const Avtp_Vss) -> VssAddrMode;
    pub fn Avtp_Vss_GetOpCode(pdu: *const Avtp_Vss) -> VssOpCode;
    pub fn Avtp_Vss_GetDatatype(pdu: *const Avtp_Vss) -> VssDatatype;
    pub fn Avtp_Vss_GetMsgTimestamp(pdu: *const Avtp_Vss) -> u64;
    pub fn Avtp_Vss_GetVssData(pdu: *const Avtp_Vss, val: *mut VssData);
}

// --- Encoding functions (libopen1722custom.so, used by gateway) ---

unsafe extern "C" {
    #[allow(dead_code)]
    pub fn Avtp_Vss_Init(vss_pdu: *mut Avtp_Vss);
    #[allow(dead_code)]
    pub fn Avtp_Vss_EnableMtv(pdu: *mut Avtp_Vss);
    #[allow(dead_code)]
    pub fn Avtp_Vss_SetMsgTimestamp(pdu: *mut Avtp_Vss, val: u64);
    #[allow(dead_code)]
    pub fn Avtp_Vss_SetAddrMode(pdu: *mut Avtp_Vss, val: VssAddrMode);
    #[allow(dead_code)]
    pub fn Avtp_Vss_SetOpCode(pdu: *mut Avtp_Vss, val: VssOpCode);
    #[allow(dead_code)]
    pub fn Avtp_Vss_SetDatatype(pdu: *mut Avtp_Vss, val: VssDatatype);
    #[allow(dead_code)]
    pub fn Avtp_Vss_SetVssPath(pdu: *mut Avtp_Vss, val: *mut VssPath);
    #[allow(dead_code)]
    pub fn Avtp_Vss_SetVssData(pdu: *mut Avtp_Vss, val: *mut VssData);
    #[allow(dead_code)]
    pub fn Avtp_Vss_Pad(pdu: *mut Avtp_Vss, vss_length: u16);
}

// --- Wrapper decoding functions (libopen1722_ffi.so) ---

unsafe extern "C" {
    pub fn open1722_ffi_common_header_get_subtype(pdu: *const Avtp_CommonHeader) -> u8;
    pub fn open1722_ffi_acf_common_get_acf_msg_type(pdu: *const Avtp_AcfCommon) -> AvtpAcfMsgType;
    pub fn open1722_ffi_ntscf_get_data_length(pdu: *const Avtp_Ntscf) -> u16;
    pub fn open1722_ffi_tscf_get_stream_data_length(pdu: *const Avtp_Tscf) -> u16;
}

// --- Wrapper encoding functions (libopen1722_ffi.so, used by gateway) ---

unsafe extern "C" {
    #[allow(dead_code)]
    pub fn open1722_ffi_ntscf_init(pdu: *mut Avtp_Ntscf);
    #[allow(dead_code)]
    pub fn open1722_ffi_ntscf_set_sequence_num(pdu: *mut Avtp_Ntscf, value: u8);
    #[allow(dead_code)]
    pub fn open1722_ffi_ntscf_set_stream_id(pdu: *mut Avtp_Ntscf, value: u64);
    #[allow(dead_code)]
    pub fn open1722_ffi_ntscf_set_data_length(pdu: *mut Avtp_Ntscf, value: u16);
}

// --- Construction helpers (used by gateway) ---

/// Creates a VssPath in interop mode from a Rust string.
/// The returned path contains a pointer into a leaked CString; call
/// `free_vss_path` after `Avtp_Vss_SetVssPath` returns.
#[allow(dead_code)]
pub fn make_vss_path_interop(path: &str) -> VssPath {
    let c_path = CString::new(path).unwrap();
    let len = path.len() as u16;
    VssPath {
        vss_interop_path: VssInteropPath {
            path_length: len,
            path: c_path.into_raw(),
        },
    }
}

/// Frees the CString inside a VssPath created by `make_vss_path_interop`.
///
/// SAFETY: only call after `Avtp_Vss_SetVssPath` has returned.
#[allow(dead_code)]
pub unsafe fn free_vss_path(p: VssPath) {
    unsafe {
        if !p.vss_interop_path.path.is_null() {
            drop(CString::from_raw(p.vss_interop_path.path));
        }
    }
}

/// Creates a VssData union from a string value.
/// Returns the data union and the heap pointer that must be freed later
/// via `free_vss_data_string`.
#[allow(dead_code)]
pub fn make_vss_data_string(val: &str) -> (VssData, *mut VssDataString) {
    let c_val = CString::new(val).unwrap();
    let len = val.len() as u16;
    let boxed = Box::new(VssDataString {
        data_length: len,
        data: c_val.into_raw(),
    });
    let ptr = Box::into_raw(boxed);
    let data = VssData {
        data_string: ptr,
    };
    (data, ptr)
}

/// Frees the heap-allocated VssDataString + CString created by
/// `make_vss_data_string`.
///
/// SAFETY: only call after `Avtp_Vss_SetVssData` has returned.
#[allow(dead_code)]
pub unsafe fn free_vss_data_string(ptr: *mut VssDataString) {
    unsafe {
        if !ptr.is_null() {
            let ds = Box::from_raw(ptr);
            if !ds.data.is_null() {
                drop(CString::from_raw(ds.data));
            }
        }
    }
}
