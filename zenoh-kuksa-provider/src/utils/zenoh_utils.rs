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

use zenoh::{bytes::ZBytes, sample::Sample};

pub fn zbytes_to_string(zbuf: &ZBytes) -> Result<String, std::str::Utf8Error> {
    zbuf.try_to_string().map(|v| v.to_string())
}

pub fn extract_attachment_as_string(sample: &Sample) -> Option<String> {
    sample
        .attachment()
        .and_then(|a| a.try_to_string().map(|v| v.to_string()).ok())
}
