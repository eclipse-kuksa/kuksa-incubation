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
use crate::quality::SignalQuality;
use crate::score_bindings::{ScoreBinding, ScoreRuntimeAdapter};
use crate::transport::{MwComMessage, MwComValue};
use async_trait::async_trait;
use score_com::{
    Builder, FindServiceSpecifier, InstanceSpecifier, Interface, LolaRuntimeBuilderImpl,
    LolaRuntimeImpl, OfferedProducer, Producer, Publisher, Runtime, RuntimeBuilder,
    SampleContainer, SampleMaybeUninit, SampleMut, ServiceDiscovery, Subscriber, Subscription,
};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const SCORE_DEFAULT_SUBSCRIPTION_QUEUE_CAPACITY: usize = 3;
const SCORE_DEFAULT_RECEIVE_BATCH_SIZE: usize = 16;

type LolaRuntime = LolaRuntimeImpl;

type VehicleDynamicsServiceInterfaceAlias =
    crate::mw_com_provider_interfaces::vehicle::VehicleDynamicsServiceInterface;
type VehicleDynamicsServiceProducer =
    <VehicleDynamicsServiceInterfaceAlias as Interface>::Producer<LolaRuntime>;
type VehicleDynamicsServiceOfferedProducer =
    <VehicleDynamicsServiceProducer as Producer<LolaRuntime>>::OfferedProducer;
type VehicleDynamicsServiceSpeedSubscription = <<LolaRuntime as Runtime>::Subscriber<
    crate::mw_com_provider_types::vehicle::SpeedSample,
> as Subscriber<
    crate::mw_com_provider_types::vehicle::SpeedSample,
    LolaRuntime,
>>::Subscription;
type VehicleDynamicsServiceCruiseControlEnabledSubscription = <<LolaRuntime as Runtime>::Subscriber<
    crate::mw_com_provider_types::vehicle::CruiseControlEnabled,
> as Subscriber<
    crate::mw_com_provider_types::vehicle::CruiseControlEnabled,
    LolaRuntime,
>>::Subscription;

#[allow(dead_code)]
enum GeneratedReceiveSubscription {
    NoReceive,
    VehicleDynamicsServiceSpeed {
        instance_name: String,
        instance_specifier: String,
        subscription: VehicleDynamicsServiceSpeedSubscription,
    },
    VehicleDynamicsServiceCruiseControlEnabled {
        instance_name: String,
        instance_specifier: String,
        subscription: VehicleDynamicsServiceCruiseControlEnabledSubscription,
    },
}

impl GeneratedReceiveSubscription {
    fn matches(&self, service_name: &str, instance_specifier: &str, member_name: &str) -> bool {
        match self {
            Self::NoReceive => false,
            Self::VehicleDynamicsServiceSpeed {
                instance_specifier: subscribed_instance,
                ..
            } => {
                service_name == "VehicleDynamicsService"
                    && subscribed_instance == instance_specifier
                    && member_name == "speed"
            }
            Self::VehicleDynamicsServiceCruiseControlEnabled {
                instance_specifier: subscribed_instance,
                ..
            } => {
                service_name == "VehicleDynamicsService"
                    && subscribed_instance == instance_specifier
                    && member_name == "cruise_control_enabled"
            }
        }
    }

    fn unsubscribe(self) {
        match self {
            Self::NoReceive => {}
            Self::VehicleDynamicsServiceSpeed { subscription, .. } => {
                let _ = subscription.unsubscribe();
            }
            Self::VehicleDynamicsServiceCruiseControlEnabled { subscription, .. } => {
                let _ = subscription.unsubscribe();
            }
        }
    }

    fn try_receive(
        &self,
        pending_messages: &mut VecDeque<MwComMessage>,
        receive_batch_size: usize,
    ) -> Result<(), MwComProviderError> {
        match self {
            Self::NoReceive => Ok(()),
            Self::VehicleDynamicsServiceSpeed {
                instance_name,
                subscription,
                ..
            } => {
                let mut samples = SampleContainer::new(receive_batch_size);
                let count = subscription
                    .try_receive(&mut samples, receive_batch_size)
                    .map_err(|error| {
                        LolaScoreRuntimeAdapter::score_error("SCORE speed receive failed", error)
                    })?;
                for _ in 0..count {
                    let sample = samples.pop_front().ok_or_else(|| {
                        MwComProviderError::Transport(
                            "SCORE speed receive returned no sample".into(),
                        )
                    })?;
                    pending_messages.push_back(MwComMessage {
                        service: "VehicleDynamicsService".into(),
                        instance: instance_name.clone(),
                        member: "speed".into(),
                        value: MwComValue::F64(sample.value),
                        source_timestamp: Some(SystemTime::now()),
                        quality: score_quality(sample.quality)?,
                    });
                }
                Ok(())
            }
            Self::VehicleDynamicsServiceCruiseControlEnabled {
                instance_name,
                subscription,
                ..
            } => {
                let mut samples = SampleContainer::new(receive_batch_size);
                let count = subscription
                    .try_receive(&mut samples, receive_batch_size)
                    .map_err(|error| {
                        LolaScoreRuntimeAdapter::score_error(
                            "SCORE cruise_control_enabled receive failed",
                            error,
                        )
                    })?;
                for _ in 0..count {
                    let sample = samples.pop_front().ok_or_else(|| {
                        MwComProviderError::Transport(
                            "SCORE cruise_control_enabled receive returned no sample".into(),
                        )
                    })?;
                    pending_messages.push_back(MwComMessage {
                        service: "VehicleDynamicsService".into(),
                        instance: instance_name.clone(),
                        member: "cruise_control_enabled".into(),
                        value: MwComValue::Bool(sample.value),
                        source_timestamp: Some(SystemTime::now()),
                        quality: SignalQuality::Valid,
                    });
                }
                Ok(())
            }
        }
    }
}

