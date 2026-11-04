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

use crate::generated;

pub fn binding_status() -> &'static str {
    generated::describe_generated_bindings()
}

pub fn struct_field_accessors() -> &'static [generated::GeneratedStructFieldAccessor] {
    generated::GENERATED_STRUCT_FIELD_ACCESSORS
}

pub fn generic_mw_com_elements() -> &'static [generated::GenericMwComElement] {
    generated::GENERATED_GENERIC_MW_COM_ELEMENTS
}

pub fn find_struct_field_accessor(
    struct_name: &str,
    field_name: &str,
) -> Option<&'static generated::GeneratedStructFieldAccessor> {
    generated::find_struct_field_accessor(struct_name, field_name)
}

pub fn find_generic_mw_com_element(
    service_name: &str,
    instance_name: &str,
    member_name: &str,
) -> Option<&'static generated::GenericMwComElement> {
    generated::find_generic_mw_com_element(service_name, instance_name, member_name)
}
