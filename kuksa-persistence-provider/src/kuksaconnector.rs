/********************************************************************************
* Copyright (c) 2024 Contributors to the Eclipse Foundation
*
* This program and the accompanying materials are made available under the
* terms of the Apache License 2.0 which is available at
* http://www.apache.org/licenses/LICENSE-2.0
*
* SPDX-License-Identifier: Apache-2.0
********************************************************************************/

use crate::storage::{self, StoreItem};

use std::collections::HashMap;
use std::fmt;
use std::time::SystemTime;

use kuksa::proto;

use std::sync::mpsc::Sender;

use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct ParseError {}

pub fn create_kuksa_client(uri: &str) -> Arc<Mutex<kuksa::Client>> {
    log::info!("Creating Kuksa Databroker client for URI: {}", uri);
    let uri = kuksa::Uri::try_from(uri).expect("Invalid URI  for Kuksa Databroker connection.");
    Arc::new(Mutex::new(kuksa::Client::new(uri)))
}

pub async fn get_from_storage_and_set_values(
    storage: &impl storage::Storage,
    kuksa_client: &Arc<Mutex<kuksa::Client>>,
    vsspaths: &Vec<String>,
) {
    for vsspath in vsspaths {
        get_from_storage_and_set(storage, kuksa_client, vsspath, false).await;
    }
}

pub async fn get_from_storage_and_set_actuations(
    storage: &impl storage::Storage,
    kuksa_client: &Arc<Mutex<kuksa::Client>>,
    vsspaths: &Vec<String>,
) {
    for vsspath in vsspaths {
        get_from_storage_and_set(storage, kuksa_client, vsspath, true).await;
    }
}

pub async fn get_from_storage_and_set(
    storage: &impl storage::Storage,
    kuksa_client: &Arc<Mutex<kuksa::Client>>,
    vsspath: &str,
    is_actuator: bool,
) {
    log::debug!("Query storage for VSS signal: {}", vsspath);
    let value = match storage.get(vsspath) {
        Some(x) => x,
        None => {
            log::warn!("No value for VSS signal: {} stored", vsspath);
            return;
        }
    };

    //Figure out metadata:
    let datapoint_entries = match kuksa_client
        .lock()
        .unwrap()
        .get_metadata(vec![vsspath])
        .await
    {
        Ok(data_entries) => Some(data_entries),
        Err(kuksa::Error::Status(status)) => {
            log::warn!(
                "Error: Could not get metadata for VSS signal: {}, Status: {}",
                vsspath,
                &status
            );
            None
        }
        Err(kuksa::Error::Connection(msg)) => {
            log::warn!(
                "Connection Error: Could not get metadata for VSS signal: {}, Reason: {}",
                vsspath,
                &msg
            );
            None
        }
        Err(kuksa::Error::Function(msg)) => {
            log::warn!(
                "Error: Could not get metadata for VSS signal: {}, Errors: {msg:?}",
                vsspath
            );
            None
        }
    };

    if datapoint_entries.is_none() {
        return;
    }

    /* We can only have one match, as we query only one path (user entering branch
     * in config is considered dumb) */
    if let Some(entries) = datapoint_entries {
        if let Some(metadata) = &entries.first().unwrap().metadata {
            let data_value = try_into_data_value(
                value,
                proto::v1::DataType::from_i32(metadata.data_type).unwrap(),
            );
            if data_value.is_err() {
                log::warn!(
                    "Could not parse \"{}\" as {:?}",
                    value,
                    proto::v1::DataType::from_i32(metadata.data_type).unwrap()
                );
                return;
            }

            let ts = prost_types::Timestamp::from(SystemTime::now());
            let datapoints = HashMap::from([(
                vsspath.to_string().clone(),
                proto::v1::Datapoint {
                    timestamp: Some(ts),
                    value: Some(data_value.unwrap()),
                },
            )]);

            let result = {
                if is_actuator {
                    kuksa_client
                        .lock()
                        .unwrap()
                        .set_target_values(datapoints)
                        .await
                } else {
                    kuksa_client
                        .lock()
                        .unwrap()
                        .set_current_values(datapoints)
                        .await
                }
            };

            match result {
                Ok(_) => {
                    log::debug!("Succes setting {} to {}", vsspath, value);
                }
                Err(kuksa::Error::Status(status)) => {
                    log::warn!(
                        "Error: Could not set value for VSS signal: {}, Status: {}",
                        vsspath,
                        &status
                    );
                }
                Err(kuksa::Error::Connection(msg)) => {
                    log::warn!(
                        "Connection Error: Could not set value for VSS signal: {}, Reason: {}",
                        vsspath,
                        &msg
                    );
                }
                Err(kuksa::Error::Function(msg)) => {
                    log::warn!(
                        "Error: Could not set value for VSS signal: {}, Errors: {msg:?}",
                        vsspath
                    );
                }
            };
        }
    }
}

