/********************************************************************************
* Copyright (c) 2026 Contributors to the Eclipse Foundation
*
* This program and the accompanying materials are made available under the
* terms of the Apache License 2.0 which is available at
* http://www.apache.org/licenses/LICENSE-2.0
*
* SPDX-License-Identifier: Apache-2.0
********************************************************************************/

fn main() {
    println!("cargo:rustc-link-search=native=/usr/local/lib");
    println!("cargo:rustc-link-search=native=/tmp/build-ffi");
    println!("cargo:rustc-link-lib=dylib=open1722");
    println!("cargo:rustc-link-lib=dylib=open1722custom");
    println!("cargo:rustc-link-lib=dylib=open1722_ffi");
}
