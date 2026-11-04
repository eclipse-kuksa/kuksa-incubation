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

use crate::config::ProviderConfig;
use crate::error::MwComProviderError;
use crate::generated;
use crate::transport::{MwComMessage, MwComTransport};
use async_trait::async_trait;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ScoreBinding<'a> {
    pub service_name: &'a str,
    pub instance_name: &'a str,
    pub instance_specifier: &'a str,
    pub member_name: &'a str,
    pub element_kind: &'a str,
    pub payload_type: &'a str,
}

impl<'a> From<&'a generated::GenericMwComElement> for ScoreBinding<'a> {
    fn from(element: &'a generated::GenericMwComElement) -> Self {
        Self {
            service_name: element.service_name,
            instance_name: element.instance_name,
            instance_specifier: element.instance_specifier,
            member_name: element.member_name,
            element_kind: element.element_kind,
            payload_type: element.payload_type,
        }
    }
}

pub fn generated_score_bindings() -> impl Iterator<Item = ScoreBinding<'static>> {
    generated::GENERATED_GENERIC_MW_COM_ELEMENTS
        .iter()
        .map(ScoreBinding::from)
}

pub fn find_score_binding(
    service: &str,
    instance: &str,
    member: &str,
) -> Option<&'static generated::GenericMwComElement> {
    // Runtime config can use either the generated instance name or the LoLa
    // instance specifier. Accept both while resolving to one exact binding.
    generated::GENERATED_GENERIC_MW_COM_ELEMENTS
        .iter()
        .find(|element| {
            element.service_name == service
                && (element.instance_name == instance || element.instance_specifier == instance)
                && element.member_name == member
        })
}

pub fn validate_score_bindings(config: &ProviderConfig) -> Result<(), MwComProviderError> {
    validate_score_bindings_with(config, find_score_binding)
}

fn validate_score_bindings_with(
    config: &ProviderConfig,
    mut find_binding: impl FnMut(&str, &str, &str) -> Option<&'static generated::GenericMwComElement>,
) -> Result<(), MwComProviderError> {
    for mapping in &config.mappings {
        // Fail fast during process startup instead of discovering missing
        // generated bindings after the provider has already connected.
        let element = find_binding(&mapping.service, &mapping.instance, &mapping.member)
            .ok_or_else(|| {
                MwComProviderError::Config(format!(
                    "mapping {} references missing generated SCORE binding {}:{}:{}",
                    mapping.vss_path, mapping.service, mapping.instance, mapping.member
                ))
            })?;

        if element.instance_specifier.is_empty() {
            return Err(MwComProviderError::Config(format!(
                "mapping {} uses generated SCORE binding {}:{}:{} without an instance_specifier",
                mapping.vss_path, mapping.service, mapping.instance, mapping.member
            )));
        }
    }

    Ok(())
}

#[async_trait]
pub trait ScoreRuntimeAdapter: Send + Sync {
    async fn connect(&mut self) -> Result<(), MwComProviderError>;
    async fn disconnect(&mut self) -> Result<(), MwComProviderError>;

