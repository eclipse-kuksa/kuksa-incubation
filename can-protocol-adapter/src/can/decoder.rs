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
use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use can_dbc::{Signal, DBC};
use log::{max_level, trace, LevelFilter};
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::mem;

pub struct Decoder {
    signals: HashMap<String, Signal>,
}

impl Decoder {
    pub fn new(dbc_file_path: &str) -> Result<Self, String> {
        let mut f = File::open(dbc_file_path).map_err(|e| format!("Failed to open file: {}", e))?;
        let mut buffer = Vec::new();
        f.read_to_end(&mut buffer)
            .map_err(|e| format!("Failed to read file: {}", e))?;
        let dbc =
            DBC::from_slice(&buffer).map_err(|e| format!("Failed to parse DBC file: {:?}", e))?;

        let mut signals = HashMap::new();
        for message in dbc.messages() {
            for signal in message.signals() {
                signals.insert(signal.name().to_string(), signal.clone());
            }
        }

        if max_level() <= LevelFilter::Trace {
            for message in dbc.messages() {
                for signal in message.signals() {
                    trace!(
                        "Parsed signal: {} start_bit: {}, size: {}, factor: {}, offset: {}",
                        signal.name(),
                        signal.start_bit(),
                        signal.signal_size(),
                        signal.factor(),
                        signal.offset()
                    );
                }
            }
        }
        Ok(Self { signals })
    }

    pub fn decode_message_by_name(&self, signal_name: &str, msg: Vec<u8>) -> Result<f64, String> {
        let signal = self
            .signals
            .get(signal_name)
            .ok_or_else(|| format!("Signal '{}' not found", signal_name))?;

        let mut padded_msg = msg.clone();

        // Pad with zeros to ensure 8 bytes
        padded_msg.resize(8, 0);

        let msg64: u64 = match signal.byte_order() {
            can_dbc::ByteOrder::BigEndian => (&padded_msg[..])
                .read_u64::<BigEndian>()
                .map_err(|e| e.to_string())?,
            can_dbc::ByteOrder::LittleEndian => (&padded_msg[..])
                .read_u64::<LittleEndian>()
                .map_err(|e| e.to_string())?,
        };

        trace!("Signal: {}", signal_name);
        trace!("Start Bit: {}", signal.start_bit());
        trace!("Signal Size: {}", signal.signal_size());
        trace!("Byte Order: {:?}", signal.byte_order());
        trace!("Factor: {}", signal.factor());
        trace!("Offset: {}", signal.offset());
        trace!("CAN Rx: Data:{:X?}", msg);
        trace!("Message (msg64): {:X}", msg64);

        let data: u64;
        match signal.byte_order() {
            can_dbc::ByteOrder::BigEndian => {
                let u64_size_in_bits = mem::size_of::<u64>() * 8;
                let shifted_value = msg64
                    >> (u64_size_in_bits as u64
                        - ((signal.start_bit() + 1) + signal.signal_size()));
                let bit_mask: u64 = (1 << signal.signal_size()) - 1;
                data = shifted_value & bit_mask;
                trace!(
                    "shifted_value: {:X?} bit_mask: {:X?}",
                    shifted_value,
                    bit_mask
                );
            }
            can_dbc::ByteOrder::LittleEndian => {
                let shifted_value = msg64 >> (signal.start_bit + 1);
                let bit_mask = (1 << signal.signal_size()) - 1;
                data = shifted_value & bit_mask;
                trace!(
                    "shifted_value: {:X?} bit_mask: {:X?}",
                    shifted_value,
                    bit_mask
                );
            }
        };
        let result: f64 = (data as f64) * signal.factor() + signal.offset();
        Ok(result)
    }
}
