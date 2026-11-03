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

use thiserror::Error;

#[derive(Debug, Error)]
pub enum MwComProviderError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("mapping error: {0}")]
    Mapping(String),

    #[error("transport error: {0}")]
    Transport(String),

    #[error("SCORE service {service} at {instance} is not available")]
    ServiceUnavailable { service: String, instance: String },

    #[error("databroker registration failed: {0}")]
    Registration(String),

    #[error("databroker update failed: {0}")]
    BrokerUpdate(String),

    #[error("signal quality rejected: {0}")]
    Quality(String),

    #[error("provider is shutting down")]
    Shutdown,

    #[error("no mw::com sample available")]
    NoData,
}