    async fn find_service(&self, binding: ScoreBinding<'_>) -> Result<bool, MwComProviderError>;

    async fn subscribe(&mut self, binding: ScoreBinding<'_>) -> Result<(), MwComProviderError>;

    async fn unsubscribe(&mut self, binding: ScoreBinding<'_>) -> Result<(), MwComProviderError>;

    async fn recv(&mut self) -> Result<Option<MwComMessage>, MwComProviderError>;

    async fn send_actuation(
        &mut self,
        binding: ScoreBinding<'_>,
        message: MwComMessage,
    ) -> Result<(), MwComProviderError>;
}

pub struct ScoreBindingTransport<A> {
    adapter: A,
    subscribed: HashSet<(&'static str, &'static str, &'static str)>,
}

impl<A> ScoreBindingTransport<A> {
    pub fn new(adapter: A) -> Self {
        Self {
            adapter,
            subscribed: HashSet::new(),
        }
    }

    pub fn adapter(&self) -> &A {
        &self.adapter
    }

    pub fn adapter_mut(&mut self) -> &mut A {
        &mut self.adapter
    }

    fn resolve(
        service: &str,
        instance: &str,
        member: &str,
    ) -> Result<&'static generated::GenericMwComElement, MwComProviderError> {
        find_score_binding(service, instance, member).ok_or_else(|| {
            MwComProviderError::Transport(format!(
                "no generated SCORE binding for {service}:{instance}:{member}"
            ))
        })
    }
}

#[async_trait]
impl<A> MwComTransport for ScoreBindingTransport<A>
where
    A: ScoreRuntimeAdapter,
{
    async fn connect(&mut self) -> Result<(), MwComProviderError> {
        self.adapter.connect().await
    }

    async fn disconnect(&mut self) -> Result<(), MwComProviderError> {
        let subscriptions = self.subscribed.drain().collect::<Vec<_>>();
        for (service, instance, member) in subscriptions {
            // Unsubscribe exact generated bindings; the same member name can be
            // present on multiple S-Core instances.
            let binding = Self::resolve(service, instance, member)?;
            self.adapter.unsubscribe(binding.into()).await?;
        }
        self.adapter.disconnect().await
    }

    async fn subscribe(
        &mut self,
        service: &str,
        instance: &str,
        member: &str,
    ) -> Result<(), MwComProviderError> {
        let element = Self::resolve(service, instance, member)?;
        let binding = ScoreBinding::from(element);
        let key = (
            element.service_name,
            element.instance_name,
            element.member_name,
        );

        if self.subscribed.contains(&key) {
            return Err(MwComProviderError::Transport(format!(
                "already subscribed to {service}:{instance}:{member}"
            )));
        }

        if !self.adapter.find_service(binding).await? {
            return Err(MwComProviderError::ServiceUnavailable {
                service: binding.service_name.to_string(),
                instance: binding.instance_specifier.to_string(),
            });
        }

        self.adapter.subscribe(binding).await?;
        self.subscribed.insert(key);
        Ok(())
    }

    async fn recv(&mut self) -> Result<MwComMessage, MwComProviderError> {
        let message = self
            .adapter
            .recv()
            .await?
            .ok_or(MwComProviderError::NoData)?;
        Ok(message)
    }

    async fn send_actuation(&mut self, message: MwComMessage) -> Result<(), MwComProviderError> {
        let element = Self::resolve(&message.service, &message.instance, &message.member)?;
        self.adapter
            .send_actuation(ScoreBinding::from(element), message)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Direction, SignalMapping, VssDataType};
    use crate::quality::SignalQuality;
    use crate::transport::MwComValue;
    use std::sync::{Arc, Mutex};

    static TEST_RECEIVE_INSTANCE_A: generated::GenericMwComElement =
        generated::GenericMwComElement {
            service_name: "VehicleDynamicsService",
            instance_name: "receive_a",
            instance_specifier: "/Vehicle/A/Instance",
            member_name: "speed",
            element_kind: "event",
            payload_type: "SpeedSample",
            sample_size: 16,
            sample_alignment: 8,
            has_serialized_format: false,
        };

    static TEST_RECEIVE_INSTANCE_B: generated::GenericMwComElement =
        generated::GenericMwComElement {
            service_name: "VehicleDynamicsService",
            instance_name: "receive_b",
            instance_specifier: "/Vehicle/B/Instance",
            member_name: "speed",
            element_kind: "event",
            payload_type: "SpeedSample",
            sample_size: 16,
            sample_alignment: 8,
            has_serialized_format: false,
        };

    static TEST_ACTUATOR_INSTANCE_A: generated::GenericMwComElement =
        generated::GenericMwComElement {
            service_name: "VehicleDynamicsService",
            instance_name: "actuator_a",
            instance_specifier: "/Vehicle/A/Actuator",
            member_name: "target_speed",
            element_kind: "event",
            payload_type: "TargetSpeed",
            sample_size: 8,
            sample_alignment: 8,
            has_serialized_format: false,
        };

    static TEST_ACTUATOR_INSTANCE_B: generated::GenericMwComElement =
        generated::GenericMwComElement {
            service_name: "VehicleDynamicsService",
            instance_name: "actuator_b",
            instance_specifier: "/Vehicle/B/Actuator",
            member_name: "target_speed",
            element_kind: "event",
            payload_type: "TargetSpeed",
            sample_size: 8,
            sample_alignment: 8,
            has_serialized_format: false,
        };

    struct RecordingAdapter {
        calls: Arc<Mutex<Vec<String>>>,
        next_message: Option<MwComMessage>,
        service_available: bool,
    }

    impl Default for RecordingAdapter {
        fn default() -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                next_message: None,
                service_available: true,
            }
        }
    }

    #[async_trait]
    impl ScoreRuntimeAdapter for RecordingAdapter {
        async fn connect(&mut self) -> Result<(), MwComProviderError> {
            self.calls.lock().unwrap().push("connect".into());
            Ok(())
        }

        async fn disconnect(&mut self) -> Result<(), MwComProviderError> {
            self.calls.lock().unwrap().push("disconnect".into());
            Ok(())
        }

        async fn find_service(
            &self,
            binding: ScoreBinding<'_>,
        ) -> Result<bool, MwComProviderError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("find:{}", binding.instance_specifier));
            Ok(self.service_available)
        }

