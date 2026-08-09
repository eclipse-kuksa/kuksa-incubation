/********************************************************************************
* Copyright (c) 2026 Contributors to the Eclipse Foundation
*
* This program and the accompanying materials are made available under the
* terms of the Apache License 2.0 which is available at
* http://www.apache.org/licenses/LICENSE-2.0
*
* SPDX-License-Identifier: Apache-2.0
********************************************************************************/

//! KUKSA Databroker provider — receives ACF-VSS messages from the Open1722
//! listener and publishes them to the KUKSA Databroker via its gRPC API.
//!
//! The provider uses a bidirectional streaming RPC (`OpenProviderStream`) for
//! low-latency value publication; it falls back to unary `publish_value` calls
//! for signals that have not yet been registered.

use std::collections::HashMap;

use kuksa_rust_sdk::kuksa::common::types::OpenProviderStream;
use kuksa_rust_sdk::kuksa::common::ClientTraitV2;
use kuksa_rust_sdk::kuksa::val::v2::KuksaClientV2;
use kuksa_rust_sdk::proto::kuksa::val::v2 as proto;
use log::{error, info};

use crate::open1722_vss::{ParsedVssMessage, VssValue};

/// Manages the connection to the KUKSA Databroker and publishes VSS values
/// received from Open1722 ACF-VSS frames.
pub struct KuksaProvider {
    client: KuksaClientV2,
    provider_stream: Option<OpenProviderStream>,
    registered_signals: HashMap<String, i32>,
}

impl KuksaProvider {
    pub fn new(host: &str) -> Self {
        let uri = host.parse().expect("Invalid KUKSA Databroker URI");
        Self {
            client: KuksaClientV2::new(uri),
            provider_stream: None,
            registered_signals: HashMap::new(),
        }
    }

    /// Opens a bidirectional provider stream to the Databroker for
    /// high-frequency value publishing.
    pub async fn open_stream(&mut self) -> Result<(), String> {
        self.provider_stream = Some(
            self.client
                .open_provider_stream(None)
                .await
                .map_err(|e| format!("Failed to open provider stream: {e:?}"))?,
        );
        info!("Provider stream opened successfully");
        Ok(())
    }

    /// Publishes a VSS value to the Databroker. Uses the provider stream
    /// if the signal is registered, otherwise falls back to unary `publish_value`.
    pub async fn publish_value(&mut self, msg: &ParsedVssMessage) -> Result<(), String> {
        let value = vss_value_to_proto(&msg.value);

        let use_stream = self
            .provider_stream
            .is_some()
            && self.registered_signals.contains_key(&msg.path);

        if use_stream {
            let stream = self.provider_stream.as_mut().unwrap();
            let id = self.registered_signals[&msg.path];

            let mut data_points = HashMap::new();
            data_points.insert(id, proto::Datapoint {
                timestamp: None,
                value: Some(value),
            });

            let request = proto::OpenProviderStreamRequest {
                action: Some(proto::open_provider_stream_request::Action::PublishValuesRequest(
                    proto::PublishValuesRequest {
                        request_id: 0,
                        data_points,
                    },
                )),
            };

            stream
                .sender
                .send(request)
                .await
                .map_err(|e| format!("Failed to send publish value: {e}"))?;
        } else {
            self.client
                .publish_value(msg.path.clone(), value)
                .await
                .map_err(|e| format!("Failed to publish value directly: {e:?}"))?;
        }

        Ok(())
    }

    /// Registers a signal with the Databroker so it accepts publish requests
    /// for this signal via the provider stream.
    pub async fn register_signal(&mut self, path: &str, sample_interval_ms: u32) -> Result<(), String> {
        let signal_id = self.resolved_id_for_path(path).await?;

        self.registered_signals.insert(path.to_string(), signal_id);

        if let Some(stream) = &mut self.provider_stream {
            let mut intervals = HashMap::new();
            intervals.insert(signal_id, proto::SampleInterval {
                interval_ms: sample_interval_ms,
            });

            let request = proto::OpenProviderStreamRequest {
                action: Some(proto::open_provider_stream_request::Action::ProvideSignalRequest(
                    proto::ProvideSignalRequest {
                        signals_sample_intervals: intervals,
                    },
                )),
            };

            stream
                .sender
                .send(request)
                .await
                .map_err(|e| format!("Failed to register signal: {e}"))?;

            info!("Registered signal: {path} (id={signal_id})");
        }
        Ok(())
    }

    /// Resolves a VSS path to its numeric signal ID via the Databroker's
    /// `resolve_ids_for_paths` RPC.  This ID is required before the signal
    /// can be published through the provider stream.
    async fn resolved_id_for_path(&mut self, path: &str) -> Result<i32, String> {
        self.client
            .resolve_ids_for_paths(vec![path.to_string()])
            .await
            .map_err(|e| format!("Failed to resolve signal ID for {path}: {e:?}"))
            .and_then(|map| {
                map.get(path)
                    .copied()
                    .ok_or_else(|| format!("Signal not found in databroker: {path}"))
            })
    }
}

/// Converts an Open1722 ACF-VSS value to the KUKSA protobuf Value type.
/// Convert a parsed Open1722 value into the KUKSA protobuf `Value` union.
/// Narrower integer types (i8, i16, u8, u16) are widened to 32-bit before
/// wrapping, matching KUKSA's available typed-value variants.
fn vss_value_to_proto(value: &VssValue) -> proto::Value {
    let typed_value = match value {
        VssValue::Bool(v) => proto::value::TypedValue::Bool(*v),
        VssValue::Int8(v) => proto::value::TypedValue::Int32(*v as i32),
        VssValue::Int16(v) => proto::value::TypedValue::Int32(*v as i32),
        VssValue::Int32(v) => proto::value::TypedValue::Int32(*v),
        VssValue::Int64(v) => proto::value::TypedValue::Int64(*v),
        VssValue::Uint8(v) => proto::value::TypedValue::Uint32(*v as u32),
        VssValue::Uint16(v) => proto::value::TypedValue::Uint32(*v as u32),
        VssValue::Uint32(v) => proto::value::TypedValue::Uint32(*v),
        VssValue::Uint64(v) => proto::value::TypedValue::Uint64(*v),
        VssValue::Float(v) => proto::value::TypedValue::Float(*v),
        VssValue::Double(v) => proto::value::TypedValue::Double(*v),
        VssValue::String(v) => proto::value::TypedValue::String(v.clone()),
        VssValue::Unknown => {
            error!("Unknown VSS datatype, cannot convert to proto Value");
            return proto::Value { typed_value: None };
        }
    };

    proto::Value {
        typed_value: Some(typed_value),
    }
}
