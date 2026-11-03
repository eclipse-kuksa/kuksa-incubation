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
use async_trait::async_trait;
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq)]
pub enum MwComValue {
    Bool(bool),
    String(String),
    I64(i64),
    U64(u64),
    F64(f64),
    Bytes(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MwComMessage {
    // These three fields are the stable lookup key between generated S-Core
    // metadata and the runtime provider config.
    pub service: String,
    pub instance: String,
    pub member: String,
    pub value: MwComValue,
    pub source_timestamp: Option<SystemTime>,
    pub quality: SignalQuality,
}

// Middleware integrations implement this trait. The provider stays independent
// of S-Core LoLa details; tests use the mock transport and production uses the
// generated ScoreBindingTransport adapter.
#[async_trait]
pub trait MwComTransport: Send + Sync {
    async fn connect(&mut self) -> Result<(), MwComProviderError>;
    async fn disconnect(&mut self) -> Result<(), MwComProviderError>;
    async fn subscribe(
        &mut self,
        service: &str,
        instance: &str,
        member: &str,
    ) -> Result<(), MwComProviderError>;
    async fn recv(&mut self) -> Result<MwComMessage, MwComProviderError>;
    async fn send_actuation(&mut self, message: MwComMessage) -> Result<(), MwComProviderError>;
}

#[cfg(feature = "mock-transport")]
pub mod mock {
    use super::*;
    use tokio::sync::mpsc;

    pub struct MockMwComTransport {
        inbound: mpsc::Receiver<MwComMessage>,
        outbound: mpsc::Sender<MwComMessage>,
        connected: bool,
    }

    impl MockMwComTransport {
        pub fn new(
            inbound: mpsc::Receiver<MwComMessage>,
            outbound: mpsc::Sender<MwComMessage>,
        ) -> Self {
            Self {
                inbound,
                outbound,
                connected: false,
            }
        }
    }

    #[async_trait]
    impl MwComTransport for MockMwComTransport {
        async fn connect(&mut self) -> Result<(), MwComProviderError> {
            self.connected = true;
            Ok(())
        }

        async fn disconnect(&mut self) -> Result<(), MwComProviderError> {
            self.connected = false;
            Ok(())
        }

        async fn subscribe(
            &mut self,
            _service: &str,
            _instance: &str,
            _member: &str,
        ) -> Result<(), MwComProviderError> {
            Ok(())
        }

        async fn recv(&mut self) -> Result<MwComMessage, MwComProviderError> {
            if !self.connected {
                return Err(MwComProviderError::Transport(
                    "mock transport is disconnected".into(),
                ));
            }
            self.inbound
                .recv()
                .await
                .ok_or(MwComProviderError::Shutdown)
        }

        async fn send_actuation(
            &mut self,
            message: MwComMessage,
        ) -> Result<(), MwComProviderError> {
            self.outbound.send(message).await.map_err(|_| {
                MwComProviderError::Transport("mock actuation receiver dropped".into())
            })
        }
    }
}
