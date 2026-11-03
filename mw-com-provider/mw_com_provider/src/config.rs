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

use crate::error::MwComProviderError;
use crate::lifecycle::ReconnectConfig;
use crate::quality::SignalQuality;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Datapoint,
    Actuator,
    Bidirectional,
}

impl Direction {
    pub fn receives_datapoints(self) -> bool {
        matches!(self, Direction::Datapoint | Direction::Bidirectional)
    }

    pub fn accepts_actuation(self) -> bool {
        matches!(self, Direction::Actuator | Direction::Bidirectional)
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VssDataType {
    Bool,
    String,
    Int8,
    Int32,
    Int64,
    Uint8,
    Uint32,
    Uint64,
    Float,
    Double,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderConfig {
    #[serde(default = "default_provider_name")]
    pub provider_name: String,
    #[serde(default)]
    pub reconnect: ReconnectConfig,
    #[serde(default = "default_queue_capacity")]
    pub queue_capacity: usize,
    #[serde(default = "default_stale_after_ms")]
    pub stale_after_ms: u64,
    pub mappings: Vec<SignalMapping>,
}

impl ProviderConfig {
    pub fn load_json(path: impl AsRef<Path>) -> Result<Self, MwComProviderError> {
        let content = fs::read_to_string(path.as_ref()).map_err(|error| {
            MwComProviderError::Config(format!(
                "failed to read {}: {error}",
                path.as_ref().display()
            ))
        })?;
        let config: Self = serde_json::from_str(&content).map_err(|error| {
            MwComProviderError::Config(format!(
                "failed to parse {}: {error}",
                path.as_ref().display()
            ))
        })?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), MwComProviderError> {
        if self.mappings.is_empty() {
            return Err(MwComProviderError::Config(
                "at least one VSS mapping is required".into(),
            ));
        }

        let mut signal_ids = HashSet::new();
        let mut mw_com_bindings = HashSet::new();
        for mapping in &self.mappings {
            // Validate the config before any middleware connection is opened so
            // startup failures point to the mapping file rather than LoLa state.
            if mapping.vss_path.trim().is_empty() {
                return Err(MwComProviderError::Config(
                    "mapping has an empty VSS path".into(),
                ));
            }
            if mapping.service.trim().is_empty()
                || mapping.instance.trim().is_empty()
                || mapping.member.trim().is_empty()
            {
                return Err(MwComProviderError::Config(format!(
                    "mapping for {} has an incomplete mw::com binding",
                    mapping.vss_path
                )));
            }
            if !signal_ids.insert(mapping.signal_id) {
                return Err(MwComProviderError::Config(format!(
                    "duplicate signal id {} in mapping config",
                    mapping.signal_id
                )));
            }
            if !mw_com_bindings.insert((
                mapping.service.as_str(),
                mapping.instance.as_str(),
                mapping.member.as_str(),
            )) {
                return Err(MwComProviderError::Config(format!(
                    "duplicate mw::com binding {}:{}:{} in mapping config",
                    mapping.service, mapping.instance, mapping.member
                )));
            }
            if mapping.cycle_time_ms == 0 {
                return Err(MwComProviderError::Config(format!(
                    "mapping for {} must use a non-zero cycle_time_ms",
                    mapping.vss_path
                )));
            }
            if !mapping.scale.is_finite() || !mapping.offset.is_finite() {
                return Err(MwComProviderError::Config(format!(
                    "mapping for {} has a non-finite scale or offset",
                    mapping.vss_path
                )));
            }
            if mapping.min.is_some_and(|min| !min.is_finite())
                || mapping.max.is_some_and(|max| !max.is_finite())
            {
                return Err(MwComProviderError::Config(format!(
                    "mapping for {} has a non-finite min or max",
                    mapping.vss_path
                )));
            }
            if let (Some(min), Some(max)) = (mapping.min, mapping.max) {
                if min > max {
                    return Err(MwComProviderError::Config(format!(
                        "mapping for {} has min {min} greater than max {max}",
                        mapping.vss_path
                    )));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SignalMapping {
    // The generated config contains stable IDs. The composition binary rewrites
    // them to Databroker runtime IDs after seeding metadata.
    pub signal_id: i32,
    pub vss_path: String,
    pub datatype: VssDataType,
    // service/instance/member identify the exact generated S-Core binding.
    pub service: String,
    pub instance: String,
    pub member: String,
    #[serde(default)]
    pub field: Option<String>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default = "default_scale")]
    pub scale: f64,
    #[serde(default)]
    pub offset: f64,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub required_quality: SignalQuality,
    #[serde(default = "default_direction")]
    pub direction: Direction,
    #[serde(default = "default_cycle_time_ms")]
    pub cycle_time_ms: u32,
}

fn default_provider_name() -> String {
    "mw_com_provider".to_string()
}

fn default_queue_capacity() -> usize {
    256
}

fn default_stale_after_ms() -> u64 {
    1_000
}

fn default_scale() -> f64 {
    1.0
}

fn default_direction() -> Direction {
    Direction::Datapoint
}

fn default_cycle_time_ms() -> u32 {
    100
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping(signal_id: i32) -> SignalMapping {
        SignalMapping {
            signal_id,
            vss_path: "Vehicle.Speed".into(),
            datatype: VssDataType::Double,
            service: "VehicleSpeedService".into(),
            instance: "front".into(),
            member: "speed".into(),
            field: None,
            unit: Some("km/h".into()),
            scale: 1.0,
            offset: 0.0,
            min: Some(0.0),
            max: Some(300.0),
            required_quality: SignalQuality::Valid,
            direction: Direction::Datapoint,
            cycle_time_ms: 100,
        }
    }

    fn config(mappings: Vec<SignalMapping>) -> ProviderConfig {
        ProviderConfig {
            provider_name: "test".into(),
            reconnect: Default::default(),
            queue_capacity: 4,
            stale_after_ms: 1_000,
            mappings,
        }
    }

    #[test]
    fn rejects_empty_mapping_list() {
        let error = config(vec![]).validate().unwrap_err();

        assert!(error.to_string().contains("at least one VSS mapping"));
    }

    #[test]
    fn rejects_empty_vss_path() {
        let mut mapping = mapping(1);
        mapping.vss_path = " ".into();

        let error = config(vec![mapping]).validate().unwrap_err();

        assert!(error.to_string().contains("empty VSS path"));
    }

    #[test]
    fn rejects_incomplete_mw_com_binding() {
        let mut mapping = mapping(1);
        mapping.member = "".into();

        let error = config(vec![mapping]).validate().unwrap_err();

        assert!(error.to_string().contains("incomplete mw::com binding"));
    }

    #[test]
    fn rejects_duplicate_signal_ids() {
        let error = config(vec![mapping(1), mapping(1)]).validate().unwrap_err();

        assert!(error.to_string().contains("duplicate signal id 1"));
    }

    #[test]
    fn rejects_duplicate_mw_com_bindings() {
        let mut second_mapping = mapping(2);
        second_mapping.vss_path = "Vehicle.Cabin.HVAC.AmbientAirTemperature".into();

        let error = config(vec![mapping(1), second_mapping])
            .validate()
            .unwrap_err();

        assert!(error.to_string().contains("duplicate mw::com binding"));
    }

    #[test]
    fn rejects_zero_cycle_time() {
        let mut mapping = mapping(1);
        mapping.cycle_time_ms = 0;

        let error = config(vec![mapping]).validate().unwrap_err();

        assert!(error.to_string().contains("non-zero cycle_time_ms"));
    }

    #[test]
    fn rejects_non_finite_scale() {
        let mut mapping = mapping(1);
        mapping.scale = f64::INFINITY;

        let error = config(vec![mapping]).validate().unwrap_err();

        assert!(error.to_string().contains("non-finite scale or offset"));
    }

    #[test]
    fn rejects_min_greater_than_max() {
        let mut mapping = mapping(1);
        mapping.min = Some(300.0);
        mapping.max = Some(0.0);

        let error = config(vec![mapping]).validate().unwrap_err();

        assert!(error.to_string().contains("min 300 greater than max 0"));
    }
}
