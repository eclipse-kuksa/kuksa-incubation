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
use super::decoder;
use super::socket;
use crate::kuksa_provider::provider::Provider;
use crate::time::Instant;
use crate::utils::adapter_config::AdapterConfig;
use log::{debug, error, trace, warn};
use std::error::Error as StdError;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::time::{self, sleep, Duration};

pub async fn initialize_can_socket(
    adapter_config: &AdapterConfig,
) -> Result<socket::Socket, Box<dyn StdError>> {
    // Get CAN interface, socket type and protocol from the adapter config.
    let can_interface = adapter_config.can_config.can_interface.clone();
    let socket_can_type = adapter_config.can_config.socket_can_type.clone();
    let socket_can_protocol = adapter_config.can_config.socket_can_protocol.clone();

    // Create a new socket instance.
    let mut socket = socket::Socket::new(can_interface, socket_can_type, socket_can_protocol);

    // Open the CAN socket.
    socket.open_socket(
        adapter_config.can_config.rx_id,
        adapter_config.can_config.tx_id,
        adapter_config.can_config.use_extended_id,
    )?;

    // Set the socket to non-blocking mode.
    socket.set_nonblocking()?;

    Ok(socket)
}

pub async fn send_can_data(
    socket: Arc<Mutex<socket::Socket>>,
    adapter_config: Arc<AdapterConfig>,
    pid_tx: mpsc::Sender<usize>,
    mut pid_rx: mpsc::Receiver<bool>,
) {
    // Time interval for reading and sending requests.
    let mut interval = time::interval(Duration::from_millis(1));
    // Read the next request inteval for each PID.
    let mut next_request_times: Vec<Instant> = adapter_config
        .pid_table
        .iter()
        .map(|entry| Instant::now() + Duration::from_millis(entry.interval_ms as u64))
        .collect();

    loop {
        interval.tick().await;

        // Iterate over each entry in the PID table.
        for (i, entry) in adapter_config.pid_table.iter().enumerate() {
            // Check if it is time to send the next request for current PID.
            if Instant::now() >= next_request_times[i] {
                // Lock the socket mutex for exclusive access.
                let mut socket_lock = socket.lock().await;
                // Write the request PID to the socket.
                if let Err(err) = socket_lock.write_socket(&entry.request_pid) {
                    error!("Error sending CAN data: {}", err);
                    continue;
                }
                //Release the socket lock.
                drop(socket_lock);
                debug!("CAN Tx: PID: {:X?}", entry.request_pid);
                if let Err(err) = pid_tx.send(i).await {
                    error!("Error sending request info: {}", err);
                }
                // Wait for a response from the receive_can_data function , Max timeout 1 second
                match time::timeout(Duration::from_secs(1), pid_rx.recv()).await {
                    Ok(Some(_)) => {
                        trace!("Received response for PID: {:X?}", entry.request_pid);
                    }
                    Err(_) => {
                        warn!(
                            "Timeout waiting for response for PID: {:X?}",
                            entry.request_pid
                        );
                    }
                    Ok(None) => {
                        error!("Response channel closed.");
                        return;
                    }
                }
                // Update the next request time for this PID.
                next_request_times[i] =
                    Instant::now() + Duration::from_millis(entry.interval_ms as u64);
            }
        }
    }
}

pub async fn receive_can_data(
    socket: Arc<Mutex<socket::Socket>>,
    adapter_config: Arc<AdapterConfig>,
    provider: Arc<Mutex<Provider>>,
    decoder: Arc<Mutex<decoder::Decoder>>,
    mut res_rx: mpsc::Receiver<usize>,
    res_tx: mpsc::Sender<bool>,
) {
    loop {
        // Receive the index of the request PID.
        match res_rx.recv().await {
            Some(index) => {
                trace!("Received index: {}", index);
                // Get the corresponding entry from the PID table.
                if let Some(entry) = adapter_config.pid_table.get(index) {
                    // Calculate the timeout duration for waiting for a response.
                    let delay_duration = Duration::from_millis(entry.response_timeout_ms as u64);
                    // Sleep for the configured response timeout to allow the CAN response to arrive.
                    sleep(delay_duration).await;
                    //Lock the socket mutex for exclusive access and read data from the socket.
                    let mut socket_lock = socket.lock().await;
                    let (notify, data) = match socket_lock.read_socket() {
                        // Define data outside the match arm.
                        Ok(data) => {
                            debug!("CAN Rx: Data:{:X?}", data);

                            let notify = if data.len() >= 2
                                && entry.response_pid.len() >= 2
                                && data[0..2] == entry.response_pid[0..2]
                            {
                                trace!("Received matching response for index: {}", index);
                                true
                            } else {
                                warn!("Received mismatched response for index: {}", index);
                                false
                            };
                            (notify, Some(data)) // Return data with notify
                        }
                        Err(err) => {
                            error!("Error receiving CAN data: {}", err);
                            (false, None) // Return None for data
                        }
                    };
                    // Release the socket lock.
                    drop(socket_lock);

                    // If a matching response was received as per response pid entry, process the data.
                    if notify {
                        let data = data.expect("Data should be present if notify is true");
                        // Send a notification to the sender task.
                        if let Err(err) = res_tx.send(true).await {
                            error!("Error sending response notification: {}", err);
                        }
                        let dbc_signal = entry.dbc_signal_name.clone();
                        let decoder = decoder.lock().await;
                        match decoder.decode_message_by_name(&dbc_signal, data) {
                            Ok(decoded_value) => {
                                debug!(
                                    "Decoded value for signal {}: {}",
                                    dbc_signal, decoded_value
                                );
                                // Set the decoded value in the provider.
                                let vss_signal = entry.vss_signal.signal_name.clone();
                                let vss_datatype = entry.vss_signal.datatype.clone();
                                let _provider_handle = tokio::spawn({
                                    let provider_instance = provider.clone();
                                    async move {
                                        let mut provider = provider_instance.lock().await;
                                        if let Err(e) = provider
                                            .set_datapoint_values(
                                                &vss_signal,
                                                decoded_value,
                                                &vss_datatype,
                                            )
                                            .await
                                        {
                                            error!("Error setting datapoint value: {}", e);
                                        }
                                    }
                                });
                            }
                            Err(err) => {
                                error!("Error decoding message for {}: {}", dbc_signal, err);
                            }
                        }
                    }
                } else {
                    error!("Invalid index received: {}", index);
                }
            }
            None => {
                error!("Sender task closed the channel.");
                break;
            }
        }
    }
}
