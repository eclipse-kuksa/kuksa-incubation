// #################################################################################
// # Copyright (c) 2024 Contributors to the Eclipse Foundation
// #
// # See the NOTICE file(s) distributed with this work for additional
// # information regarding copyright ownership.
// #
// # This program and the accompanying materials are made available under the
// # terms of the Apache License 2.0 which is available at
// # http://www.apache.org/licenses/LICENSE-2.0
// #
// # SPDX-License-Identifier: Apache-2.0
// #################################################################################

#pragma once

#include <stddef.h>
#include <stdbool.h>
#include <pb_encode.h>
#include <pb_decode.h>
#include "generated/val.pb.h"
#include "generated/types.pb.h"

bool decode_GetResponse(const uint8_t *buffer, size_t buffer_len);

void decode_DataEntry(pb_istream_t *stream, const pb_field_t fields[], void *const *arg);

void decode_Datapoint(pb_istream_t *stream, const pb_field_t fields[], void *const *arg);

void decode_Timestamp(pb_istream_t *stream, const pb_field_t fields[], void *const *arg);

void handle_GetResponse_Error(pb_istream_t *stream, const pb_field_t fields[], void *const *arg);
