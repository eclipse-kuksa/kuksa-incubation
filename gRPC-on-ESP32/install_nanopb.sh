#################################################################################
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
#################################################################################

#!/bin/bash

URL="https://jpa.kapsi.fi/nanopb/download/nanopb-0.4.8-linux-x86.tar.gz"
TARGET_DIR="components"
FINAL_NAME="nanopb"
CMAKELISTS="$TARGET_DIR/nanopb/CMakeLists.txt"

# Download and extract the main package
curl -o "nanopb-0.4.8-linux-x86.tar.gz" $URL
mkdir -p "$TARGET_DIR"
tar -xzf "nanopb-0.4.8-linux-x86.tar.gz" -C "$TARGET_DIR"
mv "$TARGET_DIR/nanopb-0.4.8-linux-x86" "$TARGET_DIR/$FINAL_NAME"
rm "nanopb-0.4.8-linux-x86.tar.gz"


echo "Downloading timestamp protobuf definitions."
mkdir -p "$TARGET_DIR/nanopb/google/protobuf/"
PROTO_FILES=(
    "https://raw.githubusercontent.com/protocolbuffers/protobuf/main/src/google/protobuf/wrappers.proto"
    "https://raw.githubusercontent.com/protocolbuffers/protobuf/main/src/google/protobuf/empty.proto"
    "https://raw.githubusercontent.com/protocolbuffers/protobuf/main/src/google/protobuf/timestamp.proto"
)

for URL in "${PROTO_FILES[@]}"; do
    FILE_NAME=$(basename $URL)
    curl -o "proto/$FILE_NAME" $URL
done

# Define CMake configuration and overwrite
CMAKE_CONFIG='idf_component_register(SRCS
                    "pb_common.c"
                    "pb_encode.c"
                    "pb_decode.c"
                    "google/protobuf/empty.pb.c"
                    "google/protobuf/timestamp.pb.c"
                    "google/protobuf/wrappers.pb.c"
                    INCLUDE_DIRS .
                    )'

# Write the configuration to CMakeLists.txt
echo "$CMAKE_CONFIG" > "$CMAKELISTS"

echo "CMakeLists.txt has been updated."

echo "Installation of nanopb is complete."
