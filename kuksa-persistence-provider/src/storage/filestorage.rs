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

use super::{FileStorageType, Storage, StorageConfig, StorageType, StoreItem};
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
    fn new(config: StorageConfig) -> Self {
        match config.storagetype {
            StorageType::FileStorageType(FileStorageType { filepath }) => {
                log::info!("Initializing file storage on {}", filepath);
                let path = filepath.clone();
                println!("Reading storage from {}", path);
                let config_str = std::fs::read_to_string(&path).unwrap();
                println!("config_str {}", config_str);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, io::Read};
    use tempfile::{tempfile, Builder};

    const FILEPATH_PREFIX: &str = "/tmp/test_storage";

    const TESTJSON: &str = r#"{
        "test_key": {
          "value": "test_value"
        }
      }"#;

    #[test]
    fn test_file_storage_new() {
        let mut storage_file = Builder::new()
            .prefix(FILEPATH_PREFIX)
            .suffix(".json")
            .tempfile()
            .unwrap();
        let filepath = storage_file.path().to_str().unwrap().to_string();

        writeln!(storage_file, "{}", TESTJSON).unwrap();

        let config = StorageConfig {
            storagetype: StorageType::FileStorageType(FileStorageType {
                filepath: filepath.clone(),
            }),
        };

        let storage = FileStorage::new(config);
        assert!(storage.get("test_key").is_some());
        assert_eq!(storage.get("test_key").unwrap(), "test_value");
    }

    #[test]
    fn test_file_storage_get() {
        let mut storage_file = Builder::new()
            .prefix(FILEPATH_PREFIX)
            .suffix(".json")
            .tempfile()
            .unwrap();
        let filepath = storage_file.path().to_str().unwrap().to_string();

        writeln!(storage_file, "{}", TESTJSON).unwrap();

        let config = StorageConfig {
            storagetype: StorageType::FileStorageType(FileStorageType {
                filepath: filepath.clone(),
            }),
        };

        let storage = FileStorage::new(config);
        assert_eq!(storage.get("test_key").unwrap(), "test_value");
        assert!(storage.get("non_existent_key").is_none());
    }

    #[test]
    fn test_file_storage_set() {
        // Create an empty temporary file
        let mut storage_file = Builder::new()
            .prefix(FILEPATH_PREFIX)
            .suffix(".json")
            .tempfile()
            .unwrap();
        let filepath = storage_file.path().to_str().unwrap().to_string();

        // let testjson = r#"{}"#;
        // writeln!(storage_file, "{}", TESTJSON).unwrap();
        storage_file.write_all(TESTJSON.as_bytes()).unwrap();

        let config = StorageConfig {
            storagetype: StorageType::FileStorageType(FileStorageType {
                filepath: filepath.clone(),
            }),
        };

        let storage = FileStorage::new(config);
        storage.set("test_key", "value-test-set").unwrap();

        // Allow some time for the background thread to process the set operation
        std::thread::sleep(std::time::Duration::from_millis(100));

        let updated_content = fs::read_to_string(filepath).unwrap();
        print!("updated_content: {}", updated_content);
        assert!(
            updated_content
                == r#"{
  "test_key": {
    "value": "value-test-set"
  }
}"#
        );
    }

    #[test]
    fn test_file_storage_invalid_path() {
        let config = StorageConfig {
            storagetype: StorageType::FileStorageType(FileStorageType {
                filepath: "/invalid/path/to/storage.json".to_string(),
            }),
        };

        let result = std::panic::catch_unwind(|| {
            FileStorage::new(config);
        });

        assert!(result.is_err());
    }
}
