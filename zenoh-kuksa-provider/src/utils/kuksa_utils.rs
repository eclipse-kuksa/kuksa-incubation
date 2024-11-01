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

use kuksa::proto::v1::{datapoint::Value, DataType, Datapoint};
use kuksa::Client;
use log::warn;
use prost_types::Timestamp;
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
use zenoh::{buffers::ZBuf, sample::Sample};

use crate::utils::{metadata_store::MetadataInfo, zenoh_utils::zbuf_to_string};

pub async fn fetch_metadata(
    mut kuksa_client: Client,
    paths: Vec<&str>,
    metadata_store: &super::metadata_store::MetadataStore,
) -> Client {
    let mut store = metadata_store.lock().await;

    let data_entries: Vec<kuksa::DataEntry> = kuksa_client.get_metadata(paths).await.unwrap();

    for entry in data_entries {
        store.insert(
            entry.path,
            MetadataInfo {
                data_type: DataType::from_i32(entry.metadata.unwrap().data_type)
                    .unwrap_or(DataType::Unspecified),
            },
        );
    }

    kuksa_client
}

pub fn new_datapoint(data_type: &DataType, payload: &ZBuf) -> Datapoint {
    let value = match data_type {
        DataType::String => Value::String(zbuf_to_string(payload).unwrap()),
        DataType::Boolean => Value::Bool(zbuf_to_string(payload).unwrap().parse().unwrap()),
        DataType::Int8 | DataType::Int16 | DataType::Int32 => {
            Value::Int32(zbuf_to_string(payload).unwrap().parse().unwrap())
        }
        DataType::Int64 => Value::Int64(zbuf_to_string(payload).unwrap().parse().unwrap()),
        DataType::Uint8 | DataType::Uint16 | DataType::Uint32 => {
            Value::Uint32(zbuf_to_string(payload).unwrap().parse().unwrap())
        }
        DataType::Uint64 => Value::Uint64(zbuf_to_string(payload).unwrap().parse().unwrap()),
        DataType::Float => Value::Float(zbuf_to_string(payload).unwrap().parse().unwrap()),
        DataType::Double => Value::Double(zbuf_to_string(payload).unwrap().parse().unwrap()),
        // TODO: Add cases for array types
        _ => Value::String(format!("Unsupported type: {:?}", data_type)),
    };

    let now = SystemTime::now();
    let duration_since_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");

    let timestamp = Timestamp {
        seconds: duration_since_epoch.as_secs() as i64,
        nanos: duration_since_epoch.subsec_nanos() as i32,
    };

    Datapoint {
        timestamp: Some(timestamp), // TODO: get timestamp right
        value: Some(value),
    }
}

pub fn new_datapoint_for_update(
    path: &str,
    sample: &Sample,
    metadata_store: &HashMap<String, MetadataInfo>,
) -> HashMap<String, Datapoint> {
    let mut datapoint_update = HashMap::new();

    datapoint_update.insert(
        path.to_string(),
        metadata_store
            .get(path)
            .map(|metadata_info| new_datapoint(&metadata_info.data_type, &sample.value.payload))
            .unwrap(),
    );

    datapoint_update
}

pub fn datapoint_to_string(datapoint: &Datapoint) -> Option<String> {
    datapoint
        .value
        .as_ref()
        .map(|value| match value {
            kuksa::proto::v1::datapoint::Value::String(v) => v.clone(),
            kuksa::proto::v1::datapoint::Value::Bool(v) => v.to_string(),
            kuksa::proto::v1::datapoint::Value::Int32(v) => v.to_string(),
            kuksa::proto::v1::datapoint::Value::Int64(v) => v.to_string(),
            kuksa::proto::v1::datapoint::Value::Uint32(v) => v.to_string(),
            kuksa::proto::v1::datapoint::Value::Uint64(v) => v.to_string(),
            kuksa::proto::v1::datapoint::Value::Float(v) => v.to_string(),
            kuksa::proto::v1::datapoint::Value::Double(v) => v.to_string(),
            kuksa::proto::v1::datapoint::Value::StringArray(v) => format!("{:?}", v.values),
            kuksa::proto::v1::datapoint::Value::BoolArray(v) => format!("{:?}", v.values),
            kuksa::proto::v1::datapoint::Value::Int32Array(v) => format!("{:?}", v.values),
            kuksa::proto::v1::datapoint::Value::Int64Array(v) => format!("{:?}", v.values),
            kuksa::proto::v1::datapoint::Value::Uint32Array(v) => format!("{:?}", v.values),
            kuksa::proto::v1::datapoint::Value::Uint64Array(v) => format!("{:?}", v.values),
            kuksa::proto::v1::datapoint::Value::FloatArray(v) => format!("{:?}", v.values),
            kuksa::proto::v1::datapoint::Value::DoubleArray(v) => format!("{:?}", v.values),
        })
        .or_else(|| {
            warn!("Datapoint has no value");
            None
        })
}
