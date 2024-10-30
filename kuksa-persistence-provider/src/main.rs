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
use std::fmt::Debug;
use std::{env, path::PathBuf};
use anyhow::{Result, anyhow};

// Ensure no unsafe code is used
#[forbid(unsafe_code)]

// Use one of two JSON libraries
#[cfg(all(feature = "json_tinyjson", feature = "json_djson"))]
compile_error!(
    "feature \"json_tinyjson\" and feature \"json_djson\" cannot be enabled at the same time"
);

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

// Parse the configuration file to a ConfigValue
// The configuration file is expected to be in JSON format

fn parse_cfgfile(path: &PathBuf) -> Result<ConfigValue> {
    let config_str = std::fs::read_to_string(&path)?;

    log::debug!("Configuration file content: {}", config_str);

    #[cfg(feature = "json_tinyjson")]
    let parsed_cfg: ConfigValue = config_str.parse::<tinyjson::JsonValue>()?.into();

    #[cfg(feature = "json_djson")]
    let parsed_cfg: ConfigValue = djson::from_reader(config_str.as_bytes())?;

    Ok(parsed_cfg)
}

// Create a storage instance from the configuration
fn create_storage_from_cfg(parsed_cfg: &ConfigValue) -> Result<impl storage::Storage> {
    let parsed_cfg_hash = match parsed_cfg {
        ConfigValue::Object(o) => o,
        _ => {
            return Err(anyhow!("Error: JSON config data structure is wrong"));
        }
    };

    log::debug!("Parsed JSON data structure: {:?}", parsed_cfg_hash);

    let parsed_storage_type: storage::StorageType = match parsed_cfg_hash["state-storage"] {
        ConfigValue::Object(ref parsed_storage_config) => {
            match parsed_storage_config
                .get("type")
            {
                // For FileStorageType
                Some(ConfigValue::String(s)) if *s == "file".to_string() => {
                    // FileStorageType needs filepath otherwise default is
                    match parsed_storage_config.get("path") {
                        Some(ConfigValue::String(path)) => {
                            storage::StorageType::FileStorageType(storage::FileStorageType {
                                filepath: path.clone(),
                            })
                        }
                        None => {
                            log::info!(
                                "state storage path is not existent, using default storage path"
                            );
                            storage::StorageType::FileStorageType(
                                storage::FileStorageType::default(),
                            )
                        }
                        _ => {
                            log::error!("Error: state storage path is invalid");
                            std::process::exit(1);
                        }
                    }
                }
                None => {
                    log::info!("no state storage type found, using default");
                    storage::StorageType::default()
                }
                // Undefined storage types
                _ => {
                    return Err(anyhow!("Error: state storage type is invalid"));
                }
            }
        }
        _ => {
            log::info!("no state storage configuration found, using default");
            storage::StorageType::default()
        }
    };

    Ok(storage::FileStorage::new(storage::StorageConfig {
        storagetype: parsed_storage_type,
    }))
}

