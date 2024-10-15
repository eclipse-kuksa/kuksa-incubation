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

use zenoh::buffers::ZBuf;
use zenoh::prelude::*;

pub fn split_once(s: &str, c: char) -> (&[u8], &[u8]) {
    let s_bytes = s.as_bytes();
    match s.find(c) {
        Some(index) => {
            let (l, r) = s_bytes.split_at(index);
            (l, &r[1..])
        }
        None => (s_bytes, &[]),
    }
}

pub fn zbuf_to_string(zbuf: &ZBuf) -> Result<String, std::str::Utf8Error> {
    let mut bytes = Vec::new();
    for zslice in zbuf.zslices() {
        bytes.extend_from_slice(zslice.as_slice());
    }
    String::from_utf8(bytes).map_err(|e| e.utf8_error())
}

pub fn extract_attachment_as_string(sample: &Sample) -> String {
    if let Some(attachment) = sample.attachment() {
        let bytes = attachment.iter().next().unwrap();
        String::from_utf8_lossy(bytes.1.as_slice()).to_string()
    } else {
        String::new()
    }
}
