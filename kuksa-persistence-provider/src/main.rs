/********************************************************************************
* Copyright (c) 2024 Contributors to the Eclipse Foundation
*
* This program and the accompanying materials are made available under the
* terms of the Apache License 2.0 which is available at
* http://www.apache.org/licenses/LICENSE-2.0
*
* SPDX-License-Identifier: Apache-2.0
********************************************************************************/

mod kuksaconnector;
mod storage;

use storage::Storage;

use clap::Parser;
use std::collections::HashMap;
use std::{env, path::PathBuf};

// Use one of two JSON libraries
#[cfg(all(feature = "json_tinyjson", feature = "json_djson"))]
compile_error!("feature \"json_tinyjson\" and feature \"json_djson\" cannot be enabled at the same time");

#[cfg(feature = "json_tinyjson")]
use tinyjson::JsonValue;


#[cfg(feature = "json_djson")]
use djson::Value as JsonValue;


use tokio::signal::ctrl_c;

#[derive(Debug, Clone, PartialEq)]
enum ConfigValue {
    Number(f64),
    Boolean(bool),
    String(String),
    Null,
    Array(Vec<ConfigValue>),
    Object(HashMap<String, ConfigValue>),
}

// Convert from JSON value (depending on the feature) to ConfigValue
impl From<JsonValue> for ConfigValue {
    fn from(value: JsonValue) -> Self {
        match value {
            JsonValue::Null => ConfigValue::Null,
            JsonValue::Boolean(b) => ConfigValue::Boolean(b),
            JsonValue::Number(n) => ConfigValue::Number(n),
            JsonValue::String(s) => ConfigValue::String(s),
            JsonValue::Array(a) => ConfigValue::Array(a.into_iter().map(Into::into).collect()),
            JsonValue::Object(o) => {
                ConfigValue::Object(o.into_iter().map(|(k, v)| (k, v.into())).collect())
            }
        }
    }
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct CmdLine {
    /// JSON file containing the configuration
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,
}

#[tokio::main]
async fn main() {
    // Initialize logger
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info")
    }
    env_logger::init();

    let args = CmdLine::parse();

    let config_path = args.config.unwrap_or_else(|| PathBuf::from("config.json"));

