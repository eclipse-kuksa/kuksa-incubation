 #!/bin/bash
 #
 # Copyright (c) 2024 Contributors to the Eclipse Foundation
 #
 # See the NOTICE file(s) distributed with this work for additional
 # information regarding copyright ownership.
 #
 # This program and the accompanying materials are made available under the
 # terms of the Apache License 2.0 which is available at
 # http://www.apache.org/licenses/LICENSE-2.0
 #
 # SPDX-License-Identifier: Apache-2.0
 #

 # This script builds the can-protocol-adapter for specified architectures in either debug or release mode.
 # It also runs basic cargo quality checks like clippy, audit, and deny for code quality and security.

# Define the target directory
TARGET_DIR="target/logs"

# Create the target directory if it doesn't exist
mkdir -p $TARGET_DIR

CLIPPY_LOG="$TARGET_DIR/clippy_report.log"
AUDIT_LOG="$TARGET_DIR/audit_report.log"
DENY_LOG="$TARGET_DIR/deny_report.log"
BUILD_LOG="$TARGET_DIR/build_output.log"

display_help() {
  echo "Usage: $0 [TARGET] [--release]"
  echo "  TARGET: The target platform (e.g. x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu)."
  echo "  --release: Build in release mode.If omitted, builds in debug mode."
}

if [ -z "$1" ] || [[ "$1" == "--help" ]]; then
  display_help
  exit 0  
fi

TARGET="$1"
RELEASE="$2"

# Run cargo clippy with -D warnings
echo "Running cargo clippy..."
cargo clippy -- -D warnings 2>&1 | tee $CLIPPY_LOG
if [ $? -ne 0 ]; then
    echo "Clippy failed! Check $CLIPPY_LOG for details."
    exit 1
fi

# Run cargo audit
echo "Running cargo audit..."
cargo audit 2>&1 | tee $AUDIT_LOG
if [ $? -ne 0 ]; then
    echo "Cargo audit failed! Check $AUDIT_LOG for details."
    exit 1
fi

# If all checks passed, build the project
echo "Building the project..."
cargo build --target $TARGET $RELEASE 2>&1 | tee $BUILD_LOG
if [ $? -ne 0 ]; then
    echo "Build failed! Check $BUILD_LOG for details."
    exit 1
fi

echo "All tasks completed successfully!"
