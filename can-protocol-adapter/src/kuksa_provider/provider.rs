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
use http::Uri;
use log::{error, info};
use std::collections::HashMap;
use std::error::Error as StdError;

use databroker_proto::kuksa::val::{self as proto, v1::Datapoint};
use kuksa::KuksaClient;
use proto::v1;
/// The `Provider` struct manages the connection to a Kuksa Data Broker
/// and provides methods to interact with it.
pub struct Provider {
    client: Option<KuksaClient>,
    broker_ip: String,
    broker_port: String,
}

impl Provider {
    pub fn new(broker_ip: String, broker_port: String) -> Self {
        Self {
            client: None,
            broker_ip,
            broker_port,
        }
    }

    pub async fn connect_to_databroker(&mut self) -> Result<(), Box<dyn StdError>> {
        match self.client {
            Some(_) => {
                info!("Already connected to data broker.");
                Ok(())
            }
            None => {
                let broker_address = format!("http://{}:{}/", self.broker_ip, self.broker_port);
                let uri: Uri = broker_address.parse().expect("Invalid broker URI");
                // Create a new Kuksa client instance
                let mut kuksa_client = KuksaClient::new(uri);
                // Attempt connection
                match kuksa_client.basic_client.try_connect().await {
                    Ok(_) => {
                        info!(
                            "Successfully connected to the databroker to {}",
                            broker_address
                        );
                        self.client = Some(kuksa_client);
                    }
                    Err(err) => {
                        error!("Failed to connect to Kuksa server: {}", err);
                        return Err(err.into());
                    }
                };
                Ok(())
            }
        }
    }

    pub async fn register_datapoints(
        &mut self,
        datapoints: HashMap<String, v1::Metadata>,
    ) -> Result<(), Box<dyn StdError>> {
        match &mut self.client {
            Some(client) => {
                // Attempt to register datapoints
                let result = client.set_metadata(datapoints).await;

                match result {
                    Ok(_) => {
                        info!("Successfully set metadata values.");
                        Ok(())
                    }
                    Err(err) => {
                        error!("Failed to set metadata: {}", err);
                        Err(Box::new(err))
                    }
                }
            }
            None => {
                let err_msg = "Not connected to databroker";
                error!("{}", err_msg);
                Err(err_msg.into())
            }
        }
    }

    pub async fn set_datapoint_values(
        &mut self,
        signal: &str,
        signal_value: f64,
        value_type: &str,
    ) -> Result<(), Box<dyn StdError>> {
        // Convert the signal value to the corresponding protobuf Value type.
        let value = match value_type {
            "float" => Some(proto::v1::datapoint::Value::Float(signal_value as f32)),
            "Int32" => Some(proto::v1::datapoint::Value::Int32(signal_value as i32)),
            "String" => Some(proto::v1::datapoint::Value::String(
                signal_value.to_string(),
            )),
            "double" | "Double" => Some(proto::v1::datapoint::Value::Double(signal_value)),
            "uint32" | "UInt32" => Some(proto::v1::datapoint::Value::Uint32(signal_value as u32)),
            "bool" | "Bool" | "boolean" | "Boolean" => {
                Some(proto::v1::datapoint::Value::Bool(signal_value != 0.0))
            }
            _ => {
                error!("Unsupported value type: {}", value_type);
                return Err("Unsupported value type".into());
            }
        };
        // Create the Datapoint with the converted value.
        let mut datapoints = HashMap::new();
        datapoints.insert(
            signal.to_string(),
            Datapoint {
                value,
                timestamp: None,
            },
        );
        // Set the datapoint value using the client.
        match self.client.as_mut() {
            Some(client) => match client.set_current_values(datapoints).await {
                Ok(_) => {
                    info!(
                        "Successfully set datapoint value for signal: {}, value: {}",
                        signal, signal_value
                    );
                    Ok(())
                }
                Err(err) => {
                    error!("Failed to set datapoint value for {}: {}", signal, err);
                    Err(err.into())
                }
            },
            None => {
                error!("Not connected to databroker");
                Err("Not connected to databroker".into())
            }
        }
    }
}
