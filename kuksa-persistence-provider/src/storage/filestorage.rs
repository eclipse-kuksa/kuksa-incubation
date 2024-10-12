/********************************************************************************
* Copyright (c) 2024 Contributors to the Eclipse Foundation
*
* This program and the accompanying materials are made available under the
* terms of the Apache License 2.0 which is available at
* http://www.apache.org/licenses/LICENSE-2.0
*
* SPDX-License-Identifier: Apache-2.0
********************************************************************************/

use tinyjson::JsonValue;

use super::{Storage, StoreItem};
use std::collections::HashMap;

use std::io::Write;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};

use std::fs::File;

use log;

pub struct FileStorage {
    state: JsonValue,
    queue: Sender<StoreItem>,
}

impl Storage for FileStorage {
    fn new(config: &JsonValue) -> Self {
        match config["path"].get::<String>() {
            Some(x) => {
                log::info!("Initializing file storage on {}", x);
                let path = x.clone();
                println!("Reading storage from {}", path);
                let config_str = std::fs::read_to_string(&path).unwrap();
                let state: JsonValue = config_str.parse().unwrap();
                let mut state_copy = state.get::<HashMap<String, JsonValue>>().unwrap().clone();
                let (tx, rx): (Sender<StoreItem>, Receiver<StoreItem>) = mpsc::channel();
                let fs = FileStorage { state, queue: tx };
                std::thread::spawn(move || loop {
                    match rx.recv() {
                        Ok(msg) => {
                            log::info!("Store value: {} for path {}", msg.value, msg.path);
                            let mut val: HashMap<String, JsonValue> = HashMap::new();
                            val.insert("value".to_string(), JsonValue::String(msg.value.clone()));
                            state_copy.insert(msg.path.clone(), JsonValue::from(val));
                            let out_json: JsonValue = JsonValue::from(state_copy.to_owned());
                            let mut file = File::create(&path).unwrap();
                            match file.write_all(out_json.format().unwrap().as_bytes()) {
                                Ok(_) => {}
                                Err(e) => {
                                    log::error!("Error writing to state storage file: {:?}", e);
                                    break;
                                }
                            }
                            let _ = file.flush();
                            drop(file);
                        }
                        Err(_) => {
                            log::error!("Error receiving message");
                            break;
                        }
                    }
                });
                fs
            }
            _ => {
                log::error!("Error: file storage path is invalid");
                std::process::exit(1);
            }
        }
    }

    fn get(&self, vsspath: &str) -> Option<&str> {
        log::debug!("Try getting VSS signal {}", vsspath);
        if !self
            .state
            .get::<HashMap<String, JsonValue>>()
            .unwrap()
            .contains_key(vsspath)
        {
            return None;
        }

        let entry: Option<&HashMap<String, JsonValue>> = self.state[vsspath].get();

        if entry.is_some() && entry.unwrap().contains_key("value") {
            let value = entry.unwrap()["value"].get::<String>();

            if let Some(v) = value {
                return Some(v);
            }
            log::warn!(
                "Error reading {vsspath}, make sure all values are quoted and stored as string"
            )
        }
        None
    }

    fn set(&self, vsspath: &str, vssvalue: &str) -> Result<(), ()> {
        log::debug!("Setting VSS signal {} to {}", vsspath, vssvalue);
        self.queue
            .send(StoreItem {
                path: vsspath.to_string(),
                value: vssvalue.to_string(),
            })
            .map_err(|_| ())
    }

    fn get_queue(&self) -> Sender<StoreItem> {
        self.queue.clone()
    }
}

impl FileStorage {}