fn collect_vss_paths(
    config: &ConfigValue,
    section_name: &str,
    element_name: &str,
    str_arrays: &mut [&mut Vec<String>],
) {
    let parsed_cfg_hash = match config {
        ConfigValue::Object(o) => o,
        _ => {
            log::error!("Error: JSON config data structure is wrong");
            std::process::exit(1);
        }
    };
    // Find section in parsed config
    match parsed_cfg_hash.get(section_name) {
        Some(ConfigValue::Object(section)) => {
            match section.get(element_name) {
                Some(ConfigValue::Array(elements)) => {
                    for path in elements {
                        match path {
                            ConfigValue::String(vsspath) => {
                                // restore_current_values.push(vsspath.clone());
                                for str_array in str_arrays.iter_mut() {
                                    str_array.push(vsspath.clone());
                                }
                            }
                            _ => {
                                log::info!("invalid {} {} found", section_name, element_name);
                            }
                        }
                    }
                }
                _ => {
                    log::info!("no {} {} found", section_name, element_name);
                }
            }
        }
        None => {
            log::info!("no {} configuration found", section_name);
        }
        _ => {
            log::info!("invalid {} configuration found", section_name);
        }
    }
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

    let parsed_cfg = match parse_cfgfile(&config_path)  {
        Ok(cfg) => cfg,
        Err(e) => {
            log::error!("Error parsing configuration file: {}", e);
            std::process::exit(1);
        }
    };
    let storage = match create_storage_from_cfg(&parsed_cfg) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Error creating storage: {}", e);
            std::process::exit(1);
        }
    };

    let mut restore_current_values: Vec<String> = vec![];
    let mut restore_actuation_values: Vec<String> = vec![];
    let mut watch_current_values: Vec<String> = vec![];
    let mut watch_actuation_values: Vec<String> = vec![];

    collect_vss_paths(
        &parsed_cfg,
        "restore-only",
        "values",
        &mut [&mut restore_current_values],
    );
    collect_vss_paths(
        &parsed_cfg,
        "restore-only",
        "actuators",
        &mut [&mut restore_actuation_values],
    );

    collect_vss_paths(
        &parsed_cfg,
        "restore-and-watch",
        "values",
        &mut [&mut restore_current_values, &mut watch_current_values],
    );
    collect_vss_paths(
        &parsed_cfg,
        "restore-and-watch",
        "actuators",
        &mut [&mut restore_actuation_values, &mut watch_actuation_values],
    );

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempdir::TempDir;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_parse_cfgfile_valid_json_simple() {
        let dir = TempDir::new("test").unwrap();
        let file_path = dir.path().join("config.json");
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, r#"{{"key": "value"}}"#).unwrap();

        let config = parse_cfgfile(&file_path).unwrap();
        match config {
            ConfigValue::Object(ref obj) => {
                assert_eq!(obj.get("key"), Some(&ConfigValue::String("value".to_string())));
            }
            _ => panic!("Parsed config is not an object"),
        }
    }

    #[test]
    fn test_parse_cfgfile_valid_cfg() {
        let dir = TempDir::new("test").unwrap();
        let file_path = dir.path().join("config.json");
        let mut file = File::create(&file_path).unwrap();
        let test_json_string = r#"
            {
                "restore-only": {
                    "values": [
                        "Vehicle.VehicleIdentification.VIN",
                        "Vehicle.VehicleIdentification.VehicleInteriorColor"
                    ],
                    "actuators": [
                        "Vehicle.Cabin.Infotainment.HMI.TemperatureUnit"
                    ]
                }
            }
        "#;

        writeln!(file, "{}", test_json_string).unwrap();
        let config = parse_cfgfile(&file_path).unwrap();
        match config {
            ConfigValue::Object(ref obj) => {
                assert_eq!(obj.get("restore-only"), Some(&ConfigValue::Object({
                    let mut map = HashMap::new();
                    map.insert("values".to_string(), ConfigValue::Array(vec![
                        ConfigValue::String("Vehicle.VehicleIdentification.VIN".to_string()),
                        ConfigValue::String("Vehicle.VehicleIdentification.VehicleInteriorColor".to_string()),
                    ]));
                    map.insert("actuators".to_string(), ConfigValue::Array(vec![
                        ConfigValue::String("Vehicle.Cabin.Infotainment.HMI.TemperatureUnit".to_string()),
                    ]));
                    map
                })));
            }
            _ => panic!("Parsed config is not an object"),
        }
    }


    #[test]
    fn test_parse_cfgfile_invalid_json_1() {
        let dir = TempDir::new("test").unwrap();
        let file_path = dir.path().join("config.json");
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, r#"{{"key" "value"}}"#).unwrap(); // Invalid JSON

        let result = parse_cfgfile(&file_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_cfgfile_invalid_json_unexpeof() {
        let dir = TempDir::new("test").unwrap();
        let file_path = dir.path().join("config.json");
        let mut file = File::create(&file_path).unwrap();
        let test_json_string = r#"{{"key" "value"}"#;
        writeln!(file, "{}", test_json_string).unwrap();

        let result = parse_cfgfile(&file_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_storage_from_cfg_nofile() {
        let dir = TempDir::new("test").unwrap();
        let file_path = dir.path().join("config.json");
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, r#"{{"key": "value"}}"#).unwrap(); // Invalid JSON
        let mut config = HashMap::new();
        let mut storage_config = HashMap::new();
        storage_config.insert("type".to_string(), ConfigValue::String("file".to_string()));
        storage_config.insert("path".to_string(), ConfigValue::String(file_path.to_str().unwrap().to_string()));
        config.insert("state-storage".to_string(), ConfigValue::Object(storage_config));

        let storage = create_storage_from_cfg(&ConfigValue::Object(config));
        assert!(storage.is_ok());
    }



    #[test]
    fn test_collect_vss_paths() {
        let mut config = HashMap::new();
        let mut section = HashMap::new();
        section.insert("values".to_string(), ConfigValue::Array(vec![ConfigValue::String("path1".to_string()), ConfigValue::String("path2".to_string())]));
        config.insert("restore-only".to_string(), ConfigValue::Object(section));

        let mut restore_current_values: Vec<String> = vec![];
        collect_vss_paths(&ConfigValue::Object(config), "restore-only", "values", &mut [&mut restore_current_values]);

        assert_eq!(restore_current_values, vec!["path1".to_string(), "path2".to_string()]);
    }
}