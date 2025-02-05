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

use crate::AdapterConfig;
use log::warn;
use std::{error::Error, fs};

pub fn read_config(path: &str) -> Result<AdapterConfig, Box<dyn Error>> {
    let config_str = fs::read_to_string(path)?;
    let config = serde_json::from_str(&config_str)?;

    Ok(config)
}

pub fn validate_adapter_config(config: &AdapterConfig) -> Result<(), Box<dyn Error>> {
    // Validate general config
    if config.general_config.dbcfile.is_empty() {
        return Err("CAN interface not specified in configuration".into());
    }
    if config.general_config.broker_ip.is_empty() {
        return Err("Broker IP address not specified in configuration".into());
    }
    if config.general_config.broker_port.is_empty() {
        return Err("Invalid Broker port specified in configuration".into());
    }

    // Validate CAN config
    if config.can_config.can_interface.is_empty() {
        return Err("CAN interface not specified in CAN configuration".into());
    }
    if config.can_config.socket_can_type.is_empty() {
        return Err("Socket CAN type not specified in CAN configuration".into());
    }
    if config.can_config.socket_can_protocol.is_empty() {
        return Err("Socket CAN protocol not specified in CAN configurationn".into());
    }

    // Validate PID table entries
    if config.pid_table.is_empty() {
        return Err("No PID entries found in configuration".into());
    }
    let mut valid_entry_found = false;
    for (i, entry) in config.pid_table.iter().enumerate() {
        if entry.request_pid.is_empty() {
            warn!("Warning: Request PID is empty for entry {} in PID table", i);
            continue;
        }
        valid_entry_found = true;
    }
    if !valid_entry_found {
        return Err("No valid PID entries found in configuration".into());
    }

    if config.pid_table.len() == 1 {
        warn!("Warning: Only one valid PID entry found in configuration.");
    }

    Ok(())
}
