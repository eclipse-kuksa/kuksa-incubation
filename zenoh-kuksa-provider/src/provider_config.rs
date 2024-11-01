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

use log::error;
use serde::{Deserialize, Serialize};
use zenoh::config::Config;
use zenoh::prelude::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub zenoh: ZenohConfig,
    pub kuksa: KuksaConfig,
    pub signals: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ZenohConfig {
    pub mode: String,
    pub connect: Vec<String>,
    pub key_exp: String,
    pub scouting: ScoutingConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScoutingConfig {
    pub multicast: MulticastConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MulticastConfig {
    pub enabled: bool,
    pub interface: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KuksaConfig {
    pub databroker_url: String,
}

impl ProviderConfig {
    pub fn to_zenoh_config(&self) -> Result<Config, String> {
        let mut config = Config::default();

        let mode = match self.zenoh.mode.as_str() {
            "peer" => WhatAmI::Peer,
            "client" => WhatAmI::Client,
            "router" => {
                error!("Router is not a valid Zenoh mode for runnig the provider");
                return Err("Invalid Zenoh mode".into());
            }
            _ => {
                error!("Invalid Zenoh mode specified in config");
                return Err("Invalid Zenoh mode".into());
            }
        };
        config.set_mode(Some(mode)).unwrap();

        if self.zenoh.scouting.multicast.enabled {
            config.scouting.multicast.set_enabled(Some(true)).unwrap();

            config
                .scouting
                .multicast
                .set_interface(Some(String::from(&self.zenoh.scouting.multicast.interface)))
                .unwrap();

            config.scouting.multicast.autoconnect();
        } else {
            config.scouting.multicast.set_enabled(Some(false)).unwrap();
            if self.zenoh.connect.is_empty() {
                error!("Configuration error: Scouting is disabled and no Zenoh router connection string provided.
                        Either enable scouting or specify a valid connection string in the configuration.");
                return Err("Invalid connection configuration.".into());
            } else {
                for endpoint in &self.zenoh.connect {
                    config.connect.endpoints.push(endpoint.parse().unwrap());
                }
            }
        }

        Ok(config)
    }

    pub fn remove_duplicate_active_signals(&mut self) {
        let mut seen = std::collections::HashSet::new();
        self.signals.retain(|signal| seen.insert(signal.clone()));
    }
}
