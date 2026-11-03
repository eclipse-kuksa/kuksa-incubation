/********************************************************************************
 * Copyright (c) 2026 Contributors to the Eclipse Foundation
 *
 * See the NOTICE file(s) distributed with this work for additional
 * information regarding copyright ownership.
 *
 * This program and the accompanying materials are made available under the
 * terms of the Apache License Version 2.0 which is available at
 * https://www.apache.org/licenses/LICENSE-2.0
 *
 * SPDX-License-Identifier: Apache-2.0
 ********************************************************************************/

use databroker::authorization::Authorization;
use databroker::broker::{ChangeType, DataBroker, DataType, DataValue, EntryType};
use databroker::grpc;
use databroker::permissions::ALLOW_ALL;
use mw_com_provider::score_bindings::{validate_score_bindings, ScoreBindingTransport};
use mw_com_provider::{
    Direction, LolaScoreRuntimeAdapter, MwComProviderWorker, ProviderConfig, VssDataType,
};
use std::collections::HashSet;
use std::net::SocketAddr;
use std::thread::JoinHandle;
use tokio::sync::oneshot;
use tracing::info;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    databroker::init_logging();

    let args = DemoArgs::parse()?;
    let mut config = ProviderConfig::load_json(&args.provider_config)?;
    // The generated provider config is the source of truth. Optional filtering
    // is applied before Databroker runtime IDs are assigned below.
    if let Some(included_paths) = args.included_paths() {
        config
            .mappings
            .retain(|mapping| included_paths.contains(mapping.vss_path.as_str()));
        if config.mappings.is_empty() {
            return Err("--include-vss-path removed all provider mappings".into());
        }
    }
    validate_score_bindings(&config)?;

    let broker = DataBroker::new("databroker-mw-com", "databroker-mw-com");
    // Generated configs carry stable model IDs. Databroker assigns entry IDs at
    // runtime, so rewrite mappings before registering provider callbacks.
    seed_databroker_metadata(&broker, &mut config).await?;

    let ProviderWorkerHandle {
        shutdown_tx: provider_shutdown_tx,
        done_rx: mut provider_done_rx,
        join_handle: provider_join_handle,
    } = start_provider_worker(broker.clone(), config, args.score_config)?;

    let (server_shutdown_tx, server_shutdown_rx) = oneshot::channel();
    let mut provider_shutdown_tx = Some(provider_shutdown_tx);
    let mut server_shutdown_tx = Some(server_shutdown_tx);

    info!(
        address = %args.address,
        provider_config = %args.provider_config,
        "Starting Databroker with in-process mw::com provider"
    );

    let apis = [grpc::server::Api::KuksaValV1, grpc::server::Api::KuksaValV2];
    let server = grpc::server::serve_tcp(
        args.address,
        broker,
        &apis,
        Authorization::Disabled,
        async move {
            let _ = server_shutdown_rx.await;
        },
    );
    tokio::pin!(server);

    tokio::select! {
        server_result = &mut server => {
            send_shutdown(&mut provider_shutdown_tx);
            server_result?;
        }
        provider_result = &mut provider_done_rx => {
            send_shutdown(&mut server_shutdown_tx);
            provider_result
                .map_err(|error| format!("mw::com provider worker stopped unexpectedly: {error}"))?
                .map_err(|error| format!("mw::com provider worker failed: {error}"))?;
            server.await?;
        }
        signal_result = tokio::signal::ctrl_c() => {
            signal_result?;
            send_shutdown(&mut provider_shutdown_tx);
            send_shutdown(&mut server_shutdown_tx);
            server.await?;
        }
    }

    provider_join_handle
        .join()
        .map_err(|_| "mw::com provider worker thread panicked")?;
    Ok(())
}

fn send_shutdown(tx: &mut Option<oneshot::Sender<()>>) {
    if let Some(tx) = tx.take() {
        let _ = tx.send(());
    }
}

async fn seed_databroker_metadata(
    broker: &DataBroker,
    config: &mut ProviderConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    for mapping in &mut config.mappings {
        // Add entries with the correct VSS field model so clients can use normal
        // read/subscribe behavior for datapoints and actuate behavior for targets.
        let signal_id = broker
            .authorized_access(&ALLOW_ALL)
            .add_entry(
                mapping.vss_path.clone(),
                data_type(mapping.datatype),
                ChangeType::Continuous,
                entry_type(mapping.direction),
                format!("mw::com provider mapping for {}", mapping.vss_path),
                mapping
                    .min
                    .map(|value| bound_value(mapping.datatype, value)),
                mapping
                    .max
                    .map(|value| bound_value(mapping.datatype, value)),
                None,
                mapping.unit.clone(),
            )
            .await
            .map_err(|error| format!("failed to seed {}: {error:?}", mapping.vss_path))?;
        mapping.signal_id = signal_id;
    }

    Ok(())
}

fn bound_value(data_type: VssDataType, value: f64) -> DataValue {
    match data_type {
        VssDataType::Bool => DataValue::Bool(value != 0.0),
        VssDataType::String => DataValue::String(value.to_string()),
        VssDataType::Int8 | VssDataType::Int32 => DataValue::Int32(value as i32),
        VssDataType::Int64 => DataValue::Int64(value as i64),
        VssDataType::Uint8 | VssDataType::Uint32 => DataValue::Uint32(value as u32),
        VssDataType::Uint64 => DataValue::Uint64(value as u64),
        VssDataType::Float => DataValue::Float(value as f32),
        VssDataType::Double => DataValue::Double(value),
    }
}

