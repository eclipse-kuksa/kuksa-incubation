/********************************************************************************
 * Copyright (c) 2024 Contributors to the Eclipse Foundation
 *
 * See the NOTICE file(s) distributed with this work for additional
 * information regarding copyright ownership.
 *
 * This program and the accompanying materials are made available under the
 * terms of the Apache License 2.0 which is available at
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * SPDX-License-Identifier: Apache-2.0
 ********************************************************************************/

use serde::{Deserialize, Deserializer};

#[derive(Deserialize, Clone)]
pub struct AdapterConfig {
    pub general_config: GeneralConfig,
    pub can_config: CanConfig,
    pub pid_table: Vec<PidEntry>,
}

#[derive(Deserialize, Clone)]
pub struct GeneralConfig {
    pub broker_ip: String,
    pub broker_port: String,
    pub dbcfile: String,
}

#[derive(Deserialize, Clone)]
pub struct CanConfig {
    pub can_interface: String,
    pub use_extended_id: bool,
    #[serde(deserialize_with = "from_hex_string")]
    pub tx_id: u32,
    #[serde(deserialize_with = "from_hex_string")]
    pub rx_id: u32,
    pub socket_can_type: String,
    pub socket_can_protocol: String,
}

#[derive(Deserialize, Clone)]
pub struct PidEntry {
    #[serde(deserialize_with = "from_hex_string_to_bytes")]
    pub request_pid: Vec<u8>,
    #[serde(deserialize_with = "from_hex_string_to_bytes")]
    pub response_pid: Vec<u8>,
    pub response_timeout_ms: u32,
    // The 'description' field is currently unused.
    #[allow(dead_code)]
    pub description: String,
    // The 'expected_response_length' field is currently unused but will be used in future development.
    #[allow(dead_code)]
    pub expected_response_length: u32,
    pub interval_ms: u32,
    pub dbc_signal_name: String,
    pub vss_signal: VssSignal,
}

#[derive(Deserialize, Clone)]
pub struct VssSignal {
    pub signal_name: String,
    pub datatype: String,
    pub unit: String,
}

fn from_hex_string<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    u32::from_str_radix(s.trim_start_matches("0x"), 16).map_err(serde::de::Error::custom)
}

fn from_hex_string_to_bytes<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    let bytes: Result<Vec<u8>, _> = s
        .split_whitespace()
        .map(|hex_str| u8::from_str_radix(hex_str.trim_start_matches("0x"), 16))
        .collect();

    bytes.map_err(serde::de::Error::custom)
}
