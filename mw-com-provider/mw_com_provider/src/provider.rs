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
use crate::config::{Direction, SignalMapping, VssDataType};
use crate::error::MwComProviderError;
use crate::mapper::MwComMapper;
use crate::transport::MwComMessage;
use crate::transport::MwComTransport;
use async_trait::async_trait;
use databroker::broker::{
    ActuationChange, ActuationError, ActuationProvider, DataBroker, DataType, EntryType, Field,
    GetValuesProviderResponse, Metadata, RegisterSignalError, SignalProvider,
};
use databroker::permissions::Permissions;
use databroker::types::{SignalId, TimeInterval};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::time::sleep;
use tokio_stream::StreamExt;
use tracing::{debug, info, warn};

#[derive(Clone)]
pub struct MwComProvider {
    broker: DataBroker,
    permissions: Permissions,
    transport: Arc<RwLock<dyn MwComTransport>>,
    mapper: Arc<MwComMapper>,
}

impl MwComProvider {
    pub fn new<T>(
        broker: DataBroker,
        permissions: Permissions,
        config: ProviderConfig,
        transport: T,
    ) -> Result<Self, MwComProviderError>
    where
        T: MwComTransport + 'static,
    {
        Ok(Self {
            broker,
            permissions,
            transport: Arc::new(RwLock::new(transport)),
            mapper: Arc::new(MwComMapper::new(config)?),
        })
    }

    pub fn from_config_file<T>(
        broker: DataBroker,
        permissions: Permissions,
        config_path: impl AsRef<Path>,
        transport: T,
    ) -> Result<Self, MwComProviderError>
    where
        T: MwComTransport + 'static,
    {
        let config = ProviderConfig::load_json(config_path)?;
        Self::new(broker, permissions, config, transport)
    }

    pub async fn register_with_broker(&self) -> Result<(), MwComProviderError> {
        let broker = self.broker.authorized_access(&self.permissions);
        validate_mappings_against_broker(&self.mapper, &broker).await?;

        let signal_intervals = self.mapper.signal_intervals();
        let signal_count = signal_intervals.len();

        if signal_count > 0 {
            broker
                .register_signals(signal_intervals, Box::new(self.clone()))
                .await
                .map_err(|error| MwComProviderError::Registration(error.1))?;
        }

        let actuator_signal_ids = self.mapper.actuator_signal_ids();
        if !actuator_signal_ids.is_empty() {
            broker
                .provide_actuation(actuator_signal_ids, Box::new(self.clone()))
                .await
                .map_err(|error| MwComProviderError::Registration(error.1))?;
        }

        info!(
            provider = %self.mapper.config().provider_name,
            signals = signal_count,
            actuators = self.mapper.actuator_signal_ids().len(),
            "mw::com provider registered with Databroker"
        );
        Ok(())
    }