    // Reading configuration file into a string
    log::info!("Reading configuration from: {}", config_path.display());
    let config_str = match std::fs::read_to_string(&config_path) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Error reading configuration file: {:?}", e);
            std::process::exit(1);
        }
    };

    log::debug!("Configuration file content: {}", config_str);

    #[cfg(feature = "json_tinyjson")]
    let parsed_cfg: ConfigValue  = match config_str.parse::<tinyjson::JsonValue>() {
        Ok(p) => p.into(),
        Err(e) => {
            log::error!("Error parsing JSON data structure: {:?}", e);
            std::process::exit(1);
        }
    };


    #[cfg(feature = "json_djson")]
    let parsed_cfg: ConfigValue = match djson::from_reader(config_str.as_bytes()) {
        Ok(p) => p.into(),
        Err(e) => {
            log::error!("Error parsing JSON data structure: {:?}", e);
            std::process::exit(1);
        }
    };

    let parsed_cfg_hash = match parsed_cfg {
        ConfigValue::Object(o) => o,
        _ => {
            log::error!("Error: JSON config data structure is wrong");
            std::process::exit(1);
        }
    };

    log::debug!("Parsed JSON data structure: {:?}", parsed_cfg_hash);


    let parsed_storage_type: storage::StorageType = match parsed_cfg_hash["state-storage"] {
        ConfigValue::Object(ref parsed_storage_config) => {
                match parsed_storage_config.get("type").expect("state-storage type configuration missing") {
                    // For FileStorageType
                    ConfigValue::String( s) if *s == "file".to_string() => {
                        // FileStorageType needs filepath otherwise default is
                        match parsed_storage_config.get("path"){
                            Some(ConfigValue::String( path)) => {
                                storage::StorageType::FileStorageType(storage::FileStorageType {filepath: path.clone()})
                            },
                            None => {
                                log::info!("state storage path is not existent, using default storage path");
                                storage::StorageType::FileStorageType(storage::FileStorageType::default())
                            }
                            _ => {
                                log::error!("Error: state storage path is invalid");
                                std::process::exit(1);
                            }
                        }

                    },
                    // Undefined storage types
                    _ => {
                        log::error!("Error: state storage type is invalid");
                        std::process::exit(1);
                    }
                }
            }
        _ => {
            log::info!("no state storage configuration found, using default");
            storage::StorageType::default()
        }
    };

    
    let storage = storage::FileStorage::new(storage::StorageConfig {
        storagetype: parsed_storage_type,
    });

    let mut restore_current_values: Vec<String> = vec![];
    let mut restore_actuation_values: Vec<String> = vec![];
    let mut watch_current_values: Vec<String> = vec![];
    let mut watch_actuation_values: Vec<String> = vec![];

    match parsed_cfg_hash.get("restore-only") {
        Some(ConfigValue::Object(section)) => {

            match section.get("values") {
                Some(ConfigValue::Array(elements)) => {
                    for path in elements {
                        match path {
                            ConfigValue::String(vsspath) => {
                                restore_current_values.push(vsspath.clone());
                            }
                            _ => {
                                log::info!("invalid restore-only value found");
                            }
                        }
                    }
                }
                _ => {
                    log::info!("no restore-only values found");
                }
            }

            match section.get("actuators"){
                Some(ConfigValue::Array(elements)) => {
                    for path in elements {
                        // restore_actuation_values.push(path.get::<String>().unwrap().to_string());
                        match path {
                            ConfigValue::String(vsspath) => {
                                restore_actuation_values.push(vsspath.clone());
                            }
                            _ => {
                                log::info!("invalid restore-only actuator found");
                            }
                        }
                    }
                }
                _ => {
                    log::info!("no restore-only actuators found");
                }
            }
        }
        None => {
            log::info!("no restore-only configuration found");
        }
        _ => {
            log::info!("invalid restore-only configuration found");
        }
    }


    match parsed_cfg_hash.get("restore-and-watch") {
        Some(ConfigValue::Object(section)) => {

            match section.get("values") {
                Some(ConfigValue::Array(elements)) => {
                    for path in elements {
                        match path {
                            ConfigValue::String(vsspath) => {
                                restore_current_values.push(vsspath.clone());
                                watch_current_values.push(vsspath.clone());
                            }
                            _ => {
                                log::info!("invalid restore-only value found");
                            }
                        }
                    }
                }
                _ => {
                    log::info!("no restore-only values found");
                }
            }

            match section.get("actuators"){
                Some(ConfigValue::Array(elements)) => {
                    for path in elements {
                        // restore_actuation_values.push(path.get::<String>().unwrap().to_string());
                        match path {
                            ConfigValue::String(vsspath) => {
                                restore_actuation_values.push(vsspath.clone());
                                watch_actuation_values.push(vsspath.clone());
                            }
                            _ => {
                                log::info!("invalid restore-only actuator found");
                            }
                        }
                    }
                }
                _ => {
                    log::info!("no restore-only actuators found");
                }
            }
        }
        None => {
            log::info!("no restore-only configuration found");
        }
        _ => {
            log::info!("invalid restore-only configuration found");
        }
    }


    // Each subscription needs a separate client
    let kuksa_client = kuksaconnector::create_kuksa_client("grpc://127.0.01:55556");
    let kuksa_client2 = kuksaconnector::create_kuksa_client("grpc://127.0.01:55556");

    kuksaconnector::get_from_storage_and_set_values(
        &storage,
        &kuksa_client,
        &restore_current_values,
    )
    .await;
    kuksaconnector::get_from_storage_and_set_actuations(
        &storage,
        &kuksa_client,
        &restore_actuation_values,
    )
    .await;

    drop(restore_actuation_values);
    drop(restore_current_values);

    kuksaconnector::watch_values(
        storage.get_queue(),
        &kuksa_client,
        watch_current_values.iter().map(|s| &**s).collect(),
        false,
    )
    .await;
    kuksaconnector::watch_values(
        storage.get_queue(),
        &kuksa_client2,
        watch_actuation_values.iter().map(|s| &**s).collect(),
        true,
    )
    .await;

    tokio::select! {
        _ = ctrl_c() => {
            println!("Received Ctrl+C, exiting.");
            return;
        }
    }
}
