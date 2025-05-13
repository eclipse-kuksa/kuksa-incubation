/********************************************************************************
 * Copyright (c) 2024 Contributors to the Eclipse Foundation
 *
 * See the NOTICE file(s) distributed with this work for additional
 * information regarding copyright ownership.
 *
 * This program and the accompanying materials are made available under the
 * terms of the Apache License 2.0 which is available at
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * SPDX-License-Identifier: Apache-2.0
 ********************************************************************************/

use std::collections::HashMap;
use std::sync::Arc;
use kuksa_rust_sdk::v1_proto::DataType;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy)]
pub struct MetadataInfo {
    pub data_type: DataType,
}

pub type MetadataStore = Arc<Mutex<HashMap<String, MetadataInfo>>>;

pub fn create_metadata_store() -> MetadataStore {
    Arc::new(Mutex::new(HashMap::new()))
}
