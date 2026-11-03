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

pub mod vehicle {
    use score_com::{ProviderInfo, Publisher, Subscriber};

    score_com::interface!(interface VehicleDynamicsService {
        Id = "VehicleDynamicsService",
        speed: Event<crate::mw_com_provider_types::vehicle::SpeedSample>,
        target_speed: Event<crate::mw_com_provider_types::vehicle::TargetSpeed>,
        cruise_control_enabled: Event<crate::mw_com_provider_types::vehicle::CruiseControlEnabled>,
    });
}
