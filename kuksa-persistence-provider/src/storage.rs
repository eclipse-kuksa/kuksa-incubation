/********************************************************************************
* Copyright (c) 2024 Contributors to the Eclipse Foundation
*
* This program and the accompanying materials are made available under the
* terms of the Apache License 2.0 which is available at
* http://www.apache.org/licenses/LICENSE-2.0
*
* SPDX-License-Identifier: Apache-2.0
********************************************************************************/

pub mod filestorage;

use std::sync::mpsc::Sender;

pub use filestorage::FileStorage;
use tinyjson::JsonValue;

pub struct StoreItem {
    pub path: String,
    pub value: String,
}

pub trait Storage {
    fn new(config: &JsonValue) -> Self;

    fn get(&self, vsspath: &str) -> Option<&str>;

    fn set(&self, vsspath: &str, vssvalue: &str) -> Result<(), ()>;

    fn get_queue(&self) -> Sender<StoreItem>;
}
