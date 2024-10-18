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
use log::{debug, info};
use provider_config::ProviderConfig;
use std::collections::HashMap;
use std::sync::Arc;
use std::{error::Error, fs};
use tokio::sync::Mutex;

mod provider_config;
mod utils;

use utils::kuksa_utils::{datapoint_to_string, fetch_metadata, new_datapoint_for_update};
use utils::metadata_store::{create_metadata_store, MetadataStore};
use utils::zenoh_utils::{extract_attachment_as_string, split_once};
use zenoh::prelude::r#async::*;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to a valid json5 configuration file
    #[arg(
        short,
        long,
        default_value = "DEFAULT_CONFIG.json5",
        env = "PROVIDER_CONFIG"
    )]
    config: String,
}

fn read_config(path: &str) -> Result<ProviderConfig, Box<dyn Error>> {
    let config_str = fs::read_to_string(path)?;
    let config = json5::from_str(&config_str)?;

    Ok(config)
}

async fn handling_zenoh_subscribtion(
    provider_config: Arc<ProviderConfig>,
    session: Arc<Session>,
    metadata_store: MetadataStore,
    kuksa_client: Arc<Mutex<kuksa::Client>>,
) {
    info!("Listening on selector: {:?}", provider_config.zenoh.key_exp);

    let provider_config_clone = Arc::clone(&provider_config);
    let subscriber = session
        .declare_subscriber(provider_config_clone.zenoh.key_exp.clone())
        .res()
        .await
        .unwrap();

    let store = metadata_store.lock().await;

    while let Ok(sample) = subscriber.recv_async().await {
        let vss_path = sample.key_expr.replace("/", ".").to_string();

        debug!(
            "Received ('{}': '{}' with attachment: {:?})",
            &vss_path, sample.value, sample.attachment
        );

        let field_type = extract_attachment_as_string(&sample);

        if field_type == "currentValue" {
            let datapoint_update = new_datapoint_for_update(&vss_path, &sample, &store);

            let mut sub_client = kuksa_client.lock().await;
            debug!("Forwarding: {:#?}", datapoint_update);
            sub_client
                .set_current_values(datapoint_update)
                .await
                .unwrap();
            drop(sub_client);
        }
    }
}

async fn publish_to_zenoh(
    provider_config: Arc<ProviderConfig>,
    session: Arc<Session>,
    mut kuksa_client: kuksa::Client,
) {
    let attachment = Some(String::from("type=targetValue"));

    let vss_paths = Vec::from_iter(provider_config.signals.iter().map(String::as_str));

    let mut publishers: HashMap<String, zenoh::publication::Publisher> = HashMap::new();
    for vss_path in &vss_paths {
        let zenoh_key = vss_path.replace(".", "/");
        let publisher = session
            .declare_publisher(zenoh_key.clone())
            .res()
            .await
            .unwrap();
        publishers.insert(vss_path.to_string(), publisher);
    }
    info!(
        "Subscribing to the following paths on the Kuksa Databroker: {:?}",
        vss_paths
    );

    match kuksa_client.subscribe_target_values(vss_paths).await {
        Ok(mut stream) => {
            while let Some(response) = stream.message().await.unwrap() {
                for update in &response.updates {
                    if let Some(entry) = &update.entry {
                        if let Some(datapoint) = &entry.actuator_target {
                            let vss_path = &entry.path;

                            if let Some(publisher) = publishers.get(vss_path.as_str()) {
                                let buf = match datapoint_to_string(datapoint) {
                                    Some(v) => v,
                                    None => "null".to_string(),
                                };

                                let mut put = publisher.put(buf.clone());

                                // Attachments look like this: "key1=value1&key2=value2"
                                if let Some(attachment) = &attachment {
                                    put = put.with_attachment(
                                        std::iter::once(attachment)
                                            .map(|pair| split_once(pair, '='))
                                            .collect(),
                                    );
                                }
                                put.res().await.unwrap();
                                debug!(
                                    "Published data: {} to topic: {} with attachment: {:?}",
                                    buf, vss_path, attachment
                                );
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
async fn main() {
    // Initialize the logger
    env_logger::init();
    let args = Args::parse();

    let mut provider_config = match read_config(&args.config) {
        Ok(provider_config) => provider_config,
        Err(err) => {
            panic!(
                "Failed to open configuration file at path {}, {}",
                &args.config, err
            );
        }
    };
    provider_config.remove_duplicate_active_signals();
    let provider_config = Arc::new(provider_config);

    let zenoh_session_config = provider_config.to_zenoh_config().unwrap();

    let zenoh_session = Arc::new(zenoh::open(zenoh_session_config).res().await.unwrap());

    let metadata_store = create_metadata_store();

    let uri = kuksa::Uri::try_from(provider_config.kuksa.databroker_url.as_str())
        .expect("Invalid URI for Kuksa Databroker connection.");
    let client = Arc::new(Mutex::new(kuksa::Client::new(uri.clone())));
    let actuation_client = kuksa::Client::new(uri);

    fetch_metadata(
        client.clone(),
        provider_config.signals.iter().map(|s| s as &str).collect(),
        &metadata_store,
    )
    .await;

    let subscriber_handle = tokio::spawn({
        let session = Arc::clone(&zenoh_session);
        let provider_config = Arc::clone(&provider_config);
        let metadata_store = Arc::clone(&metadata_store);
        let kuksa_client = Arc::clone(&client);
        handling_zenoh_subscribtion(provider_config, session, metadata_store, kuksa_client)
    });

    let publisher_handle = tokio::spawn({
        let session = Arc::clone(&zenoh_session);
        let provider_config = Arc::clone(&provider_config);
        publish_to_zenoh(provider_config, session, actuation_client)
    });

    let _ = subscriber_handle.await;
    let _ = publisher_handle.await;
}
