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

use clap::Parser;
use kuksa_rust_sdk::kuksa::common::ClientTraitV1;
use kuksa_rust_sdk::kuksa::val::v1::KuksaClient;
use log::{debug, error, info, warn};
use provider_config::ProviderConfig;
use std::collections::HashMap;
use std::sync::Arc;
use std::{fs, process};
use zenoh::pubsub::Publisher;
use zenoh::Session;

mod provider_config;
mod utils;

use utils::kuksa_utils::{datapoint_to_string, fetch_metadata, new_datapoint_for_update};
use utils::metadata_store::{create_metadata_store, MetadataStore};
use utils::zenoh_utils::extract_attachment_as_string;

const VEHCILE_KEY_EXPR: &str = "Vehicle/**";

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to a valid json5 configuration file.
    #[arg(
        short,
        long,
        default_value = "DEFAULT_CONFIG.json5",
        env = "PROVIDER_CONFIG"
    )]
    config: String,
}

impl Args {
    fn read_config(&self) -> Result<ProviderConfig, Box<dyn std::error::Error>> {
        let config_str = fs::read_to_string(&self.config)?;
        let config = json5::from_str(&config_str)?;

        Ok(config)
    }
}

async fn handling_zenoh_subscription(
    session: Arc<Session>,
    metadata_store: MetadataStore,
    mut kuksa_client: KuksaClient,
) {
    info!("Listening on selector: {:?}", VEHCILE_KEY_EXPR);

    let subscriber = session.declare_subscriber(VEHCILE_KEY_EXPR).await.unwrap();

    let store = metadata_store.lock().await;

    while let Ok(sample) = subscriber.recv_async().await {
        let vss_path = sample.key_expr().replace("/", ".").to_string();

        debug!(
            "Received ('{}': '{:?}' with attachment: {:?})",
            &vss_path,
            sample.payload(),
            sample.attachment()
        );

        if let Some(field_type) = extract_attachment_as_string(&sample) {
            if field_type == "currentValue" {
                let datapoint_update = new_datapoint_for_update(&vss_path, &sample, &store);
                debug!("Forwarding: {:#?}", datapoint_update);
                if let Err(e) = kuksa_client.set_current_values(datapoint_update).await {
                    error!("failed to publish current value to Kuksa Databroker: {e}");
                }
            }
        }
    }
}

async fn publish_to_zenoh(
    provider_config: ProviderConfig,
    session: Arc<Session>,
    mut kuksa_client: KuksaClient
) {
    let vss_paths = Vec::from_iter(provider_config.signals.iter().map(String::as_str));

    let mut publishers: HashMap<String, Publisher<'_>> = HashMap::new();
    for vss_path in provider_config.signals.clone() {
        let zenoh_key = vss_path.replace(".", "/");
        let publisher = session.declare_publisher(zenoh_key.clone()).await.unwrap();
        publishers.insert(vss_path, publisher);
    }
    info!(
        "Subscribing to the following paths on the Kuksa Databroker: {:?}",
        vss_paths
    );

    match kuksa_client.subscribe_target_values(provider_config.signals).await {
        Ok(mut stream) => {
            while let Some(response) = stream.message().await.unwrap() {
                for update in &response.updates {
                    if let Some(entry) = &update.entry {
                        if let Some(datapoint) = &entry.actuator_target {
                            let vss_path = &entry.path;

                            if let Some(publisher) = publishers.get(vss_path.as_str()) {
                                if let Some(value) = datapoint_to_string(datapoint) {
                                    if let Err(e) =
                                        publisher.put(value).attachment("targetValue").await
                                    {
                                        warn!(
                                            "Failed to publish target value to Zenoh network: {e}"
                                        );
                                    } else {
                                        debug!(
                                            "Published target value [signal: {}] to Zenoh network",
                                            vss_path
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Err(e) => {
            eprintln!("Error subscribing: {:?}", e);
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the logger
    env_logger::init();
    let args = Args::parse();
    let provider_config = args.read_config().inspect_err(|e| {
        error!("Failed to open configuration file [{}]: {e}", args.config);
        process::exit(1);
    })?;

    let zenoh_session = zenoh::open(provider_config.zenoh_config.to_owned())
        .await
        .map(Arc::new)
        .map_err(|e| e as Box<dyn std::error::Error>)?;

    let metadata_store = create_metadata_store();

    let uri = http::Uri::try_from(provider_config.kuksa.databroker_url.as_str())
        .expect("Invalid URI for Kuksa Databroker connection.");
    let mut client = KuksaClient::new(uri.clone());
    let actuation_client = KuksaClient::new(uri);

    client = fetch_metadata(
        client,
        provider_config.signals.clone(),
        &metadata_store,
    )
    .await;

    let subscriber_handle = tokio::spawn({
        let session = Arc::clone(&zenoh_session);
        let metadata_store = Arc::clone(&metadata_store);
        handling_zenoh_subscription(session, metadata_store, client)
    });

    let publisher_handle = tokio::spawn({
        let session = Arc::clone(&zenoh_session);
        publish_to_zenoh(provider_config, session, actuation_client)
    });

    let _ = subscriber_handle.await;
    let _ = publisher_handle.await;
    Ok(())
}
