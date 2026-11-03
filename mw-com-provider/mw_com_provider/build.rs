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

use std::env;
use std::fs;
use std::path::PathBuf;

const DEFAULT_GENERATED_METADATA: &str = "generated/mw_com_provider_metadata.rs";

fn main() {
    println!("cargo:rerun-if-env-changed=MW_COM_PROVIDER_METADATA_RS");

    copy_generated_metadata();
}

fn copy_generated_metadata() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must be set by Cargo"));
    let generated_file = out_dir.join("mw_com_generated.rs");
    let metadata_path = env::var("MW_COM_PROVIDER_METADATA_RS")
        .unwrap_or_else(|_| DEFAULT_GENERATED_METADATA.to_string());

    println!("cargo:rerun-if-changed={metadata_path}");

    let content = fs::read_to_string(&metadata_path).unwrap_or_else(|_| default_metadata());
    fs::write(generated_file, content).expect("failed to write generated mw::com metadata");
}

fn default_metadata() -> String {
    r#"/********************************************************************************
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

pub const MW_COM_SERVICE_DEFS: &str = "unset";
pub const MW_COM_BINDINGS_AVAILABLE: bool = false;

pub fn describe_generated_bindings() -> &'static str {
    ""
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct GeneratedStructFieldAccessor {
    pub struct_name: &'static str,
    pub field_name: &'static str,
    pub datatype: &'static str,
    pub size: usize,
    pub alignment: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct GenericMwComElement {
    pub service_name: &'static str,
    pub instance_name: &'static str,
    pub instance_specifier: &'static str,
    pub member_name: &'static str,
    pub element_kind: &'static str,
    pub payload_type: &'static str,
    pub sample_size: usize,
    pub sample_alignment: usize,
    pub has_serialized_format: bool,
}

pub const GENERATED_STRUCT_FIELD_ACCESSORS: &[GeneratedStructFieldAccessor] = &[];
pub const GENERATED_GENERIC_MW_COM_ELEMENTS: &[GenericMwComElement] = &[];

pub fn find_struct_field_accessor(
    struct_name: &str,
    field_name: &str,
) -> Option<&'static GeneratedStructFieldAccessor> {
    GENERATED_STRUCT_FIELD_ACCESSORS
        .iter()
        .find(|accessor| accessor.struct_name == struct_name && accessor.field_name == field_name)
}

pub fn find_generic_mw_com_element(
    service_name: &str,
    instance_name: &str,
    member_name: &str,
) -> Option<&'static GenericMwComElement> {
    GENERATED_GENERIC_MW_COM_ELEMENTS.iter().find(|element| {
        element.service_name == service_name
            && (element.instance_name == instance_name || element.instance_specifier == instance_name)
            && element.member_name == member_name
    })
}
"#
    .to_string()
}
