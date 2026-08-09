/********************************************************************************
* Copyright (c) 2026 Contributors to the Eclipse Foundation
*
* This program and the accompanying materials are made available under the
* terms of the Apache License 2.0 which is available at
* http://www.apache.org/licenses/LICENSE-2.0
*
* SPDX-License-Identifier: Apache-2.0
********************************************************************************/

//! Command-line configuration for the ACF-VSS gateway binary (`main_gateway`).

/// Command-line configuration for the ACF-VSS gateway (CSV replay).
#[derive(clap::Parser, Debug, Clone)]
#[command(name = "acf-vss-gateway")]
#[command(about = "Replays CSV signal logs as ACF-VSS frames over UDP or raw Ethernet")]
pub struct GatewayConfig {
    /// CSV file to replay
    #[arg(long, env = "GATEWAY_CSV_FILE")]
    pub csv_file: String,

    /// Loop over CSV from start after reaching end
    #[arg(long)]
    pub infinite: bool,

    /// Process target/actuation rows (default: skip them)
    #[arg(long)]
    pub include_actuation: bool,

    /// Multiplier for CSV row delays (0 = send as fast as possible)
    #[arg(long, default_value = "1.0")]
    pub delay_multiplier: f64,

    /// Destination IP address (UDP mode, default: 127.0.0.1)
    #[arg(long, default_value = "127.0.0.1")]
    pub destination: String,

    /// Destination UDP port
    #[arg(long, default_value = "17220")]
    pub udp_port: u16,

    /// Network interface for raw Ethernet mode
    #[arg(long)]
    pub interface: Option<String>,

    /// Destination MAC for raw Ethernet mode (required with --interface)
    #[arg(long, requires = "interface")]
    pub mac_address: Option<String>,
}