fn data_type(data_type: VssDataType) -> DataType {
    match data_type {
        VssDataType::Bool => DataType::Bool,
        VssDataType::String => DataType::String,
        VssDataType::Int8 => DataType::Int8,
        VssDataType::Int32 => DataType::Int32,
        VssDataType::Int64 => DataType::Int64,
        VssDataType::Uint8 => DataType::Uint8,
        VssDataType::Uint32 => DataType::Uint32,
        VssDataType::Uint64 => DataType::Uint64,
        VssDataType::Float => DataType::Float,
        VssDataType::Double => DataType::Double,
    }
}

fn entry_type(direction: Direction) -> EntryType {
    match direction {
        Direction::Datapoint => EntryType::Sensor,
        Direction::Actuator | Direction::Bidirectional => EntryType::Actuator,
    }
}

struct DemoArgs {
    address: SocketAddr,
    provider_config: String,
    score_config: String,
    include_vss_paths: Vec<String>,
}

impl DemoArgs {
    fn parse() -> Result<Self, Box<dyn std::error::Error>> {
        let mut address = "127.0.0.1:55555".parse()?;
        let mut provider_config =
            "mw_com_provider/generated/mw_com_provider_config.json".to_string();
        let mut score_config =
            "mw_com_provider/generated/vehicle_dynamics_lola_config.json".to_string();
        let mut include_vss_paths = Vec::new();

        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--address" | "--addr" => {
                    address = required_arg(&mut args, "--address")?.parse()?
                }
                "--provider-config" => {
                    provider_config = required_arg(&mut args, "--provider-config")?
                }
                "--score-config" => score_config = required_arg(&mut args, "--score-config")?,
                "--include-vss-path" => {
                    include_vss_paths.push(required_arg(&mut args, "--include-vss-path")?)
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                unknown => return Err(format!("unknown option: {unknown}").into()),
            }
        }

        Ok(Self {
            address,
            provider_config,
            score_config,
            include_vss_paths,
        })
    }

    fn included_paths(&self) -> Option<HashSet<&str>> {
        if self.include_vss_paths.is_empty() {
            None
        } else {
            Some(self.include_vss_paths.iter().map(String::as_str).collect())
        }
    }
}

fn required_arg(
    args: &mut impl Iterator<Item = String>,
    option: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    args.next()
        .ok_or_else(|| format!("missing value for {option}").into())
}

fn print_usage() {
    eprintln!(
        "Usage: databroker-mw-com [OPTIONS]\n\nOptions:\n  --address <ADDR>                  Databroker gRPC bind address [default: 127.0.0.1:55555]\n  --provider-config <FILE>          Provider mapping config [default: mw_com_provider/generated/mw_com_provider_config.json]\n  --score-config <FILE>             SCORE LoLa config [default: mw_com_provider/generated/vehicle_dynamics_lola_config.json]\n  --include-vss-path <PATH>         Include only this VSS mapping; repeatable [default: all configured mappings]\n  -h, --help                        Print help"
    );
}

struct ProviderWorkerHandle {
    shutdown_tx: oneshot::Sender<()>,
    done_rx: oneshot::Receiver<Result<(), String>>,
    join_handle: JoinHandle<()>,
}

fn start_provider_worker(
    broker: DataBroker,
    config: ProviderConfig,
    score_config: String,
) -> Result<ProviderWorkerHandle, Box<dyn std::error::Error>> {
    // The LoLa adapter can make native calls and owns runtime-specific state.
    // Keep it on a dedicated Tokio runtime and report readiness before serving
    // Kuksa gRPC traffic.
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let (done_tx, done_rx) = oneshot::channel();

    let join_handle = std::thread::Builder::new()
        .name("mw-com-lola-worker".to_string())
        .spawn(move || {
            let ready_tx_for_worker = ready_tx.clone();
            let result = run_provider_worker(
                broker,
                config,
                score_config,
                shutdown_rx,
                ready_tx_for_worker,
            )
            .map_err(|error| error.to_string());
            if let Err(error) = &result {
                let _ = ready_tx.send(Err(error.clone()));
            }
            let _ = done_tx.send(result);
        })?;

    ready_rx
        .recv()
        .map_err(|error| format!("mw::com provider worker did not report startup: {error}"))?
        .map_err(|error| format!("mw::com provider worker failed to start: {error}"))?;

    Ok(ProviderWorkerHandle {
        shutdown_tx,
        done_rx,
        join_handle,
    })
}

fn run_provider_worker(
    broker: DataBroker,
    config: ProviderConfig,
    score_config: String,
    shutdown_rx: oneshot::Receiver<()>,
    ready_tx: std::sync::mpsc::Sender<Result<(), String>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(provider_worker_threads())
        .thread_name("mw-com-lola-runtime")
        .enable_all()
        .build()?;

    runtime.block_on(async move {
        let adapter = LolaScoreRuntimeAdapter::new(score_config);
        let transport = ScoreBindingTransport::new(adapter);
        let provider = MwComProviderWorker::new(broker, ALLOW_ALL.clone(), config, transport)?;
        provider.register_with_broker().await?;
        ready_tx.send(Ok(())).map_err(|error| {
            format!("failed to report mw::com provider worker startup: {error}")
        })?;
        provider.run_until_shutdown(shutdown_rx).await?;
        Ok(())
    })
}

fn provider_worker_threads() -> usize {
    std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(2)
        .max(2)
}
