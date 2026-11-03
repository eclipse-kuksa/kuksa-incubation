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

#[cfg(feature = "score-lola")]
use databroker::broker::{ChangeType, DataBroker, DataType, DataValue, EntryType};
#[cfg(feature = "score-lola")]
use databroker::permissions::ALLOW_ALL;
#[cfg(feature = "score-lola")]
use mw_com_provider::score_bindings::{validate_score_bindings, ScoreBindingTransport};
#[cfg(feature = "score-lola")]
use mw_com_provider::{
    Direction, LolaScoreRuntimeAdapter, MwComProvider, ProviderConfig, VssDataType,
};
#[cfg(feature = "score-lola")]
use tokio::sync::oneshot;

#[cfg(feature = "score-lola")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = RuntimeArgs::parse()?;
    let config = ProviderConfig::load_json(&args.provider_config)?;
    validate_score_bindings(&config)?;

    let broker = DataBroker::new(&args.provider_name, &args.provider_name);
    if args.seed_databroker_metadata {
        seed_databroker_metadata(&broker, &config).await?;
    }

    let adapter = if let Some(score_config) = args.score_config {
        LolaScoreRuntimeAdapter::new(score_config)
    } else {
        LolaScoreRuntimeAdapter::from_score_config_env()?
    };
    let transport = ScoreBindingTransport::new(adapter);
    let provider = MwComProvider::new(broker, ALLOW_ALL.clone(), config, transport)?;

    provider.register_with_broker().await?;
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let provider_task = tokio::spawn(async move { provider.run_until_shutdown(shutdown_rx).await });

    tokio::signal::ctrl_c().await?;
    let _ = shutdown_tx.send(());
    provider_task.await??;
    Ok(())
}

#[cfg(not(feature = "score-lola"))]
fn main() {
    eprintln!(
        "mw_com_provider binary requires --features score-lola for the current delivery runtime"
    );
    std::process::exit(2);
}

#[cfg(feature = "score-lola")]
struct RuntimeArgs {
    provider_config: String,
    score_config: Option<String>,
    provider_name: String,
    seed_databroker_metadata: bool,
}

#[cfg(feature = "score-lola")]
impl RuntimeArgs {
    fn parse() -> Result<Self, Box<dyn std::error::Error>> {
        let mut provider_config = "generated/mw_com_provider_config.json".to_string();
        let mut score_config = None;
        let mut provider_name = "mw_com_provider".to_string();
        let mut seed_databroker_metadata = false;

        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--provider-config" => {
                    provider_config = required_arg(&mut args, "--provider-config")?
                }
                "--score-config" => score_config = Some(required_arg(&mut args, "--score-config")?),
                "--provider-name" => provider_name = required_arg(&mut args, "--provider-name")?,
                "--seed-databroker-metadata" => seed_databroker_metadata = true,
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                unknown => return Err(format!("unknown option: {unknown}").into()),
            }
        }

        Ok(Self {
            provider_config,
            score_config,
            provider_name,
            seed_databroker_metadata,
        })
    }
}

#[cfg(feature = "score-lola")]
fn required_arg(
    args: &mut impl Iterator<Item = String>,
    option: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    args.next()
        .ok_or_else(|| format!("missing value for {option}").into())
}

#[cfg(feature = "score-lola")]
async fn seed_databroker_metadata(
    broker: &DataBroker,
    config: &ProviderConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    for mapping in &config.mappings {
        let data_type = data_type(mapping.datatype);
        let entry_type = entry_type(mapping.direction);
        broker
            .authorized_access(&ALLOW_ALL)
            .add_entry(
                mapping.vss_path.clone(),
                data_type,
                ChangeType::Continuous,
                entry_type,
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
    }

    Ok(())
}

#[cfg(feature = "score-lola")]
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

#[cfg(feature = "score-lola")]
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

#[cfg(feature = "score-lola")]
fn entry_type(direction: Direction) -> EntryType {
    match direction {
        Direction::Datapoint => EntryType::Sensor,
        Direction::Actuator | Direction::Bidirectional => EntryType::Actuator,
    }
}

#[cfg(feature = "score-lola")]
fn print_usage() {
    eprintln!(
        "Usage: mw_com_provider [OPTIONS]\n\nOptions:\n  --provider-config <FILE>      Provider mapping config [default: generated/mw_com_provider_config.json]\n  --score-config <FILE>         SCORE LoLa config; otherwise SCORE_CONFIG_PATH is used\n  --provider-name <NAME>        In-process Databroker authority name [default: mw_com_provider]\n  --seed-databroker-metadata    Seed Databroker entries from provider config for standalone E2E runs\n  -h, --help                    Print help"
    );
}
