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

use crate::config::{ProviderConfig, SignalMapping, VssDataType};
use crate::error::MwComProviderError;
use crate::transport::{MwComMessage, MwComValue};
use databroker::broker::{ActuationChange, DataValue, Datapoint, EntryUpdate};
use databroker::types::{SignalId, TimeInterval};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
struct MwComKey {
    service: String,
    instance: String,
    member: String,
}

impl MwComKey {
    fn new(service: &str, instance: &str, member: &str) -> Self {
        Self {
            service: service.to_string(),
            instance: instance.to_string(),
            member: member.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MwComMapper {
    config: ProviderConfig,
    by_member: HashMap<MwComKey, SignalMapping>,
    by_signal_id: HashMap<i32, SignalMapping>,
}

impl MwComMapper {
    pub fn new(config: ProviderConfig) -> Result<Self, MwComProviderError> {
        config.validate()?;
        let mut by_member = HashMap::new();
        let mut by_signal_id = HashMap::new();

        // Build both lookup directions once. The receive path indexes by
        // mw::com binding, while the actuation path indexes by Databroker ID.
        for mapping in &config.mappings {
            by_member.insert(
                MwComKey::new(&mapping.service, &mapping.instance, &mapping.member),
                mapping.clone(),
            );
            by_signal_id.insert(mapping.signal_id, mapping.clone());
        }

        Ok(Self {
            config,
            by_member,
            by_signal_id,
        })
    }

    pub fn config(&self) -> &ProviderConfig {
        &self.config
    }

    pub fn subscriptions(&self) -> impl Iterator<Item = &SignalMapping> {
        self.config
            .mappings
            .iter()
            .filter(|mapping| mapping.direction.receives_datapoints())
    }

    pub fn signal_intervals(&self) -> HashMap<SignalId, TimeInterval> {
        self.config
            .mappings
            .iter()
            .filter(|mapping| mapping.direction.receives_datapoints())
            .map(|mapping| {
                (
                    SignalId::new(mapping.signal_id),
                    TimeInterval::new(mapping.cycle_time_ms),
                )
            })
            .collect()
    }

    pub fn actuator_signal_ids(&self) -> Vec<i32> {
        self.config
            .mappings
            .iter()
            .filter(|mapping| mapping.direction.accepts_actuation())
            .map(|mapping| mapping.signal_id)
            .collect()
    }

    pub fn message_to_update(
        &self,
        message: &MwComMessage,
    ) -> Result<(i32, EntryUpdate), MwComProviderError> {
        let key = MwComKey::new(&message.service, &message.instance, &message.member);
        let mapping = self.by_member.get(&key).ok_or_else(|| {
            MwComProviderError::Mapping(format!(
                "no mapping for {}:{}:{}",
                message.service, message.instance, message.member
            ))
        })?;

        if !mapping.direction.receives_datapoints() {
            return Err(MwComProviderError::Mapping(format!(
                "mapping for {} does not accept datapoints",
                mapping.vss_path
            )));
        }

        qualify_message(
            message,
            mapping,
            Duration::from_millis(self.config.stale_after_ms),
        )?;
        // Convert into the VSS-facing value domain before updating Databroker:
        // this is where scaling, offset, bounds, and type checks are enforced.
        let value = convert_value(&message.value, mapping)?;
        let datapoint = Datapoint {
            ts: SystemTime::now(),
            source_ts: message.source_timestamp,
            value,
        };

        Ok((
            mapping.signal_id,
            EntryUpdate {
                datapoint: Some(datapoint),
                ..Default::default()
            },
        ))
    }

    pub fn actuation_to_message(
        &self,
        change: &ActuationChange,
    ) -> Result<MwComMessage, MwComProviderError> {
        let mapping = self.by_signal_id.get(&change.id).ok_or_else(|| {
            MwComProviderError::Mapping(format!("no mapping for signal id {}", change.id))
        })?;

        if !mapping.direction.accepts_actuation() {
            return Err(MwComProviderError::Mapping(format!(
                "mapping for {} does not accept actuation",
                mapping.vss_path
            )));
        }

        Ok(MwComMessage {
            service: mapping.service.clone(),
            instance: mapping.instance.clone(),
            member: mapping.member.clone(),
            value: data_value_to_mw_com(&change.data_value, mapping)?,
            source_timestamp: Some(SystemTime::now()),
            quality: mapping.required_quality,
        })
    }
}

fn qualify_message(
    message: &MwComMessage,
    mapping: &SignalMapping,
    stale_after: Duration,
) -> Result<(), MwComProviderError> {
    if message.quality < mapping.required_quality {
        return Err(MwComProviderError::Quality(format!(
            "{} quality {:?} is lower than required {:?}",
            mapping.vss_path, message.quality, mapping.required_quality
        )));
    }

    if let Some(source_timestamp) = message.source_timestamp {
        if let Ok(age) = SystemTime::now().duration_since(source_timestamp) {
            if age > stale_after {
                return Err(MwComProviderError::Quality(format!(
                    "{} sample is stale: {:?} > {:?}",
                    mapping.vss_path, age, stale_after
                )));
            }
        }
    }

    Ok(())
}

fn convert_value(
    value: &MwComValue,
    mapping: &SignalMapping,
) -> Result<DataValue, MwComProviderError> {
    match mapping.datatype {
        VssDataType::Bool => match value {
            MwComValue::Bool(value) => Ok(DataValue::Bool(*value)),
            _ => type_error(value, mapping),
        },
        VssDataType::String => match value {
            MwComValue::String(value) => Ok(DataValue::String(value.clone())),
            _ => type_error(value, mapping),
        },
        VssDataType::Int8 => numeric_i64(value, mapping).and_then(|value| {
            i32::try_from(i8::try_from(value).map_err(|_| {
                MwComProviderError::Mapping(format!("{} is out of Int8 range", mapping.vss_path))
            })?)
            .map(DataValue::Int32)
            .map_err(|_| {
                MwComProviderError::Mapping(format!("{} is out of Int32 range", mapping.vss_path))
            })
        }),
        VssDataType::Int32 => numeric_i64(value, mapping).and_then(|value| {
            i32::try_from(value).map(DataValue::Int32).map_err(|_| {
                MwComProviderError::Mapping(format!("{} is out of Int32 range", mapping.vss_path))
            })
        }),
        VssDataType::Int64 => numeric_i64(value, mapping).map(DataValue::Int64),
        VssDataType::Uint8 => numeric_u64(value, mapping).and_then(|value| {
            u32::try_from(u8::try_from(value).map_err(|_| {
                MwComProviderError::Mapping(format!("{} is out of Uint8 range", mapping.vss_path))
            })?)
            .map(DataValue::Uint32)
            .map_err(|_| {
                MwComProviderError::Mapping(format!("{} is out of Uint32 range", mapping.vss_path))
            })
        }),
        VssDataType::Uint32 => numeric_u64(value, mapping).and_then(|value| {
            u32::try_from(value).map(DataValue::Uint32).map_err(|_| {
                MwComProviderError::Mapping(format!("{} is out of Uint32 range", mapping.vss_path))
            })
        }),
        VssDataType::Uint64 => numeric_u64(value, mapping).map(DataValue::Uint64),
        VssDataType::Float => {
            numeric_f64(value, mapping).map(|value| DataValue::Float(value as f32))
        }
        VssDataType::Double => numeric_f64(value, mapping).map(DataValue::Double),
    }
}

fn data_value_to_mw_com(
    value: &DataValue,
    mapping: &SignalMapping,
) -> Result<MwComValue, MwComProviderError> {
    match (mapping.datatype, value) {
        (VssDataType::Bool, DataValue::Bool(value)) => Ok(MwComValue::Bool(*value)),
        (VssDataType::String, DataValue::String(value)) => Ok(MwComValue::String(value.clone())),
        (VssDataType::Int8 | VssDataType::Int32 | VssDataType::Int64, DataValue::Int32(value)) => {
            Ok(MwComValue::I64(*value as i64))
        }
        (VssDataType::Int8 | VssDataType::Int32 | VssDataType::Int64, DataValue::Int64(value)) => {
            Ok(MwComValue::I64(*value))
        }
        (
            VssDataType::Uint8 | VssDataType::Uint32 | VssDataType::Uint64,
            DataValue::Uint32(value),
        ) => Ok(MwComValue::U64(*value as u64)),
        (
            VssDataType::Uint8 | VssDataType::Uint32 | VssDataType::Uint64,
            DataValue::Uint64(value),
        ) => Ok(MwComValue::U64(*value)),
        (VssDataType::Float | VssDataType::Double, DataValue::Float(value)) => {
            Ok(MwComValue::F64(*value as f64))
        }
        (VssDataType::Float | VssDataType::Double, DataValue::Double(value)) => {
            Ok(MwComValue::F64(*value))
        }
        _ => Err(MwComProviderError::Mapping(format!(
            "actuation value for {} does not match {:?}",
            mapping.vss_path, mapping.datatype
        ))),
    }
}

fn numeric_i64(value: &MwComValue, mapping: &SignalMapping) -> Result<i64, MwComProviderError> {
    let value = numeric_f64(value, mapping)?;
    if value.fract() != 0.0 {
        return Err(MwComProviderError::Mapping(format!(
            "{} expected integer value",
            mapping.vss_path
        )));
    }
    Ok(value as i64)
}

fn numeric_u64(value: &MwComValue, mapping: &SignalMapping) -> Result<u64, MwComProviderError> {
    let value = numeric_i64(value, mapping)?;
    u64::try_from(value).map_err(|_| {
        MwComProviderError::Mapping(format!("{} expected unsigned value", mapping.vss_path))
    })
}

fn numeric_f64(value: &MwComValue, mapping: &SignalMapping) -> Result<f64, MwComProviderError> {
    let raw = match value {
        MwComValue::I64(value) => *value as f64,
        MwComValue::U64(value) => *value as f64,
        MwComValue::F64(value) => *value,
        _ => return type_error(value, mapping),
    };

    // Scaling belongs to the mapping, not the generated S-Core payload. This
    // lets the middleware schema and Databroker VSS value domain evolve apart.
    let scaled = raw * mapping.scale + mapping.offset;
    if let Some(min) = mapping.min {
        if scaled < min {
            return Err(MwComProviderError::Mapping(format!(
                "{} value {scaled} below min {min}",
                mapping.vss_path
            )));
        }
    }
    if let Some(max) = mapping.max {
        if scaled > max {
            return Err(MwComProviderError::Mapping(format!(
                "{} value {scaled} above max {max}",
                mapping.vss_path
            )));
        }
    }
    Ok(scaled)
}

fn type_error<T>(value: &MwComValue, mapping: &SignalMapping) -> Result<T, MwComProviderError> {
    Err(MwComProviderError::Mapping(format!(
        "{} expected {:?}, got {:?}",
        mapping.vss_path, mapping.datatype, value
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Direction;
    use crate::quality::SignalQuality;

    fn mapping(signal_id: i32, direction: Direction, datatype: VssDataType) -> SignalMapping {
        SignalMapping {
            signal_id,
            vss_path: format!("Vehicle.Test.Signal{signal_id}"),
            datatype,
            service: "VehicleService".into(),
            instance: "front".into(),
            member: format!("signal{signal_id}"),
            field: None,
            unit: None,
            scale: 1.0,
            offset: 0.0,
            min: None,
            max: None,
            required_quality: SignalQuality::Valid,
            direction,
            cycle_time_ms: 100,
        }
    }

    fn mapper_with(mappings: Vec<SignalMapping>) -> MwComMapper {
        MwComMapper::new(ProviderConfig {
            provider_name: "test".into(),
            reconnect: Default::default(),
            queue_capacity: 4,
            stale_after_ms: 1_000,
            mappings,
        })
        .unwrap()
    }

    fn mapper() -> MwComMapper {
        let mut speed = mapping(1, Direction::Datapoint, VssDataType::Double);
        speed.vss_path = "Vehicle.Speed".into();
        speed.service = "VehicleSpeedService".into();
        speed.member = "speed".into();
        speed.unit = Some("km/h".into());
        speed.scale = 3.6;
        speed.min = Some(0.0);
        speed.max = Some(300.0);
        mapper_with(vec![speed])
    }

    fn message(member: &str, value: MwComValue) -> MwComMessage {
        MwComMessage {
            service: "VehicleService".into(),
            instance: "front".into(),
            member: member.into(),
            value,
            source_timestamp: Some(SystemTime::now()),
            quality: SignalQuality::Valid,
        }
    }

    #[test]
    fn converts_scaled_datapoint() {
        let mapper = mapper();
        let message = MwComMessage {
            service: "VehicleSpeedService".into(),
            instance: "front".into(),
            member: "speed".into(),
            value: MwComValue::F64(10.0),
            source_timestamp: Some(SystemTime::now()),
            quality: SignalQuality::Valid,
        };

        let (_id, update) = mapper.message_to_update(&message).unwrap();
        assert_eq!(update.path, None);
        assert_eq!(update.datapoint.unwrap().value, DataValue::Double(36.0));
    }

    #[test]
    fn rejects_unmapped_mw_com_member() {
        let mapper = mapper();
        let message = MwComMessage {
            service: "OtherService".into(),
            instance: "front".into(),
            member: "speed".into(),
            value: MwComValue::F64(10.0),
            source_timestamp: Some(SystemTime::now()),
            quality: SignalQuality::Valid,
        };

        let error = mapper.message_to_update(&message).unwrap_err();

        assert!(error
            .to_string()
            .contains("no mapping for OtherService:front:speed"));
    }

    #[test]
    fn rejects_datapoint_for_actuator_only_mapping() {
        let mapper = mapper_with(vec![mapping(1, Direction::Actuator, VssDataType::Bool)]);

        let error = mapper
            .message_to_update(&message("signal1", MwComValue::Bool(true)))
            .unwrap_err();

        assert!(error.to_string().contains("does not accept datapoints"));
    }

    #[test]
    fn rejects_low_quality_datapoint() {
        let mapper = mapper();
        let message = MwComMessage {
            service: "VehicleSpeedService".into(),
            instance: "front".into(),
            member: "speed".into(),
            value: MwComValue::F64(10.0),
            source_timestamp: Some(SystemTime::now()),
            quality: SignalQuality::Degraded,
        };

        let error = mapper.message_to_update(&message).unwrap_err();

        assert!(error.to_string().contains("quality Degraded is lower"));
    }

    #[test]
    fn rejects_stale_datapoint() {
        let mapper = mapper();
        let message = MwComMessage {
            service: "VehicleSpeedService".into(),
            instance: "front".into(),
            member: "speed".into(),
            value: MwComValue::F64(10.0),
            source_timestamp: Some(SystemTime::now() - Duration::from_millis(2_000)),
            quality: SignalQuality::Valid,
        };

        let error = mapper.message_to_update(&message).unwrap_err();

        assert!(error.to_string().contains("sample is stale"));
    }

    #[test]
    fn converts_uint8_datapoint_to_databroker_uint32() {
        let mapper = mapper_with(vec![mapping(1, Direction::Datapoint, VssDataType::Uint8)]);

        let (_id, update) = mapper
            .message_to_update(&message("signal1", MwComValue::U64(255)))
            .unwrap();

        assert_eq!(update.datapoint.unwrap().value, DataValue::Uint32(255));
    }

    #[test]
    fn rejects_out_of_range_int8_datapoint() {
        let mapper = mapper_with(vec![mapping(1, Direction::Datapoint, VssDataType::Int8)]);

        let error = mapper
            .message_to_update(&message("signal1", MwComValue::I64(128)))
            .unwrap_err();

        assert!(error.to_string().contains("out of Int8 range"));
    }

    #[test]
    fn converts_actuation_to_mw_com_message() {
        let mapper = mapper_with(vec![mapping(1, Direction::Actuator, VssDataType::Bool)]);

        let message = mapper
            .actuation_to_message(&ActuationChange {
                id: 1,
                data_value: DataValue::Bool(true),
            })
            .unwrap();

        assert_eq!(message.member, "signal1");
        assert_eq!(message.value, MwComValue::Bool(true));
    }

    #[test]
    fn rejects_actuation_for_datapoint_only_mapping() {
        let mapper = mapper_with(vec![mapping(1, Direction::Datapoint, VssDataType::Bool)]);

        let error = mapper
            .actuation_to_message(&ActuationChange {
                id: 1,
                data_value: DataValue::Bool(true),
            })
            .unwrap_err();

        assert!(error.to_string().contains("does not accept actuation"));
    }
}
