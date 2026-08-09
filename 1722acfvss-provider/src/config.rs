/********************************************************************************
* Copyright (c) 2026 Contributors to the Eclipse Foundation
*
* This program and the accompanying materials are made available under the
* terms of the Apache License 2.0 which is available at
* http://www.apache.org/licenses/LICENSE-2.0
*
* SPDX-License-Identifier: Apache-2.0
********************************************************************************/

use clap::Parser;

/// Command-line configuration for the Open1722 ACF-VSS to KUKSA Databroker provider.
#[derive(Parser, Debug, Clone)]
#[command(name = "open1722-kuksa-provider")]
#[command(about = "Bridges Open1722 ACF-VSS frames to KUKSA Databroker VSS signals")]
pub struct Config {
    /// KUKSA Databroker gRPC endpoint
    #[arg(
        long,
        env = "KUKSA_HOST",
        default_value = "http://localhost:55555"
    )]
    pub kuksa_host: String,

    /// UDP port to listen on for ACF-VSS frames (default IEEE 1722 port)
    #[arg(long, default_value = "17220")]
    pub udp_port: u16,

    /// Network interface for raw Ethernet mode (ETH_P_TSN)
    #[arg(long)]
    pub interface: Option<String>,

    /// Destination MAC address for raw Ethernet mode (e.g. 01:00:5e:00:00:01).
    /// Required when --interface is set.
    #[arg(
        long,
        requires = "interface",
        help = "Destination MAC address for raw Ethernet mode (e.g. 01:00:5e:00:00:01)"
    )]
    pub mac_address: Option<String>,
}