pub async fn watch_values(
    storage_queue: Sender<storage::StoreItem>,
    kuksa_client: &Arc<Mutex<kuksa::Client>>,
    vsspaths: Vec<&str>,
    is_actuator: bool,
) {
    log::info!(
        "Subscribing to  {} for VSS signals: {:?}",
        {
            match is_actuator {
                true => "actuators",
                false => "current values",
            }
        },
        &vsspaths
    );

    let res = match is_actuator {
        true => {
            kuksa_client
                .lock()
                .unwrap()
                .subscribe_target_values(vsspaths)
                .await
        }
        false => {
            kuksa_client
                .lock()
                .unwrap()
                .subscribe_current_values(vsspaths)
                .await
        }
    };
    match res {
        Ok(mut subs) => {
            tokio::spawn(async move {
                loop {
                    match subs.message().await {
                        Ok(resp) => {
                            if let Some(r) = resp {
                                for update in r.updates {
                                    if let Some(entry) = update.entry {
                                        let newdp = match is_actuator {
                                            true => entry.actuator_target,
                                            false => entry.value,
                                        };
                                        if let Some(datapoint) = newdp {
                                            let data = DisplayDatapoint(datapoint);
                                            log::info!(
                                                "Received value {} for VSS signal {}",
                                                data.to_string(),
                                                entry.path
                                            );

                                            match storage_queue.send(StoreItem {
                                                path: entry.path.clone(),
                                                value: data.to_string(),
                                            }) {
                                                Ok(_) => {}
                                                Err(err) => {
                                                    log::warn!(
                                                        "Error sending data to storage {:?}",
                                                        err
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            log::warn!("Error: Could not receive message: {:?}", err);
                            break;
                        }
                    }
                }
            });
        }
        Err(err) => {
            log::warn!("Error: Could not subscribe to VSS signals: {:?}", err);
        }
    }
}

/* Donation from databroker-cli */
fn try_into_data_value(
    input: &str,
    data_type: proto::v1::DataType,
) -> Result<proto::v1::datapoint::Value, ParseError> {
    if input == "NotAvailable" {
        return Ok(proto::v1::datapoint::Value::String(input.to_string()));
    }

    match data_type {
        proto::v1::DataType::String => Ok(proto::v1::datapoint::Value::String(input.to_owned())),
        proto::v1::DataType::StringArray => match get_array_from_input(input.to_owned()) {
            Ok(value) => Ok(proto::v1::datapoint::Value::StringArray(
                proto::v1::StringArray { values: value },
            )),
            Err(err) => Err(err),
        },
        proto::v1::DataType::Boolean => match input.parse::<bool>() {
            Ok(value) => Ok(proto::v1::datapoint::Value::Bool(value)),
            Err(_) => Err(ParseError {}),
        },
        proto::v1::DataType::BooleanArray => match get_array_from_input(input.to_owned()) {
            Ok(value) => Ok(proto::v1::datapoint::Value::BoolArray(
                proto::v1::BoolArray { values: value },
            )),
            Err(err) => Err(err),
        },
        proto::v1::DataType::Int8 => match input.parse::<i8>() {
            Ok(value) => Ok(proto::v1::datapoint::Value::Int32(value as i32)),
            Err(_) => Err(ParseError {}),
        },
        proto::v1::DataType::Int8Array => match get_array_from_input(input.to_owned()) {
            Ok(value) => Ok(proto::v1::datapoint::Value::Int32Array(
                proto::v1::Int32Array { values: value },
            )),
            Err(err) => Err(err),
        },
        proto::v1::DataType::Int16 => match input.parse::<i16>() {
            Ok(value) => Ok(proto::v1::datapoint::Value::Int32(value as i32)),
            Err(_) => Err(ParseError {}),
        },
        proto::v1::DataType::Int16Array => match get_array_from_input(input.to_owned()) {
            Ok(value) => Ok(proto::v1::datapoint::Value::Int32Array(
                proto::v1::Int32Array { values: value },
            )),
            Err(err) => Err(err),
        },
        proto::v1::DataType::Int32 => match input.parse::<i32>() {
            Ok(value) => Ok(proto::v1::datapoint::Value::Int32(value)),
            Err(_) => Err(ParseError {}),
        },
        proto::v1::DataType::Int32Array => match get_array_from_input(input.to_owned()) {
            Ok(value) => Ok(proto::v1::datapoint::Value::Int32Array(
                proto::v1::Int32Array { values: value },
            )),
            Err(err) => Err(err),
        },
        proto::v1::DataType::Int64 => match input.parse::<i64>() {
            Ok(value) => Ok(proto::v1::datapoint::Value::Int64(value)),
            Err(_) => Err(ParseError {}),
        },
        proto::v1::DataType::Int64Array => match get_array_from_input(input.to_owned()) {
            Ok(value) => Ok(proto::v1::datapoint::Value::Int64Array(
                proto::v1::Int64Array { values: value },
            )),
            Err(err) => Err(err),
        },
        proto::v1::DataType::Uint8 => match input.parse::<u8>() {
            Ok(value) => Ok(proto::v1::datapoint::Value::Uint32(value as u32)),
            Err(_) => Err(ParseError {}),
        },
        proto::v1::DataType::Uint8Array => match get_array_from_input(input.to_owned()) {
            Ok(value) => Ok(proto::v1::datapoint::Value::Uint32Array(
                proto::v1::Uint32Array { values: value },
            )),
            Err(err) => Err(err),
        },
        proto::v1::DataType::Uint16 => match input.parse::<u16>() {
            Ok(value) => Ok(proto::v1::datapoint::Value::Uint32(value as u32)),
            Err(_) => Err(ParseError {}),
        },
        proto::v1::DataType::Uint16Array => match get_array_from_input(input.to_owned()) {
            Ok(value) => Ok(proto::v1::datapoint::Value::Uint32Array(
                proto::v1::Uint32Array { values: value },
            )),
            Err(err) => Err(err),
        },
        proto::v1::DataType::Uint32 => match input.parse::<u32>() {
            Ok(value) => Ok(proto::v1::datapoint::Value::Uint32(value)),
            Err(_) => Err(ParseError {}),
        },
        proto::v1::DataType::Uint32Array => match get_array_from_input(input.to_owned()) {
            Ok(value) => Ok(proto::v1::datapoint::Value::Uint32Array(
                proto::v1::Uint32Array { values: value },
            )),
            Err(err) => Err(err),
        },
        proto::v1::DataType::Uint64 => match input.parse::<u64>() {
            Ok(value) => Ok(proto::v1::datapoint::Value::Uint64(value)),
            Err(_) => Err(ParseError {}),
        },
        proto::v1::DataType::Uint64Array => match get_array_from_input(input.to_owned()) {
            Ok(value) => Ok(proto::v1::datapoint::Value::Uint64Array(
                proto::v1::Uint64Array { values: value },
            )),
            Err(err) => Err(err),
        },
        proto::v1::DataType::Float => match input.parse::<f32>() {
            Ok(value) => Ok(proto::v1::datapoint::Value::Float(value)),
            Err(_) => Err(ParseError {}),
        },
        proto::v1::DataType::FloatArray => match get_array_from_input(input.to_owned()) {
            Ok(value) => Ok(proto::v1::datapoint::Value::FloatArray(
                proto::v1::FloatArray { values: value },
            )),
            Err(err) => Err(err),
        },
        proto::v1::DataType::Double => match input.parse::<f64>() {
            Ok(value) => Ok(proto::v1::datapoint::Value::Double(value)),
            Err(_) => Err(ParseError {}),
        },
        proto::v1::DataType::DoubleArray => match get_array_from_input(input.to_owned()) {
            Ok(value) => Ok(proto::v1::datapoint::Value::DoubleArray(
                proto::v1::DoubleArray { values: value },
            )),
            Err(err) => Err(err),
        },
        _ => Err(ParseError {}),
    }
}

pub fn get_array_from_input<T: std::str::FromStr>(values: String) -> Result<Vec<T>, ParseError> {
    let raw_input = values
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .ok_or(ParseError {})?;

    let pattern = r#"(?:\\.|[^",])*"(?:\\.|[^"])*"|[^",]+"#;

    let regex = regex::Regex::new(pattern).unwrap();
    let inputs = regex.captures_iter(raw_input);

    let mut array: Vec<T> = vec![];
    for part in inputs {
        match part[0]
            .trim()
            .replace('\"', "")
            .replace('\\', "\"")
            .parse::<T>()
        {
            Ok(value) => array.push(value),
            Err(_) => return Err(ParseError {}),
        }
    }
    Ok(array)
}

struct DisplayDatapoint(proto::v1::Datapoint);

fn display_array<T>(f: &mut fmt::Formatter<'_>, array: &[T]) -> fmt::Result
where
    T: fmt::Display,
{
    f.write_str("[")?;
    let real_delimiter = ", ";
    let mut delimiter = "";
    for value in array {
        write!(f, "{delimiter}")?;
        delimiter = real_delimiter;
        write!(f, "{value}")?;
    }
    f.write_str("]")
}

impl fmt::Display for DisplayDatapoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0.value {
            Some(value) => match value {
                proto::v1::datapoint::Value::Bool(value) => f.pad(&format!("{value}")),
                proto::v1::datapoint::Value::Int32(value) => f.pad(&format!("{value}")),
                proto::v1::datapoint::Value::Int64(value) => f.pad(&format!("{value}")),
                proto::v1::datapoint::Value::Uint32(value) => f.pad(&format!("{value}")),
                proto::v1::datapoint::Value::Uint64(value) => f.pad(&format!("{value}")),
                proto::v1::datapoint::Value::Float(value) => f.pad(&format!("{value:.2}")),
                proto::v1::datapoint::Value::Double(value) => f.pad(&format!("{value}")),
                proto::v1::datapoint::Value::String(value) => f.pad(&value.to_owned()),
                proto::v1::datapoint::Value::StringArray(array) => display_array(f, &array.values),
                proto::v1::datapoint::Value::BoolArray(array) => display_array(f, &array.values),
                proto::v1::datapoint::Value::Int32Array(array) => display_array(f, &array.values),
                proto::v1::datapoint::Value::Int64Array(array) => display_array(f, &array.values),
                proto::v1::datapoint::Value::Uint32Array(array) => display_array(f, &array.values),
                proto::v1::datapoint::Value::Uint64Array(array) => display_array(f, &array.values),
                proto::v1::datapoint::Value::FloatArray(array) => display_array(f, &array.values),
                proto::v1::datapoint::Value::DoubleArray(array) => display_array(f, &array.values),
            },
            None => f.pad("None"),
        }
    }
}
