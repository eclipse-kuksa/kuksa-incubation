/********************************************************************************
* Copyright (c) 2026 Contributors to the Eclipse Foundation
*
* This program and the accompanying materials are made available under the
* terms of the Apache License 2.0 which is available at
* http://www.apache.org/licenses/LICENSE-2.0
*
* SPDX-License-Identifier: Apache-2.0
********************************************************************************/

//! ACF-VSS gateway binary — replays CSV signal logs as ACF-VSS frames over
//! UDP or raw Ethernet.  Use `--help` for the full set of options.
#![allow(dead_code)]
mod csv_reader;
mod gateway;
mod gateway_config;

use std::io;

use clap::Parser;
use log::info;

use gateway::Gateway;
use gateway_config::GatewayConfig;

/// Parse CLI arguments, load the CSV, create the appropriate transport
/// socket (UDP or raw), and enter the replay loop.
fn main() -> io::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = GatewayConfig::parse();

    let records = csv_reader::load_csv(&config.csv_file)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    info!("Loaded {} records from {}", records.len(), config.csv_file);

    let gateway = if let Some(ref ifname) = config.interface {
        let mac_str = config.mac_address.as_ref().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "MAC address required for raw Ethernet mode")
        })?;
        let mac = parse_mac(mac_str)?;
        Gateway::new_raw(ifname, mac)?
    } else {
        Gateway::new_udp(&config.destination, config.udp_port)?
    };

    info!("Starting CSV replay{}...", if config.infinite { " (infinite loop)" } else { "" });
    gateway.replay(
        &records,
        config.delay_multiplier,
        config.include_actuation,
        config.infinite,
    )
}

/// Parse a colon-separated hex MAC address (e.g. `01:00:5e:00:00:01`) into
/// a 6-byte array.
fn parse_mac(s: &str) -> io::Result<[u8; 6]> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 6 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "MAC address must be 6 colon-separated hex bytes",
        ));
    }
    let mut mac = [0u8; 6];
    for (i, part) in parts.iter().enumerate() {
        mac[i] = u8::from_str_radix(part, 16)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("Invalid MAC: {e}")))?;
    }
    Ok(mac)
}
