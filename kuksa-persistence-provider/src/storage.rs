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

#[derive(Debug)]
pub struct StoreItem {
    pub path: String,
    pub value: String,
}

#[derive(Default, Debug, PartialEq)]
pub struct StorageConfig {
    pub storagetype: StorageType,
}

#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub enum StorageType {
    FileStorageType(FileStorageType),
    // Add more storage types here
}

#[derive(Debug, PartialEq)]
pub struct FileStorageType {
    pub filepath: String,
}

impl Default for StorageType {
    fn default() -> Self {
        StorageType::FileStorageType(FileStorageType::default())
    }
}

impl Default for FileStorageType {
    fn default() -> Self {
        FileStorageType {
            filepath: "storage.json".to_string(),
        }
    }
}

pub trait Storage {
    fn new(config: StorageConfig) -> Self where Self: Sized;

    fn get(&self, vsspath: &str) -> Option<&str>;

    fn set(&self, vsspath: &str, vssvalue: &str) -> Result<(), ()>;

    fn get_queue(&self) -> Sender<StoreItem>;
}