pub struct LolaScoreRuntimeAdapter {
    config_path: PathBuf,
    runtime: Option<LolaRuntime>,
    receive_subscriptions: Vec<GeneratedReceiveSubscription>,
    vehicledynamicsservice_offered_producers:
        HashMap<String, VehicleDynamicsServiceOfferedProducer>,
    pending_messages: VecDeque<MwComMessage>,
    receive_cursor: usize,
}

// SAFETY: The adapter does not expose references to the native LoLa handles it owns, and provider
// access to the transport is serialized through mutable operations on the surrounding lock.
unsafe impl Sync for LolaScoreRuntimeAdapter {}

impl LolaScoreRuntimeAdapter {
    pub fn new(config_path: impl Into<PathBuf>) -> Self {
        let config_path = config_path.into();
        Self {
            config_path,
            runtime: None,
            receive_subscriptions: Vec::new(),
            vehicledynamicsservice_offered_producers: HashMap::new(),
            pending_messages: VecDeque::new(),
            receive_cursor: 0,
        }
    }

    pub fn from_score_config_env() -> Result<Self, MwComProviderError> {
        let config_path = std::env::var_os("SCORE_CONFIG_PATH").ok_or_else(|| {
            MwComProviderError::Config("SCORE_CONFIG_PATH must point to the LoLa config".into())
        })?;
        Ok(Self::new(PathBuf::from(config_path)))
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    fn runtime(&self) -> Result<&LolaRuntime, MwComProviderError> {
        self.runtime.as_ref().ok_or_else(|| {
            MwComProviderError::Transport("SCORE LoLa runtime is not connected".into())
        })
    }

    fn validate_binding(binding: ScoreBinding<'_>) -> Result<(), MwComProviderError> {
        match binding.service_name {
            "VehicleDynamicsService" => Ok(()),
            other => Err(MwComProviderError::Transport(format!(
                "LolaScoreRuntimeAdapter is not generated for SCORE service {other}"
            ))),
        }
    }

    fn instance_specifier(
        binding: ScoreBinding<'_>,
    ) -> Result<InstanceSpecifier, MwComProviderError> {
        InstanceSpecifier::new(binding.instance_specifier).map_err(|error| {
            MwComProviderError::Transport(format!(
                "invalid SCORE instance specifier {}: {error:?}",
                binding.instance_specifier
            ))
        })
    }

    fn score_error(context: &str, error: impl std::fmt::Debug) -> MwComProviderError {
        MwComProviderError::Transport(format!("{context}: {error:?}"))
    }

    fn subscription_queue_capacity() -> usize {
        std::env::var("SCORE_SUBSCRIPTION_QUEUE_CAPACITY")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(SCORE_DEFAULT_SUBSCRIPTION_QUEUE_CAPACITY)
    }

    fn receive_batch_size() -> usize {
        std::env::var("SCORE_RECEIVE_BATCH_SIZE")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(SCORE_DEFAULT_RECEIVE_BATCH_SIZE)
            .min(Self::subscription_queue_capacity())
    }

    fn ensure_vehicledynamicsservice_offered_producer(
        &mut self,
        binding: ScoreBinding<'_>,
    ) -> Result<&VehicleDynamicsServiceOfferedProducer, MwComProviderError> {
        if binding.service_name != "VehicleDynamicsService" {
            return Err(MwComProviderError::Transport(format!(
                "VehicleDynamicsService producer cannot publish {}",
                binding.service_name
            )));
        }

        let instance_specifier = binding.instance_specifier.to_string();
        if !self
            .vehicledynamicsservice_offered_producers
            .contains_key(&instance_specifier)
        {
            let specifier = Self::instance_specifier(binding)?;
            let producer = self
                .runtime()?
                .producer_builder::<VehicleDynamicsServiceInterfaceAlias>(specifier)
                .build()
                .map_err(|error| Self::score_error("SCORE producer build failed", error))?;
            let offered = producer
                .offer()
                .map_err(|error| Self::score_error("SCORE producer offer failed", error))?;
            self.vehicledynamicsservice_offered_producers
                .insert(instance_specifier.clone(), offered);
        }

        self.vehicledynamicsservice_offered_producers
            .get(&instance_specifier)
            .ok_or_else(|| MwComProviderError::Transport("SCORE producer was not offered".into()))
    }
}

#[async_trait]
impl ScoreRuntimeAdapter for LolaScoreRuntimeAdapter {
    async fn connect(&mut self) -> Result<(), MwComProviderError> {
        if self.runtime.is_some() {
            return Ok(());
        }
        let mut builder = LolaRuntimeBuilderImpl::new();
        builder.load_config(&self.config_path);
        let runtime = builder
            .build()
            .map_err(|error| Self::score_error("SCORE LoLa runtime build failed", error))?;
        self.runtime = Some(runtime);
        self.ensure_vehicledynamicsservice_offered_producer(ScoreBinding {
            service_name: "VehicleDynamicsService",
            instance_name: "front_vehicle_actuator",
            instance_specifier: "/Vehicle/Service2/Instance",
            member_name: "target_speed",
            element_kind: "event",
            payload_type: "TargetSpeed",
        })?;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), MwComProviderError> {
        for subscription in self.receive_subscriptions.drain(..) {
            subscription.unsubscribe();
        }
        for (_instance_specifier, producer) in self.vehicledynamicsservice_offered_producers.drain()
        {
            producer
                .unoffer()
                .map_err(|error| Self::score_error("SCORE producer unoffer failed", error))?;
        }
        self.pending_messages.clear();
        self.receive_cursor = 0;
        self.runtime = None;
        Ok(())
    }

