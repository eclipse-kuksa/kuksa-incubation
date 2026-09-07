/********************************************************************************
* Copyright (c) 2026 Contributors to the Eclipse Foundation
*
* This program and the accompanying materials are made available under the
* terms of the Apache License 2.0 which is available at
* http://www.apache.org/licenses/LICENSE-2.0
*
* SPDX-License-Identifier: Apache-2.0
********************************************************************************/

//! Open1722 ACF-VSS → KUKSA Databroker bridge binary.
//!
//! Listens for ACF-VSS frames on UDP or raw Ethernet, parses them, and
//! publishes the decoded VSS signals to the KUKSA Databroker over gRPC.
mod config;
mod open1722_listener;
mod open1722_vss;
mod provider;

use std::collections::HashSet;
use std::io;

use clap::Parser;
use log::{error, info, warn};

use config::Config;
use provider::KuksaProvider;

/// Initialises logging, creates the listener and provider, then enters the
/// main receive-publish loop.
#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = Config::parse();

    let listener = if let Some(ref ifname) = config.interface {
        let mac_str = config.mac_address.as_ref().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "MAC address required for raw Ethernet mode")
        })?;
        let mac = parse_mac(mac_str)?;
        open1722_listener::AcfVssListener::new_raw(ifname, mac)?
    } else {
        open1722_listener::AcfVssListener::new_udp(config.udp_port)?
    };

    let mut provider = KuksaProvider::new(&config.kuksa_host);

    match provider.open_stream().await {
        Ok(()) => info!("Connected to KUKSA Databroker at {}", config.kuksa_host),
        Err(e) => {
            error!("Failed to connect to KUKSA Databroker: {e}");
            return Err(io::Error::new(io::ErrorKind::ConnectionRefused, e));
        }
    }

    info!("Listening for ACF-VSS frames...");

    let mut registered = HashSet::new();

    loop {
        match listener.recv_vss() {
            Ok(messages) => {
                for msg in messages {
                    info!(
                        "Received VSS: path={}, op_code={:?}, datatype={:?}, value={:?}",
                        msg.path, msg.op_code, msg.datatype, msg.value
                    );

                    if !registered.contains(&msg.path) {
                        match provider.register_signal(&msg.path, 0).await {
                            Ok(()) => {
                                registered.insert(msg.path.clone());
                            }
                            Err(e) => {
                                warn!("Failed to register signal {}: {e}", msg.path);
                                continue;
                            }
                        }
                    }

                    match provider.publish_value(&msg).await {
                        Ok(()) => {}
                        Err(e) => {
                            warn!("Failed to publish value for {}: {e}", msg.path);
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Error receiving data: {e}");
                continue;
            }
        }
    }
}

/// Parse a colon-separated hex MAC address into a 6-byte array.
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
