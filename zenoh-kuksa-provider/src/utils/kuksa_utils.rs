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

use kuksa_rust_sdk::kuksa::common::ClientTraitV1;
use kuksa_rust_sdk::v1_proto::datapoint::Value;
use kuksa_rust_sdk::v1_proto::{DataEntry, DataType, Datapoint};
use kuksa_rust_sdk::kuksa::val::v1::KuksaClient;
use log::warn;
use prost_types::Timestamp;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use zenoh::bytes::ZBytes;
use zenoh::sample::Sample;

use crate::utils::{metadata_store::MetadataInfo, zenoh_utils::zbytes_to_string};

pub async fn fetch_metadata(
    mut kuksa_client: KuksaClient,
    paths: Vec<String>,
    metadata_store: &super::metadata_store::MetadataStore,

) -> KuksaClient {
    let mut store = metadata_store.lock().await;

    let data_entries: Vec<DataEntry> = kuksa_client.get_metadata(paths).await.unwrap();

    for entry in data_entries {
        store.insert(
            entry.path,
            MetadataInfo {
                data_type: DataType::try_from(entry.metadata.unwrap().data_type)
                    .unwrap_or(DataType::Unspecified),
            },
        );
    }

    kuksa_client
}

pub fn new_datapoint(data_type: &DataType, payload: &ZBytes) -> Datapoint {
    let value = match data_type {
        DataType::String => Value::String(zbytes_to_string(payload).unwrap()),
        DataType::Boolean => Value::Bool(zbytes_to_string(payload).unwrap().parse().unwrap()),
        DataType::Int8 | DataType::Int16 | DataType::Int32 => {
            Value::Int32(zbytes_to_string(payload).unwrap().parse().unwrap())
        }
        DataType::Int64 => Value::Int64(zbytes_to_string(payload).unwrap().parse().unwrap()),
        DataType::Uint8 | DataType::Uint16 | DataType::Uint32 => {
            Value::Uint32(zbytes_to_string(payload).unwrap().parse().unwrap())
        }
        DataType::Uint64 => Value::Uint64(zbytes_to_string(payload).unwrap().parse().unwrap()),
        DataType::Float => Value::Float(zbytes_to_string(payload).unwrap().parse().unwrap()),
        DataType::Double => Value::Double(zbytes_to_string(payload).unwrap().parse().unwrap()),
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
            .map(|metadata_info| new_datapoint(&metadata_info.data_type, sample.payload()))
            .unwrap(),
    );

    datapoint_update
}

pub fn datapoint_to_string(datapoint: &Datapoint) -> Option<String> {
    datapoint
        .value
        .as_ref()
        .map(|value| match value {
            kuksa_rust_sdk::v1_proto::datapoint::Value::String(v) => v.clone(),
            kuksa_rust_sdk::v1_proto::datapoint::Value::Bool(v) => v.to_string(),
            kuksa_rust_sdk::v1_proto::datapoint::Value::Int32(v) => v.to_string(),
            kuksa_rust_sdk::v1_proto::datapoint::Value::Int64(v) => v.to_string(),
            kuksa_rust_sdk::v1_proto::datapoint::Value::Uint32(v) => v.to_string(),
            kuksa_rust_sdk::v1_proto::datapoint::Value::Uint64(v) => v.to_string(),
            kuksa_rust_sdk::v1_proto::datapoint::Value::Float(v) => v.to_string(),
            kuksa_rust_sdk::v1_proto::datapoint::Value::Double(v) => v.to_string(),
            kuksa_rust_sdk::v1_proto::datapoint::Value::StringArray(v) => format!("{:?}", v.values),
            kuksa_rust_sdk::v1_proto::datapoint::Value::BoolArray(v) => format!("{:?}", v.values),
            kuksa_rust_sdk::v1_proto::datapoint::Value::Int32Array(v) => format!("{:?}", v.values),
            kuksa_rust_sdk::v1_proto::datapoint::Value::Int64Array(v) => format!("{:?}", v.values),
            kuksa_rust_sdk::v1_proto::datapoint::Value::Uint32Array(v) => format!("{:?}", v.values),
            kuksa_rust_sdk::v1_proto::datapoint::Value::Uint64Array(v) => format!("{:?}", v.values),
            kuksa_rust_sdk::v1_proto::datapoint::Value::FloatArray(v) => format!("{:?}", v.values),
            kuksa_rust_sdk::v1_proto::datapoint::Value::DoubleArray(v) => format!("{:?}", v.values),
        })
        .or_else(|| {
            warn!("Datapoint has no value");
            None
        })
}