    async fn find_service(&self, binding: ScoreBinding<'_>) -> Result<bool, MwComProviderError> {
        Self::validate_binding(binding)?;
        match binding.service_name {
            "VehicleDynamicsService" => {
                let discovery = self
                    .runtime()?
                    .find_service::<VehicleDynamicsServiceInterfaceAlias>(
                        FindServiceSpecifier::Specific(Self::instance_specifier(binding)?),
                    );
                let instances = discovery
                    .get_available_instances()
                    .map_err(|error| Self::score_error("SCORE service discovery failed", error))?;
                Ok(instances.into_iter().next().is_some())
            }
            other => Err(MwComProviderError::Transport(format!(
                "LolaScoreRuntimeAdapter is not generated for SCORE service {other}"
            ))),
        }
    }

    async fn subscribe(&mut self, binding: ScoreBinding<'_>) -> Result<(), MwComProviderError> {
        Self::validate_binding(binding)?;
        let queue_capacity = Self::subscription_queue_capacity();
        let instance_specifier = binding.instance_specifier.to_string();
        if self.receive_subscriptions.iter().any(|subscription| {
            subscription.matches(
                binding.service_name,
                &instance_specifier,
                binding.member_name,
            )
        }) {
            return Ok(());
        }

        match (binding.service_name, binding.member_name) {
            ("VehicleDynamicsService", "speed") => {
                let discovery = self
                    .runtime()?
                    .find_service::<VehicleDynamicsServiceInterfaceAlias>(
                        FindServiceSpecifier::Specific(Self::instance_specifier(binding)?),
                    );
                let consumer_builder = discovery
                    .get_available_instances()
                    .map_err(|error| Self::score_error("SCORE service discovery failed", error))?
                    .into_iter()
                    .next()
                    .ok_or_else(|| {
                        MwComProviderError::Transport(format!(
                            "SCORE service {} at {} is not available",
                            binding.service_name, binding.instance_specifier
                        ))
                    })?;
                let consumer = consumer_builder
                    .build()
                    .map_err(|error| Self::score_error("SCORE consumer build failed", error))?;
                let subscription = consumer
                    .speed
                    .subscribe(queue_capacity)
                    .map_err(|error| Self::score_error("SCORE speed subscribe failed", error))?;
                self.receive_subscriptions.push(
                    GeneratedReceiveSubscription::VehicleDynamicsServiceSpeed {
                        instance_name: binding.instance_name.to_string(),
                        instance_specifier,
                        subscription,
                    },
                );
                Ok(())
            }
            ("VehicleDynamicsService", "cruise_control_enabled") => {
                let discovery = self
                    .runtime()?
                    .find_service::<VehicleDynamicsServiceInterfaceAlias>(
                        FindServiceSpecifier::Specific(Self::instance_specifier(binding)?),
                    );
                let consumer_builder = discovery
                    .get_available_instances()
                    .map_err(|error| Self::score_error("SCORE service discovery failed", error))?
                    .into_iter()
                    .next()
                    .ok_or_else(|| {
                        MwComProviderError::Transport(format!(
                            "SCORE service {} at {} is not available",
                            binding.service_name, binding.instance_specifier
                        ))
                    })?;
                let consumer = consumer_builder
                    .build()
                    .map_err(|error| Self::score_error("SCORE consumer build failed", error))?;
                let subscription = consumer
                    .cruise_control_enabled
                    .subscribe(queue_capacity)
                    .map_err(|error| {
                        Self::score_error("SCORE cruise_control_enabled subscribe failed", error)
                    })?;
                self.receive_subscriptions.push(
                    GeneratedReceiveSubscription::VehicleDynamicsServiceCruiseControlEnabled {
                        instance_name: binding.instance_name.to_string(),
                        instance_specifier,
                        subscription,
                    },
                );
                Ok(())
            }
            _ => Ok(()),
        }
    }