    pub async fn connect_and_subscribe(&self) -> Result<(), MwComProviderError> {
        let mut transport = self.transport.write().await;
        transport.connect().await?;
        for mapping in self.mapper.subscriptions() {
            if let Err(error) = transport
                .subscribe(&mapping.service, &mapping.instance, &mapping.member)
                .await
            {
                if matches!(error, MwComProviderError::ServiceUnavailable { .. }) {
                    return Err(error);
                }
                if let Err(shutdown_error) = transport.disconnect().await {
                    warn!(%shutdown_error, "mw::com provider cleanup after subscribe failure failed");
                }
                return Err(error);
            }
        }
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<(), MwComProviderError> {
        self.transport.write().await.disconnect().await
    }

    pub async fn run_until_shutdown(
        &self,
        mut shutdown: oneshot::Receiver<()>,
    ) -> Result<(), MwComProviderError> {
        let mut reconnect_delay = self.mapper.config().reconnect.initial_delay();

        loop {
            match self.connect_and_subscribe().await {
                Ok(()) => {
                    reconnect_delay = self.mapper.config().reconnect.initial_delay();
                    info!(provider = %self.mapper.config().provider_name, "mw::com provider connected");
                }
                Err(MwComProviderError::ServiceUnavailable { service, instance }) => {
                    warn!(%service, %instance, ?reconnect_delay, "mw::com provider service unavailable; retrying");
                    tokio::select! {
                        _ = sleep(reconnect_delay) => {
                            reconnect_delay = self.mapper.config().reconnect.next_delay(reconnect_delay);
                            continue;
                        }
                        _ = &mut shutdown => {
                            self.shutdown().await?;
                            return Ok(());
                        }
                    }
                }
                Err(error) => {
                    warn!(%error, ?reconnect_delay, "mw::com provider connect failed; retrying");
                    tokio::select! {
                        _ = sleep(reconnect_delay) => {
                            reconnect_delay = self.mapper.config().reconnect.next_delay(reconnect_delay);
                            continue;
                        }
                        _ = &mut shutdown => {
                            self.shutdown().await?;
                            return Ok(());
                        }
                    }
                }
            }

            loop {
                tokio::select! {
                    result = self.recv_and_publish_one() => {
                        match result {
                            Ok(()) => {}
                            Err(MwComProviderError::Quality(error)) | Err(MwComProviderError::Mapping(error)) => {
                                warn!(%error, "mw::com sample rejected");
                            }
                            Err(MwComProviderError::NoData) => {
                                tokio::task::yield_now().await;
                            }
                            Err(MwComProviderError::Shutdown) => {
                                info!("mw::com transport shut down");
                                self.shutdown().await?;
                                return Ok(());
                            }
                            Err(error) => {
                                warn!(%error, ?reconnect_delay, "mw::com receive failed; reconnecting");
                                let _ = self.shutdown().await;
                                break;
                            }
                        }
                    }
                    _ = &mut shutdown => {
                        self.shutdown().await?;
                        return Ok(());
                    }
                }
            }

            sleep(reconnect_delay).await;
            reconnect_delay = self.mapper.config().reconnect.next_delay(reconnect_delay);
        }
    }

    async fn recv_and_publish_one(&self) -> Result<(), MwComProviderError> {
        let message = self.transport.write().await.recv().await?;
        let (signal_id, update) = self.mapper.message_to_update(&message)?;
        let broker = self.broker.authorized_access(&self.permissions);
        broker
            .update_entries(vec![(signal_id, update)])
            .await
            .map_err(|errors| MwComProviderError::BrokerUpdate(format!("{errors:?}")))?;
        debug!(signal_id, "mw::com sample written to Databroker");
        Ok(())
    }
}

pub struct MwComProviderWorker<T>
where
    T: MwComTransport,
{
    broker: DataBroker,
    permissions: Permissions,
    transport: T,
    mapper: Arc<MwComMapper>,
    actuation_tx: mpsc::Sender<MwComMessage>,
    actuation_rx: mpsc::Receiver<MwComMessage>,
}

// The worker owns the middleware transport and runs all mw::com I/O from one
// async context. Databroker callbacks enqueue actuation messages instead of
// touching LoLa directly, which keeps transport ownership simple and bounded.
impl<T> MwComProviderWorker<T>
where
    T: MwComTransport + 'static,
{
    pub fn new(
        broker: DataBroker,
        permissions: Permissions,
        config: ProviderConfig,
        transport: T,
    ) -> Result<Self, MwComProviderError> {
        let queue_capacity = config.queue_capacity;
        let mapper = Arc::new(MwComMapper::new(config)?);
        let (actuation_tx, actuation_rx) = mpsc::channel(queue_capacity);
        Ok(Self {
            broker,
            permissions,
            transport,
            mapper,
            actuation_tx,
            actuation_rx,
        })
    }

    pub async fn register_with_broker(&self) -> Result<(), MwComProviderError> {
        let broker = self.broker.authorized_access(&self.permissions);
        validate_mappings_against_broker(&self.mapper, &broker).await?;

        let signal_intervals = self.mapper.signal_intervals();
        let signal_count = signal_intervals.len();
        if signal_count > 0 {
            broker
                .register_signals(
                    signal_intervals,
                    Box::new(QueuedSignalProvider {
                        mapper: Arc::clone(&self.mapper),
                    }),
                )
                .await
                .map_err(|error| MwComProviderError::Registration(error.1))?;
        }

        let actuator_signal_ids = self.mapper.actuator_signal_ids();
        let actuator_count = actuator_signal_ids.len();
        if !actuator_signal_ids.is_empty() {
            // val.v2 actuation reaches this provider callback directly. val.v1
            // CLI writes actuator targets, so the forwarder below feeds both
            // protocols into the same middleware send queue.
            broker
                .provide_actuation(
                    actuator_signal_ids,
                    Box::new(QueuedActuationProvider {
                        mapper: Arc::clone(&self.mapper),
                        actuation_tx: self.actuation_tx.clone(),
                    }),
                )
                .await
                .map_err(|error| MwComProviderError::Registration(error.1))?;
            self.start_actuator_target_forwarder().await?;
        }

        info!(
            provider = %self.mapper.config().provider_name,
            signals = signal_count,
            actuators = actuator_count,
            "mw::com provider worker registered with Databroker"
        );
        Ok(())
    }

    async fn start_actuator_target_forwarder(&self) -> Result<(), MwComProviderError> {
        let mut target_filters = HashMap::new();
        for signal_id in self.mapper.actuator_signal_ids() {
            target_filters.insert(signal_id, HashSet::from([Field::ActuatorTarget]));
        }

        if target_filters.is_empty() {
            return Ok(());
        }

        let broker = self.broker.authorized_access(&self.permissions);
        let mut target_updates =
            broker
                .subscribe(target_filters, None, None)
                .await
                .map_err(|error| {
                    MwComProviderError::Registration(format!(
                        "failed to subscribe to actuator target updates: {error:?}"
                    ))
                })?;
        // Databroker subscriptions begin with a current-value snapshot. Do not
        // replay that snapshot as a new actuator command into mw::com.
        let _ = target_updates.next().await;
        debug!("mw::com actuator target initial snapshot ignored");
        let mapper = Arc::clone(&self.mapper);
        let actuation_tx = self.actuation_tx.clone();

        tokio::spawn(async move {
            while let Some(entry_updates) = target_updates.next().await {
                let Some(entry_updates) = entry_updates else {
                    continue;
                };

                for notification in entry_updates.updates {
                    if !notification.fields.contains(&Field::ActuatorTarget) {
                        continue;
                    }

                    let Some(Some(datapoint)) = notification.update.actuator_target else {
                        continue;
                    };

                    let change = ActuationChange {
                        id: notification.id,
                        data_value: datapoint.value,
                    };
                    let message = match mapper.actuation_to_message(&change) {
                        Ok(message) => message,
                        Err(error) => {
                            warn!(%error, "mw::com actuator target update rejected");
                            continue;
                        }
                    };

                    debug!(
                        signal_id = notification.id,
                        service = %message.service,
                        instance = %message.instance,
                        member = %message.member,
                        "mw::com actuator target update queued"
                    );

                    if actuation_tx.send(message).await.is_err() {
                        warn!("mw::com actuator target forwarder stopped: worker unavailable");
                        return;
                    }
                }
            }
        });

        Ok(())
    }

    async fn connect_and_subscribe(&mut self) -> Result<(), MwComProviderError> {
        self.transport.connect().await?;
        for mapping in self.mapper.subscriptions() {
            if let Err(error) = self
                .transport
                .subscribe(&mapping.service, &mapping.instance, &mapping.member)
                .await
            {
                if matches!(error, MwComProviderError::ServiceUnavailable { .. }) {
                    return Err(error);
                }
                if let Err(shutdown_error) = self.transport.disconnect().await {
                    warn!(%shutdown_error, "mw::com provider worker cleanup after subscribe failure failed");
                }
                return Err(error);
            }
        }
        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<(), MwComProviderError> {
        self.transport.disconnect().await
    }

    pub async fn run_until_shutdown(
        mut self,
        mut shutdown: oneshot::Receiver<()>,
    ) -> Result<(), MwComProviderError> {
        let mut reconnect_delay = self.mapper.config().reconnect.initial_delay();

        loop {
            // S-Core service discovery can race with the producer process
            // startup. Keep retrying instead of failing the Databroker process.
            match self.connect_and_subscribe().await {
                Ok(()) => {
                    reconnect_delay = self.mapper.config().reconnect.initial_delay();
                    info!(provider = %self.mapper.config().provider_name, "mw::com provider worker connected");
                }
                Err(MwComProviderError::ServiceUnavailable { service, instance }) => {
                    warn!(%service, %instance, ?reconnect_delay, "mw::com provider worker service unavailable; retrying");
                    tokio::select! {
                        _ = sleep(reconnect_delay) => {
                            reconnect_delay = self.mapper.config().reconnect.next_delay(reconnect_delay);
                            continue;
                        }
                        _ = &mut shutdown => {
                            self.shutdown().await?;
                            return Ok(());
                        }
                    }
                }
                Err(error) => {
                    warn!(%error, ?reconnect_delay, "mw::com provider worker connect failed; retrying");
                    tokio::select! {
                        _ = sleep(reconnect_delay) => {
                            reconnect_delay = self.mapper.config().reconnect.next_delay(reconnect_delay);
                            continue;
                        }
                        _ = &mut shutdown => {
                            self.shutdown().await?;
                            return Ok(());
                        }
                    }
                }
            }

            loop {
                // Give queued actuation requests low latency even if the receive
                // side is busy or temporarily returns no sample.
                self.drain_actuation_queue().await?;
                tokio::select! {
                    result = self.transport.recv() => {
                        match result {
                            Ok(message) => {
                                self.publish_or_reject_message(message).await?;
                            }
                            Err(MwComProviderError::Quality(error)) | Err(MwComProviderError::Mapping(error)) => {
                                warn!(%error, "mw::com sample rejected");
                            }
                            Err(MwComProviderError::NoData) => {
                                tokio::task::yield_now().await;
                            }
                            Err(MwComProviderError::Shutdown) => {
                                info!("mw::com transport shut down");
                                self.shutdown().await?;
                                return Ok(());
                            }
                            Err(error) => {
                                warn!(%error, ?reconnect_delay, "mw::com receive failed; reconnecting");
                                let _ = self.shutdown().await;
                                break;
                            }
                        }
                    }
                    message = self.actuation_rx.recv() => {
                        if let Some(message) = message {
                            debug!(
                                service = %message.service,
                                instance = %message.instance,
                                member = %message.member,
                                "mw::com actuation request sent"
                            );
                            self.transport.send_actuation(message).await?;
                        } else {
                            warn!("mw::com actuation channel closed");
                            self.shutdown().await?;
                            return Ok(());
                        }
                    }
                    _ = &mut shutdown => {
                        self.shutdown().await?;
                        return Ok(());
                    }
                }
            }

            sleep(reconnect_delay).await;
            reconnect_delay = self.mapper.config().reconnect.next_delay(reconnect_delay);
        }
    }

    async fn publish_message(&self, message: MwComMessage) -> Result<(), MwComProviderError> {
        let (signal_id, update) = self.mapper.message_to_update(&message)?;
        let broker = self.broker.authorized_access(&self.permissions);
        broker
            .update_entries(vec![(signal_id, update)])
            .await
            .map_err(|errors| MwComProviderError::BrokerUpdate(format!("{errors:?}")))?;
        debug!(signal_id, "mw::com sample written to Databroker");
        Ok(())
    }

    async fn publish_or_reject_message(
        &self,
        message: MwComMessage,
    ) -> Result<(), MwComProviderError> {
        // Bad samples should not tear down the LoLa session. Reject them and
        // continue receiving later middleware updates.
        match self.publish_message(message).await {
            Ok(()) => Ok(()),
            Err(MwComProviderError::Quality(error)) | Err(MwComProviderError::Mapping(error)) => {
                warn!(%error, "mw::com sample rejected");
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    async fn drain_actuation_queue(&mut self) -> Result<(), MwComProviderError> {
        while let Ok(message) = self.actuation_rx.try_recv() {
            debug!(
                service = %message.service,
                instance = %message.instance,
                member = %message.member,
                "mw::com queued actuation request sent"
            );
            self.transport.send_actuation(message).await?;
        }
        Ok(())
    }
}

struct QueuedSignalProvider {
    mapper: Arc<MwComMapper>,
}

#[async_trait]
impl SignalProvider for QueuedSignalProvider {
    async fn update_filter(
        &self,
        update_filters: HashMap<SignalId, Option<TimeInterval>>,
    ) -> Result<(), (RegisterSignalError, String)> {
        debug!(
            provider = %self.mapper.config().provider_name,
            filters = update_filters.len(),
            "mw::com provider worker received Databroker filter update"
        );
        Ok(())
    }

    fn is_available(&self) -> bool {
        true
    }

    async fn get_signals_values_from_provider(
        &self,
        _signals_ids: Vec<SignalId>,
    ) -> Result<GetValuesProviderResponse, ()> {
        Err(())
    }
}

struct QueuedActuationProvider {
    mapper: Arc<MwComMapper>,
    actuation_tx: mpsc::Sender<MwComMessage>,
}

#[async_trait]
impl ActuationProvider for QueuedActuationProvider {
    async fn actuate(
        &self,
        actuation_changes: Vec<ActuationChange>,
    ) -> Result<(), (ActuationError, String)> {
        for change in actuation_changes {
            debug!(
                signal_id = change.id,
                value = ?change.data_value,
                "mw::com actuation request received"
            );
            let message = self
                .mapper
                .actuation_to_message(&change)
                .map_err(|error| (ActuationError::WrongType, error.to_string()))?;
            debug!(
                service = %message.service,
                instance = %message.instance,
                member = %message.member,
                "mw::com actuation request queued"
            );
            // Queueing keeps the Databroker callback non-blocking and avoids
            // sharing the LoLa transport across runtime boundaries.
            self.actuation_tx.send(message).await.map_err(|error| {
                (
                    ActuationError::TransmissionFailure,
                    format!("mw::com actuation worker is not available: {error}"),
                )
            })?;
        }
        Ok(())
    }

    fn is_available(&self) -> bool {
        !self.actuation_tx.is_closed()
    }
}

async fn validate_mappings_against_broker(
    mapper: &MwComMapper,
    broker: &databroker::broker::AuthorizedAccess<'_, '_>,
) -> Result<(), MwComProviderError> {
    for mapping in &mapper.config().mappings {
        // Registration is only safe after the composition binary has seeded the
        // matching Databroker metadata and rewritten generated IDs to runtime IDs.
        let metadata = broker
            .get_metadata_by_path(&mapping.vss_path)
            .await
            .ok_or_else(|| {
                MwComProviderError::Registration(format!(
                    "mapping for {} references a VSS path that is not registered in Databroker",
                    mapping.vss_path
                ))
            })?;

        validate_mapping_metadata(mapping, &metadata)?;
    }

    Ok(())
}

fn validate_mapping_metadata(
    mapping: &SignalMapping,
    metadata: &Metadata,
) -> Result<(), MwComProviderError> {
    if metadata.id != mapping.signal_id {
        return Err(MwComProviderError::Registration(format!(
            "mapping for {} uses signal id {}, but Databroker registered id {}",
            mapping.vss_path, mapping.signal_id, metadata.id
        )));
    }

    if !datatype_matches(mapping.datatype, &metadata.data_type) {
        return Err(MwComProviderError::Registration(format!(
            "mapping for {} uses datatype {:?}, but Databroker registered {:?}",
            mapping.vss_path, mapping.datatype, metadata.data_type
        )));
    }

    if !direction_matches(mapping.direction, &metadata.entry_type) {
        return Err(MwComProviderError::Registration(format!(
            "mapping for {} uses direction {:?}, but Databroker registered entry type {:?}",
            mapping.vss_path, mapping.direction, metadata.entry_type
        )));
    }

    Ok(())
}

fn datatype_matches(mapping_type: VssDataType, broker_type: &DataType) -> bool {
    matches!(
        (mapping_type, broker_type),
        (VssDataType::Bool, DataType::Bool)
            | (VssDataType::String, DataType::String)
            | (VssDataType::Int8, DataType::Int8)
            | (VssDataType::Int32, DataType::Int32)
            | (VssDataType::Int64, DataType::Int64)
            | (VssDataType::Uint8, DataType::Uint8)
            | (VssDataType::Uint32, DataType::Uint32)
            | (VssDataType::Uint64, DataType::Uint64)
            | (VssDataType::Float, DataType::Float)
            | (VssDataType::Double, DataType::Double)
    )
}

fn direction_matches(direction: Direction, entry_type: &EntryType) -> bool {
    match direction {
        Direction::Datapoint => matches!(entry_type, EntryType::Sensor | EntryType::Actuator),
        Direction::Actuator | Direction::Bidirectional => matches!(entry_type, EntryType::Actuator),
    }
}

#[async_trait]
impl SignalProvider for MwComProvider {
    async fn update_filter(
        &self,
        update_filters: HashMap<SignalId, Option<TimeInterval>>,
    ) -> Result<(), (RegisterSignalError, String)> {
        debug!(
            filters = update_filters.len(),
            "mw::com provider received Databroker filter update"
        );
        Ok(())
    }

    fn is_available(&self) -> bool {
        true
    }

    async fn get_signals_values_from_provider(
        &self,
        _signals_ids: Vec<SignalId>,
    ) -> Result<GetValuesProviderResponse, ()> {
        Err(())
    }
}

#[async_trait]
impl ActuationProvider for MwComProvider {
    async fn actuate(
        &self,
        actuation_changes: Vec<ActuationChange>,
    ) -> Result<(), (ActuationError, String)> {
        for change in actuation_changes {
            let message = self
                .mapper
                .actuation_to_message(&change)
                .map_err(|error| (ActuationError::WrongType, error.to_string()))?;
            self.transport
                .write()
                .await
                .send_actuation(message)
                .await
                .map_err(|error| (ActuationError::TransmissionFailure, error.to_string()))?;
        }
        Ok(())
    }

    fn is_available(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quality::SignalQuality;
    use crate::transport::mock::MockMwComTransport;
    use crate::transport::MwComValue;
    use async_trait::async_trait;
    use databroker::broker::ChangeType;
    use databroker::permissions::ALLOW_ALL;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, SystemTime};
    use tokio::sync::mpsc;
    use tokio::time::timeout;

    #[derive(Clone, Default)]
    struct TransportState {
        subscribed: Arc<Mutex<Vec<String>>>,
        disconnects: Arc<Mutex<usize>>,
    }

    struct FailingSubscribeTransport {
        state: TransportState,
    }

    impl FailingSubscribeTransport {
        fn new(state: TransportState) -> Self {
            Self { state }
        }
    }

    #[async_trait]
    impl MwComTransport for FailingSubscribeTransport {
        async fn connect(&mut self) -> Result<(), MwComProviderError> {
            Ok(())
        }

        async fn disconnect(&mut self) -> Result<(), MwComProviderError> {
            self.state.subscribed.lock().unwrap().clear();
            *self.state.disconnects.lock().unwrap() += 1;
            Ok(())
        }

        async fn subscribe(
            &mut self,
            _service: &str,
            _instance: &str,
            member: &str,
        ) -> Result<(), MwComProviderError> {
            if member == "target_speed" {
                return Err(MwComProviderError::Transport(
                    "target_speed subscribe failed".into(),
                ));
            }
            self.state
                .subscribed
                .lock()
                .unwrap()
                .push(member.to_string());
            Ok(())
        }

        async fn recv(&mut self) -> Result<MwComMessage, MwComProviderError> {
            Err(MwComProviderError::NoData)
        }

        async fn send_actuation(
            &mut self,
            _message: MwComMessage,
        ) -> Result<(), MwComProviderError> {
            Ok(())
        }
    }

    fn mapping(direction: Direction, datatype: VssDataType) -> SignalMapping {
        SignalMapping {
            signal_id: 42,
            vss_path: "Vehicle.Speed".into(),
            datatype,
            service: "VehicleDynamicsService".into(),
            instance: "front_vehicle".into(),
            member: "speed".into(),
            field: None,
            unit: Some("km/h".into()),
            scale: 1.0,
            offset: 0.0,
            min: Some(0.0),
            max: Some(300.0),
            required_quality: SignalQuality::Valid,
            direction,
            cycle_time_ms: 100,
        }
    }

    fn metadata(entry_type: EntryType, data_type: DataType) -> Metadata {
        Metadata {
            id: 42,
            path: "Vehicle.Speed".into(),
            glob_path: "Vehicle/Speed".into(),
            data_type,
            entry_type,
            change_type: ChangeType::Continuous,
            description: "Vehicle speed.".into(),
            min: None,
            max: None,
            allowed: None,
            unit: Some("km/h".into()),
        }
    }

    fn config(mapping: SignalMapping) -> ProviderConfig {
        ProviderConfig {
            provider_name: "test".into(),
            reconnect: Default::default(),
            queue_capacity: 4,
            stale_after_ms: 1_000,
            mappings: vec![mapping],
        }
    }

    fn config_with_mappings(mappings: Vec<SignalMapping>) -> ProviderConfig {
        ProviderConfig {
            provider_name: "test".into(),
            reconnect: Default::default(),
            queue_capacity: 4,
            stale_after_ms: 1_000,
            mappings,
        }
    }

    fn mock_transport() -> MockMwComTransport {
        let (_inbound_tx, inbound_rx) = mpsc::channel(1);
        let (outbound_tx, _outbound_rx) = mpsc::channel(1);
        MockMwComTransport::new(inbound_rx, outbound_tx)
    }

    async fn broker_with_entry(entry_type: EntryType, data_type: DataType) -> (DataBroker, i32) {
        let broker = DataBroker::new("test", "test");
        let id = broker
            .authorized_access(&ALLOW_ALL)
            .add_entry(
                "Vehicle.Speed".into(),
                data_type,
                ChangeType::Continuous,
                entry_type,
                "Vehicle speed.".into(),
                None,
                None,
                None,
                Some("km/h".into()),
            )
            .await
            .unwrap();

        (broker, id)
    }

    #[test]
    fn accepts_matching_sensor_datapoint_mapping() {
        let mapping = mapping(Direction::Datapoint, VssDataType::Double);
        let metadata = metadata(EntryType::Sensor, DataType::Double);

        validate_mapping_metadata(&mapping, &metadata).unwrap();
    }

    #[test]
    fn rejects_signal_id_mismatch() {
        let mapping = mapping(Direction::Datapoint, VssDataType::Double);
        let mut metadata = metadata(EntryType::Sensor, DataType::Double);
        metadata.id = 7;

        let error = validate_mapping_metadata(&mapping, &metadata).unwrap_err();
        assert!(error.to_string().contains("uses signal id 42"));
    }

    #[test]
    fn rejects_datatype_mismatch() {
        let mapping = mapping(Direction::Datapoint, VssDataType::Double);
        let metadata = metadata(EntryType::Sensor, DataType::Float);

        let error = validate_mapping_metadata(&mapping, &metadata).unwrap_err();
        assert!(error.to_string().contains("uses datatype Double"));
    }

    #[test]
    fn rejects_actuator_mapping_for_sensor_entry() {
        let mapping = mapping(Direction::Actuator, VssDataType::Double);
        let metadata = metadata(EntryType::Sensor, DataType::Double);

        let error = validate_mapping_metadata(&mapping, &metadata).unwrap_err();
        assert!(error.to_string().contains("uses direction Actuator"));
    }

    #[test]
    fn accepts_bidirectional_mapping_for_actuator_entry() {
        let mapping = mapping(Direction::Bidirectional, VssDataType::Double);
        let metadata = metadata(EntryType::Actuator, DataType::Double);

        validate_mapping_metadata(&mapping, &metadata).unwrap();
    }

    #[test]
    fn rejects_datapoint_mapping_for_attribute_entry() {
        let mapping = mapping(Direction::Datapoint, VssDataType::Double);
        let metadata = metadata(EntryType::Attribute, DataType::Double);

        let error = validate_mapping_metadata(&mapping, &metadata).unwrap_err();

        assert!(error.to_string().contains("entry type Attribute"));
    }

    #[test]
    fn accepts_all_supported_scalar_datatype_pairs() {
        let cases = [
            (VssDataType::Bool, DataType::Bool),
            (VssDataType::String, DataType::String),
            (VssDataType::Int8, DataType::Int8),
            (VssDataType::Int32, DataType::Int32),
            (VssDataType::Int64, DataType::Int64),
            (VssDataType::Uint8, DataType::Uint8),
            (VssDataType::Uint32, DataType::Uint32),
            (VssDataType::Uint64, DataType::Uint64),
            (VssDataType::Float, DataType::Float),
            (VssDataType::Double, DataType::Double),
        ];

        for (mapping_type, broker_type) in cases {
            assert!(datatype_matches(mapping_type, &broker_type));
        }
    }

    #[tokio::test]
    async fn register_with_broker_accepts_matching_databroker_metadata() {
        let (broker, signal_id) = broker_with_entry(EntryType::Sensor, DataType::Double).await;
        let mut mapping = mapping(Direction::Datapoint, VssDataType::Double);
        mapping.signal_id = signal_id;
        let provider =
            MwComProvider::new(broker, ALLOW_ALL.clone(), config(mapping), mock_transport())
                .unwrap();

        provider.register_with_broker().await.unwrap();
    }

    #[tokio::test]
    async fn register_with_broker_rejects_missing_databroker_path() {
        let broker = DataBroker::new("test", "test");
        let provider = MwComProvider::new(
            broker,
            ALLOW_ALL.clone(),
            config(mapping(Direction::Datapoint, VssDataType::Double)),
            mock_transport(),
        )
        .unwrap();

        let error = provider.register_with_broker().await.unwrap_err();

        assert!(error
            .to_string()
            .contains("references a VSS path that is not registered"));
    }

    #[tokio::test]
    async fn worker_disconnects_partial_subscriptions_after_subscribe_failure() {
        let broker = DataBroker::new("test", "test");
        let mut speed_mapping = mapping(Direction::Datapoint, VssDataType::Double);
        speed_mapping.signal_id = 42;
        let mut target_speed_mapping = mapping(Direction::Bidirectional, VssDataType::Double);
        target_speed_mapping.signal_id = 43;
        target_speed_mapping.vss_path = "Vehicle.ADAS.CruiseControl.TargetSpeed".into();
        target_speed_mapping.member = "target_speed".into();
        let state = TransportState::default();
        let mut provider = MwComProviderWorker::new(
            broker,
            ALLOW_ALL.clone(),
            config_with_mappings(vec![speed_mapping, target_speed_mapping]),
            FailingSubscribeTransport::new(state.clone()),
        )
        .unwrap();

        let error = provider.connect_and_subscribe().await.unwrap_err();

        assert!(error.to_string().contains("target_speed subscribe failed"));
        assert_eq!(*state.disconnects.lock().unwrap(), 1);
        assert!(state.subscribed.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn worker_rejects_out_of_range_sample_without_failing() {
        let (broker, signal_id) = broker_with_entry(EntryType::Sensor, DataType::Double).await;
        let mut speed_mapping = mapping(Direction::Datapoint, VssDataType::Double);
        speed_mapping.signal_id = signal_id;
        let provider = MwComProviderWorker::new(
            broker,
            ALLOW_ALL.clone(),
            config(speed_mapping),
            mock_transport(),
        )
        .unwrap();

        provider
            .publish_or_reject_message(MwComMessage {
                service: "VehicleDynamicsService".into(),
                instance: "front_vehicle".into(),
                member: "speed".into(),
                value: MwComValue::F64(302.4),
                source_timestamp: Some(SystemTime::now()),
                quality: SignalQuality::Valid,
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn worker_forwards_actuator_target_updates_to_transport() {
        let (broker, signal_id) = broker_with_entry(EntryType::Actuator, DataType::Double).await;
        let mut target_speed_mapping = mapping(Direction::Actuator, VssDataType::Double);
        target_speed_mapping.signal_id = signal_id;
        target_speed_mapping.member = "target_speed".into();
        let (_inbound_tx, inbound_rx) = mpsc::channel(1);
        let (outbound_tx, mut outbound_rx) = mpsc::channel(1);
        let mut provider = MwComProviderWorker::new(
            broker.clone(),
            ALLOW_ALL.clone(),
            config(target_speed_mapping),
            MockMwComTransport::new(inbound_rx, outbound_tx),
        )
        .unwrap();
        provider.register_with_broker().await.unwrap();

        broker
            .authorized_access(&ALLOW_ALL)
            .update_entries(vec![(
                signal_id,
                databroker::broker::EntryUpdate {
                    actuator_target: Some(Some(databroker::broker::Datapoint {
                        ts: SystemTime::now(),
                        source_ts: Some(SystemTime::now()),
                        value: databroker::broker::DataValue::Double(88.0),
                    })),
                    ..Default::default()
                },
            )])
            .await
            .unwrap();

        let message = timeout(Duration::from_secs(1), async {
            loop {
                provider.drain_actuation_queue().await.unwrap();
                if let Ok(message) = outbound_rx.try_recv() {
                    break message;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();

        assert_eq!(message.member, "target_speed");
        assert_eq!(message.value, MwComValue::F64(88.0));
    }

    #[tokio::test]
    async fn register_with_broker_rejects_databroker_datatype_mismatch() {
        let (broker, signal_id) = broker_with_entry(EntryType::Sensor, DataType::Float).await;
        let mut mapping = mapping(Direction::Datapoint, VssDataType::Double);
        mapping.signal_id = signal_id;
        let provider =
            MwComProvider::new(broker, ALLOW_ALL.clone(), config(mapping), mock_transport())
                .unwrap();

        let error = provider.register_with_broker().await.unwrap_err();

        assert!(error.to_string().contains("uses datatype Double"));
    }
}
