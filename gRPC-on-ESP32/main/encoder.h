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

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#include <pb_encode.h>
#include <pb_decode.h>
#include "generated/val.pb.h"
#include "generated/types.pb.h"

typedef struct
{
    char *path;
    kuksa_val_v1_View view;
    // kuksa_val_v1_Field *field;
} EntryRequest;

void log_buffer_content(const uint8_t *buffer, size_t length);
bool encode_string(pb_ostream_t *stream, const pb_field_t *field, void *const *arg);
bool get_server_info(uint8_t *buffer, size_t buffer_size, size_t *message_length);
bool kuksa_get_request(uint8_t *buffer, size_t buffer_size, size_t *message_length);
bool should_serialize_field(const kuksa_val_v1_EntryRequest *request, kuksa_val_v1_Field field);
bool encode_get_request(kuksa_val_v1_GetRequest *request, uint8_t *buffer, size_t buffer_size, size_t *message_length);
kuksa_val_v1_EntryRequest init_entry_request(EntryRequest *req);
bool callback_string_encoder(pb_ostream_t *stream, const pb_field_t *field, void *const *arg);
bool encode_entries_callback(pb_ostream_t *stream, const pb_field_t *field, void *const *arg);

kuksa_val_v1_Datapoint create_datapoint(float value);
kuksa_val_v1_DataEntry create_data_entry(const char *path, kuksa_val_v1_Datapoint datapoint);
kuksa_val_v1_EntryUpdate create_entry_update(kuksa_val_v1_DataEntry data_entry);
kuksa_val_v1_SetRequest create_set_request(kuksa_val_v1_EntryUpdate *updates);
bool encode_set_request(const kuksa_val_v1_SetRequest *set_request, uint8_t *buffer, size_t buffer_size, size_t *message_length);