    async fn unsubscribe(&mut self, binding: ScoreBinding<'_>) -> Result<(), MwComProviderError> {
        Self::validate_binding(binding)?;
        if let Some(index) = self.receive_subscriptions.iter().position(|subscription| {
            subscription.matches(
                binding.service_name,
                binding.instance_specifier,
                binding.member_name,
            )
        }) {
            self.receive_subscriptions.remove(index).unsubscribe();
        }
        if !self.receive_subscriptions.is_empty() {
            self.receive_cursor %= self.receive_subscriptions.len();
        } else {
            self.receive_cursor = 0;
        }
        Ok(())
    }

    async fn recv(&mut self) -> Result<Option<MwComMessage>, MwComProviderError> {
        if let Some(message) = self.pending_messages.pop_front() {
            return Ok(Some(message));
        }

        let subscription_count = self.receive_subscriptions.len();
        if subscription_count == 0 {
            return Ok(None);
        }
        let receive_batch_size = Self::receive_batch_size();

        for _ in 0..subscription_count {
            let index = self.receive_cursor % subscription_count;
            self.receive_subscriptions[index]
                .try_receive(&mut self.pending_messages, receive_batch_size)?;
            self.receive_cursor = (self.receive_cursor + 1) % subscription_count;
            if let Some(message) = self.pending_messages.pop_front() {
                return Ok(Some(message));
            }
        }

        Ok(None)
    }

    async fn send_actuation(
        &mut self,
        binding: ScoreBinding<'_>,
        message: MwComMessage,
    ) -> Result<(), MwComProviderError> {
        match (binding.service_name, binding.member_name) {
            ("VehicleDynamicsService", "target_speed") => {
                let value = numeric_value(&message.value, binding).map(|value| value as f64)?;
                let producer = self.ensure_vehicledynamicsservice_offered_producer(binding)?;
                tracing::debug!(value = ?value, "SCORE target_speed actuation sample publishing");
                let uninit_sample = producer.target_speed.allocate().map_err(|error| {
                    Self::score_error("SCORE target_speed allocate failed", error)
                })?;
                let sample = uninit_sample
                    .write(crate::mw_com_provider_types::vehicle::TargetSpeed { value: value });
                sample
                    .send()
                    .map_err(|error| Self::score_error("SCORE target_speed publish failed", error))
            }
            (_, other) => Err(MwComProviderError::Transport(format!(
                "SCORE actuation is not generated for {}:{}",
                binding.service_name, other
            ))),
        }
    }
}

fn score_quality(value: u8) -> Result<SignalQuality, MwComProviderError> {
    match value {
        0 => Ok(SignalQuality::Invalid),
        1 => Ok(SignalQuality::Stale),
        2 => Ok(SignalQuality::Degraded),
        3 => Ok(SignalQuality::Valid),
        other => Err(MwComProviderError::Transport(format!(
            "unsupported SCORE quality value {other}"
        ))),
    }
}

#[allow(dead_code)]
fn numeric_value(value: &MwComValue, binding: ScoreBinding<'_>) -> Result<f64, MwComProviderError> {
    match value {
        MwComValue::F64(value) => Ok(*value),
        MwComValue::I64(value) => Ok(*value as f64),
        MwComValue::U64(value) => Ok(*value as f64),
        other => Err(MwComProviderError::Transport(format!(
            "{}:{} requires numeric actuation, got {other:?}",
            binding.service_name, binding.member_name
        ))),
    }
}

#[allow(dead_code)]
fn bool_value(value: &MwComValue, binding: ScoreBinding<'_>) -> Result<bool, MwComProviderError> {
    match value {
        MwComValue::Bool(value) => Ok(*value),
        other => Err(MwComProviderError::Transport(format!(
            "{}:{} requires bool actuation, got {other:?}",
            binding.service_name, binding.member_name
        ))),
    }
}
