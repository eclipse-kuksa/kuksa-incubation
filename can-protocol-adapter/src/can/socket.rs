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
use socketcan_isotp::{self, ExtendedId, IsoTpSocket, StandardId};
use std::error::Error as StdError;
use std::sync::{Arc, Mutex};

pub struct Socket {
    pub interface_name: String,
    #[allow(dead_code)] // 'socket_type' is intended for future socket implementation for raw-can.
    pub socket_type: String,
    pub protocol: String,
    tp_socket: Option<Arc<Mutex<IsoTpSocket>>>,
}

impl Socket {
    pub fn new(interface_name: String, socket_type: String, protocol: String) -> Self {
        Socket {
            interface_name,
            socket_type,
            protocol,
            tp_socket: None,
        }
    }

    pub fn open_socket(
        &mut self,
        rxid: u32,
        txid: u32,
        is_extended: bool,
    ) -> Result<(), Box<dyn StdError>> {
        if self.protocol == "CAN_ISOTP" {
            let socket = if is_extended {
                IsoTpSocket::open(
                    &self.interface_name,
                    ExtendedId::new(rxid).expect("Invalid rx id"),
                    ExtendedId::new(txid).expect("Invalid tx id"),
                )?
            } else {
                IsoTpSocket::open(
                    &self.interface_name,
                    StandardId::new(rxid.try_into().unwrap()).expect("Invalid rx id"),
                    StandardId::new(txid.try_into().unwrap()).expect("Invalid tx id"),
                )?
            };
            self.tp_socket = Some(Arc::new(Mutex::new(socket)));
            Ok(())
        } else {
            Err("Invalid protocol".into())
        }
    }

    pub fn set_nonblocking(&mut self) -> Result<(), Box<dyn StdError>> {
        if let Some(tp_socket) = &self.tp_socket {
            let socket_lock = tp_socket.lock().unwrap();
            socket_lock.set_nonblocking(true)?; // Enable non-blocking mode
            Ok(())
        } else {
            Err("Socket not opened".into())
        }
    }

    pub fn read_socket(&mut self) -> Result<Vec<u8>, Box<dyn StdError>> {
        if let Some(tp_socket) = &self.tp_socket {
            let mut socket = tp_socket.lock().unwrap();
            let buffer = socket.read()?;
            Ok(buffer.to_vec())
        } else {
            Err("Socket not opened".into())
        }
    }

    pub fn write_socket(&mut self, data: &[u8]) -> Result<(), Box<dyn StdError>> {
        if let Some(tp_socket) = &self.tp_socket {
            let socket = tp_socket.lock().unwrap();
            socket.write(data)?;
            Ok(())
        } else {
            Err("Socket not opened".into())
        }
    }
}

#[cfg(test)]
#[test]
fn test_open_socket_success_isotp() -> Result<(), Box<dyn StdError>> {
    let mut socket = Socket::new(
        "vcan0".to_string(),
        "socket_type".to_string(),
        "CAN_ISOTP".to_string(),
    );
    assert!(socket.open_socket(0x123, 0x456).is_ok());
    assert!(socket.tp_socket.is_some());
    Ok(())
}
#[test]
fn test_open_socket_invalid_protocol() {
    let mut socket = Socket::new(
        "vcan0".to_string(),
        "socket_type".to_string(),
        "INVALID_PROTOCOL".to_string(),
    );
    assert!(socket.open_socket(0x123, 0x456).is_err());
    assert!(socket.tp_socket.is_none());
}
#[test]
fn test_write_socket_not_opened() {
    let mut socket = Socket::new(
        "vcan0".to_string(),
        "socketcan".to_string(),
        "CAN_ISOTP".to_string(),
    );

    let test_data = vec![0x01, 0x02, 0x03, 0x04];
    let result = socket.write_socket(&test_data);
    assert!(result.is_err());
}

#[test]
fn test_read_socket_not_opened() {
    let mut socket = Socket::new(
        "vcan0".to_string(),
        "socketcan".to_string(),
        "CAN_ISOTP".to_string(),
    );
    let result = socket.read_socket();
    assert!(result.is_err());
}
