/********************************************************************************
* Copyright (c) 2026 Contributors to the Eclipse Foundation
*
* This program and the accompanying materials are made available under the
* terms of the Apache License 2.0 which is available at
* http://www.apache.org/licenses/LICENSE-2.0
*
* SPDX-License-Identifier: Apache-2.0
********************************************************************************/

//! Safe wrapper around the `open1722` crate for parsing ACF-VSS PDU frames.
//!
//! The upstream `open1722` crate provides the `Vss` view, which reads the
//! fixed header, the variable-length path and the datatype-dispatched data
//! section, so there is no need to hand-roll path extraction or the value
//! union decoding here.

use open1722::acf::custom::vss::{Data, Path, Vss};
use open1722::acf::custom::{Datatype, OpCode};

#[derive(Debug)]
pub struct ParsedVssMessage {
    pub path: String,
    pub op_code: OpCode,
    pub datatype: Datatype,
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
/// timestamp and value. The `buf` slice must start at the ACF VSS PDU.
///
/// Returns `None` if the buffer is too short to hold a well-formed VSS PDU or
/// if any field cannot be decoded.
pub fn parse_vss_frame(buf: &[u8]) -> Option<ParsedVssMessage> {
    let vss = Vss::new(buf).ok()?;

    let path = match vss.path().ok()? {
        Path::Interop(bytes) => String::from_utf8_lossy(bytes).to_string(),
        Path::StaticId(id) => id.to_string(),
    };

    let op_code = vss.op_code().ok()?;
    let datatype = vss.datatype().ok()?;
    let timestamp = vss.message_timestamp();

    let value = match vss.data().ok()? {
        Data::Bool(v) => VssValue::Bool(v),
        Data::I8(v) => VssValue::Int8(v),
        Data::I16(v) => VssValue::Int16(v),
        Data::I32(v) => VssValue::Int32(v),
        Data::I64(v) => VssValue::Int64(v),
        Data::U8(v) => VssValue::Uint8(v),
        Data::U16(v) => VssValue::Uint16(v),
        Data::U32(v) => VssValue::Uint32(v),
        Data::U64(v) => VssValue::Uint64(v),
        Data::F32(v) => VssValue::Float(v),
        Data::F64(v) => VssValue::Double(v),
        Data::String(v) => VssValue::String(String::from_utf8_lossy(v).to_string()),
        // Array datatypes are not mapped to KUKSA VSS values yet.
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
