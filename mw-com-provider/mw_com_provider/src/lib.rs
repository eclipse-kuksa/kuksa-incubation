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

pub mod bindings;
pub mod config;
pub mod error;
pub mod lifecycle;
pub mod mapper;
pub mod provider;
pub mod quality;
pub mod score_bindings;
#[cfg(feature = "score-lola")]
pub mod score_lola_adapter {
    include!("../generated/mw_com_provider_lola_adapter.rs");
}
pub mod transport;

#[cfg(feature = "score-lola")]
pub mod mw_com_provider_types {
    include!("../generated/mw_com_provider_types.rs");
}

#[cfg(feature = "score-lola")]
pub mod mw_com_provider_interfaces {
    include!("../generated/mw_com_provider_interfaces.rs");
}

#[cfg(feature = "score-lola")]
pub mod mw_com_provider_instances {
    include!("../generated/mw_com_provider_instances.rs");
}

pub mod generated {
    include!("../generated/mw_com_provider_metadata.rs");
}

pub use config::{Direction, ProviderConfig, SignalMapping, VssDataType};
pub use error::MwComProviderError;
pub use mapper::MwComMapper;
pub use provider::{MwComProvider, MwComProviderWorker};
pub use quality::SignalQuality;
#[cfg(feature = "score-lola")]
pub use score_lola_adapter::LolaScoreRuntimeAdapter;
pub use transport::{MwComMessage, MwComTransport, MwComValue};
