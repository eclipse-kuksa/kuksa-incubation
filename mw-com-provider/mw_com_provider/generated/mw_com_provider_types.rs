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

use score_com::{CommData, Reloc};
use std::default::Default;

pub mod vehicle {
    use super::*;
    #[repr(C)]
    #[derive(Debug, Clone, PartialEq, Default, Reloc, CommData)]
    #[comm_data(id = "SpeedSample")]
    pub struct SpeedSample {
        pub value: f64,
        pub quality: u8,
    }
    #[repr(C)]
    #[derive(Debug, Clone, PartialEq, Default, Reloc, CommData)]
    #[comm_data(id = "CruiseControlState")]
    pub struct CruiseControlState {
        pub target_speed: f64,
        pub enabled: bool,
    }
    #[repr(C)]
    #[derive(Debug, Clone, PartialEq, Default, Reloc, CommData)]
    #[comm_data(id = "TargetSpeed")]
    pub struct TargetSpeed {
        pub value: f64,
    }
    #[repr(C)]
    #[derive(Debug, Clone, PartialEq, Default, Reloc, CommData)]
    #[comm_data(id = "CruiseControlEnabled")]
    pub struct CruiseControlEnabled {
        pub value: bool,
    }
}