        async fn subscribe(&mut self, binding: ScoreBinding<'_>) -> Result<(), MwComProviderError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("subscribe:{}", binding.member_name));
            Ok(())
        }

        async fn unsubscribe(
            &mut self,
            binding: ScoreBinding<'_>,
        ) -> Result<(), MwComProviderError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("unsubscribe:{}", binding.member_name));
            Ok(())
        }

        async fn recv(&mut self) -> Result<Option<MwComMessage>, MwComProviderError> {
            Ok(self.next_message.take())
        }

        async fn send_actuation(
            &mut self,
            binding: ScoreBinding<'_>,
            _message: MwComMessage,
        ) -> Result<(), MwComProviderError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("send:{}", binding.member_name));
            Ok(())
        }
    }

    fn provider_config() -> ProviderConfig {
        ProviderConfig {
            provider_name: "score".into(),
            reconnect: Default::default(),
            queue_capacity: 4,
            stale_after_ms: 1_000,
            mappings: vec![SignalMapping {
                signal_id: 42,
                vss_path: "Vehicle.Speed".into(),
                datatype: VssDataType::Double,
                service: "VehicleDynamicsService".into(),
                instance: "front_vehicle".into(),
                member: "speed".into(),
                field: None,
                unit: Some("km/h".into()),
                scale: 1.0,
                offset: 0.0,
                min: None,
                max: None,
                required_quality: SignalQuality::Valid,
                direction: Direction::Datapoint,
                cycle_time_ms: 100,
            }],
        }
    }

    fn mapped_signal(
        signal_id: i32,
        vss_path: &str,
        instance: &str,
        member: &str,
        direction: Direction,
    ) -> SignalMapping {
        SignalMapping {
            signal_id,
            vss_path: vss_path.into(),
            datatype: VssDataType::Double,
            service: "VehicleDynamicsService".into(),
            instance: instance.into(),
            member: member.into(),
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

    fn test_generated_binding(
        service: &str,
        instance: &str,
        member: &str,
    ) -> Option<&'static generated::GenericMwComElement> {
        match (service, instance, member) {
            ("VehicleDynamicsService", "receive_a", "speed") => Some(&TEST_RECEIVE_INSTANCE_A),
            ("VehicleDynamicsService", "receive_b", "speed") => Some(&TEST_RECEIVE_INSTANCE_B),
            ("VehicleDynamicsService", "actuator_a", "target_speed") => {
                Some(&TEST_ACTUATOR_INSTANCE_A)
            }
            ("VehicleDynamicsService", "actuator_b", "target_speed") => {
                Some(&TEST_ACTUATOR_INSTANCE_B)
            }
            _ => None,
        }
    }

    #[test]
    fn resolves_generated_binding_by_provider_instance_or_score_specifier() {
        let by_provider =
            find_score_binding("VehicleDynamicsService", "front_vehicle", "speed").unwrap();
        let by_score = find_score_binding(
            "VehicleDynamicsService",
            "/Vehicle/Service1/Instance",
            "speed",
        )
        .unwrap();

        assert_eq!(by_provider, by_score);
        assert_eq!(by_provider.instance_specifier, "/Vehicle/Service1/Instance");
    }

    #[test]
    fn validates_provider_config_against_generated_score_bindings() {
        validate_score_bindings(&provider_config()).unwrap();
    }

    #[test]
    fn allows_same_receive_member_on_different_score_instances() {
        let config = ProviderConfig {
            mappings: vec![
                mapped_signal(
                    1,
                    "Vehicle.Cabin.Row1.Speed",
                    "receive_a",
                    "speed",
                    Direction::Datapoint,
                ),
                mapped_signal(
                    2,
                    "Vehicle.Cabin.Row2.Speed",
                    "receive_b",
                    "speed",
                    Direction::Datapoint,
                ),
            ],
            ..provider_config()
        };

        validate_score_bindings_with(&config, test_generated_binding).unwrap();
    }

    #[test]
    fn allows_same_actuator_member_on_different_score_instances() {
        let config = ProviderConfig {
            mappings: vec![
                mapped_signal(
                    1,
                    "Vehicle.ADAS.Row1.TargetSpeed",
                    "actuator_a",
                    "target_speed",
                    Direction::Actuator,
                ),
                mapped_signal(
                    2,
                    "Vehicle.ADAS.Row2.TargetSpeed",
                    "actuator_b",
                    "target_speed",
                    Direction::Actuator,
                ),
            ],
            ..provider_config()
        };

        validate_score_bindings_with(&config, test_generated_binding).unwrap();
    }

    #[tokio::test]
    async fn transport_delegates_to_runtime_adapter_with_generated_binding() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let adapter = RecordingAdapter {
            calls: Arc::clone(&calls),
            next_message: Some(MwComMessage {
                service: "VehicleDynamicsService".into(),
                instance: "front_vehicle".into(),
                member: "speed".into(),
                value: MwComValue::F64(12.5),
                source_timestamp: None,
                quality: SignalQuality::Valid,
            }),
            service_available: true,
        };
        let mut transport = ScoreBindingTransport::new(adapter);

        transport.connect().await.unwrap();
        transport
            .subscribe("VehicleDynamicsService", "front_vehicle", "speed")
            .await
            .unwrap();
        assert_eq!(transport.recv().await.unwrap().value, MwComValue::F64(12.5));
        transport
            .send_actuation(MwComMessage {
                service: "VehicleDynamicsService".into(),
                instance: "front_vehicle_actuator".into(),
                member: "target_speed".into(),
                value: MwComValue::F64(80.0),
                source_timestamp: None,
                quality: SignalQuality::Valid,
            })
            .await
            .unwrap();
        transport.disconnect().await.unwrap();

        assert_eq!(
            calls.lock().unwrap().as_slice(),
            [
                "connect",
                "find:/Vehicle/Service1/Instance",
                "subscribe:speed",
                "send:target_speed",
                "unsubscribe:speed",
                "disconnect",
            ]
        );
    }

    #[tokio::test]
    async fn transport_reports_unavailable_score_service() {
        let mut transport = ScoreBindingTransport::new(RecordingAdapter {
            service_available: false,
            ..Default::default()
        });

        let error = transport
            .subscribe("VehicleDynamicsService", "front_vehicle", "speed")
            .await
            .unwrap_err();

        match error {
            MwComProviderError::ServiceUnavailable { service, instance } => {
                assert_eq!(service, "VehicleDynamicsService");
                assert_eq!(instance, "/Vehicle/Service1/Instance");
            }
            other => panic!("unexpected error: {other}"),
        }
    }
}
