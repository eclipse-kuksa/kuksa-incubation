/********************************************************************************
* Copyright (c) 2026 Contributors to the Eclipse Foundation
*
* This program and the accompanying materials are made available under the
* terms of the Apache License 2.0 which is available at
* http://www.apache.org/licenses/LICENSE-2.0
*
* SPDX-License-Identifier: Apache-2.0
********************************************************************************/

//! CSV signal replay — reads rows from a CSV log and converts them into
//! typed in-memory values ready for serialisation into ACF-VSS frames.

use std::fmt;

use serde::Deserialize;

/// One row from a CSV signal log.
///
/// Expected CSV header: `field,signal,value,delay,datatype`
#[derive(Debug, Deserialize, Clone)]
pub struct CsvRecord {
    /// `current` or `target`
    #[serde(rename = "field")]
    pub r#type: String,
    /// VSS path, e.g. `Vehicle.Speed`
    pub signal: String,
    /// Raw value string as it appears in the CSV
    pub value: String,
    /// Delay in seconds after publishing this signal
    pub delay: f64,
    /// VSS datatype name: FLOAT, INT32, BOOLEAN, etc.
    pub datatype: String,
}

impl CsvRecord {
    /// Returns true if this row is an actuation (target value) row.
    pub fn is_actuation(&self) -> bool {
        self.r#type.eq_ignore_ascii_case("target")
    }
}

/// Parsed CSV signal value, ready for conversion to VSS wire format.
#[derive(Debug, Clone)]
pub enum CsvValue {
    Bool(bool),
    Int32(i32),
    Int64(i64),
    Uint32(u32),
    Uint64(u64),
    Float(f32),
    Double(f64),
    String(String),
}

impl fmt::Display for CsvValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CsvValue::Bool(v) => write!(f, "{v}"),
            CsvValue::Int32(v) => write!(f, "{v}"),
            CsvValue::Int64(v) => write!(f, "{v}"),
            CsvValue::Uint32(v) => write!(f, "{v}"),
            CsvValue::Uint64(v) => write!(f, "{v}"),
            CsvValue::Float(v) => write!(f, "{v}"),
            CsvValue::Double(v) => write!(f, "{v}"),
            CsvValue::String(v) => write!(f, "{v}"),
        }
    }
}

impl CsvValue {
    /// Parse a raw CSV value string into the typed value indicated by `datatype`.
    pub fn parse(raw: &str, datatype: &str) -> Result<Self, String> {
        match datatype.to_uppercase().as_str() {
            "STRING" => Ok(CsvValue::String(raw.to_string())),
            "BOOLEAN" => {
                let b = match raw.to_lowercase().as_str() {
                    "true" | "1" => true,
                    "false" | "0" => false,
                    other => return Err(format!("invalid boolean: {other}")),
                };
                Ok(CsvValue::Bool(b))
            }
            "INT8" | "INT16" | "INT32" => {
                let n: i32 = raw
                    .parse()
                    .map_err(|e| format!("invalid int32 '{raw}': {e}"))?;
                Ok(CsvValue::Int32(n))
            }
            "INT64" => {
                let n: i64 = raw
                    .parse()
                    .map_err(|e| format!("invalid int64 '{raw}': {e}"))?;
                Ok(CsvValue::Int64(n))
            }
            "UINT8" | "UINT16" | "UINT32" => {
                let n: u32 = raw
                    .parse()
                    .map_err(|e| format!("invalid uint32 '{raw}': {e}"))?;
                Ok(CsvValue::Uint32(n))
            }
            "UINT64" => {
                let n: u64 = raw
                    .parse()
                    .map_err(|e| format!("invalid uint64 '{raw}': {e}"))?;
                Ok(CsvValue::Uint64(n))
            }
            "FLOAT" => {
                let n: f32 = raw
                    .parse()
                    .map_err(|e| format!("invalid float '{raw}': {e}"))?;
                Ok(CsvValue::Float(n))
            }
            "DOUBLE" => {
                let n: f64 = raw
                    .parse()
                    .map_err(|e| format!("invalid double '{raw}': {e}"))?;
                Ok(CsvValue::Double(n))
            }
            other => Err(format!("unsupported datatype: {other}")),
        }
    }
}

/// Load all records from a CSV file. Skips the header row.
pub fn load_csv(path: &str) -> Result<Vec<CsvRecord>, String> {
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(b',')
        .flexible(true)
        .from_path(path)
        .map_err(|e| format!("cannot open {path}: {e}"))?;

    let mut records = Vec::new();
    for result in rdr.deserialize() {
        let record: CsvRecord = result.map_err(|e| format!("CSV parse error: {e}"))?;
        records.push(record);
    }
    Ok(records)
}
