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
use log::{debug, error, info};
use std::error::Error;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::time::{self};

mod can;
mod kuksa_provider;
mod utils;

use can::comm;
use can::decoder::Decoder;
use kuksa_provider::provider::Provider;
use utils::adapter_config::AdapterConfig;
use utils::adapter_utils;

#[derive(Parser)]
struct Args {
    #[arg(short, long, help = "Path to JSON configuration file")]
    config: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    //Parse command line arguments to get the configuration file path.
    let args = Args::parse();
    info!(
        "Starting CAN Protocol adapter v{} with config file: {}",
        env!("CARGO_PKG_VERSION"),
        &args.config
    );
    // Read adapter configuration from the JSON file.
    let adapter_config = match adapter_utils::read_config(&args.config) {
        Ok(adapter_config) => adapter_config,
        Err(err) => {
            panic!(
                "Failed to open configuration file at path {}, {}",
                &args.config, err
            );
        }
    };

    // Validate the adapter configuration.
    adapter_utils::validate_adapter_config(&adapter_config)?;

    // Get broker IP and port from the adapter configuration.
    let broker_ip = adapter_config.general_config.broker_ip.clone();
    let broker_port = adapter_config.general_config.broker_port.clone();

    // Create a new Provider instance and connect to the data broker.
    let mut provider = Provider::new(broker_ip.clone(), broker_port.clone());
    match provider.connect_to_databroker().await {
        Ok(_) => {
            debug!(
                "Successfully connected to the databroker to {}:{}",
                broker_ip, broker_port
            );
        }
        Err(e) => {
            error!("Failed to connect to databroker: {:?}", e);
            return Err(e);
        }
    }

    //Load the DBC file and create a Decoder instance.
    let dbc_file_path = adapter_config.general_config.dbcfile.clone();
    let decoder = Decoder::new(&dbc_file_path)?;
    info!(
        "DBC file loaded from path: {}. DBC File Parsing successful.",
        dbc_file_path
    );

    // Register the user defined datapoints with the data broker.
    match provider.register_datapoints(&adapter_config).await {
        Ok(_) => {
            info!("Successfully registered datapoints.");
        }
        Err(e) => {
            error!("Failed to register datapoints: {:?}", e);
            return Err(e);
        }
    }

    // Initialize the CAN socket.
    let socket = match comm::initialize_can_socket(&adapter_config).await {
        Ok(socket) => socket,
        Err(err) => {
            error!("Error initializing socket: {}", err);
            return Err(err);
        }
    };

    // Channels for inter-task communication
    let (pid_tx, pid_rx) = mpsc::channel::<usize>(256);
    let (res_tx, res_rx) = mpsc::channel::<bool>(256);

    // Create shared resources using Arc and Mutex.
    let shared_socket = Arc::new(Mutex::new(socket));
    let shared_provider = Arc::new(Mutex::new(provider));
    let shared_decoder = Arc::new(Mutex::new(decoder));
    let adapter_config = Arc::new(adapter_config);

    // Spawn a task for sending CAN data.
    let send_task_handle = tokio::spawn({
        let socket_instance = Arc::clone(&shared_socket);
        let adapter_config = Arc::clone(&adapter_config);
        async move {
            comm::send_can_data(socket_instance, adapter_config, pid_tx, res_rx).await;
        }
    });

    // Spawn a task for receiving CAN data.
    let receive_task_handle = tokio::spawn({
        let socket_instance = Arc::clone(&shared_socket);
        let adapter_config = Arc::clone(&adapter_config);
        let provider_instance = Arc::clone(&shared_provider);
        async move {
            comm::receive_can_data(
                socket_instance,
                adapter_config,
                provider_instance,
                shared_decoder,
                pid_rx,
                res_tx,
            )
            .await;
        }
    });

    // Wait for both tasks to complete.
    let _ = send_task_handle.await;
    let _ = receive_task_handle.await;

    Ok(())
}
