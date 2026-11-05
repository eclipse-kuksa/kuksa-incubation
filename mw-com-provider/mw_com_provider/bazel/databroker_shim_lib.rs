/*
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
 */

#[path = "../../databroker/src/broker.rs"]
pub mod broker;
#[path = "../../databroker/src/filter/mod.rs"]
pub mod filter;
#[path = "../../databroker/src/glob.rs"]
pub mod glob;
#[path = "../../databroker/src/permissions.rs"]
pub mod permissions;
#[path = "../../databroker/src/types.rs"]
pub mod types;

pub mod open_telemetry {
    pub fn broadcast_drop_counter() -> Option<()> {
        None
    }
}
