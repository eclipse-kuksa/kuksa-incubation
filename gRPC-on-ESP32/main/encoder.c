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

#include <stdio.h>
#include "esp_log.h"
#include <pb_encode.h>
#include <pb_decode.h>
#include "generated/val.pb.h"
#include "generated/types.pb.h"
#include "encoder.h"

static const char *TAG = "ENCODER";

void log_buffer_content(const uint8_t *buffer, size_t length)
{
    printf("Encoded Buffer: ");
    for (size_t i = 0; i < length; i++)
    {
        printf("%02X ", buffer[i]);
    }
    printf("\n");
}

bool get_server_info(uint8_t *buffer, size_t buffer_size, size_t *message_length)
{
    bool status;

    kuksa_val_v1_GetServerInfoRequest get_server_info_request = kuksa_val_v1_GetServerInfoRequest_init_zero;

    pb_ostream_t stream = pb_ostream_from_buffer(buffer, buffer_size);

    status = pb_encode(&stream, kuksa_val_v1_GetServerInfoRequest_fields, &get_server_info_request);
    *message_length = stream.bytes_written;

    if (!status)
    {
        printf("Encoding failed: %s\n", PB_GET_ERROR(&stream));
        return false;
    }

    return status;
}

bool encode_fields_array(pb_ostream_t *stream, const pb_field_t *field, void *const *arg)
{
    uint32_t *fields = (uint32_t *)*arg;
    size_t count = 1; // Change this as necessary to match the number of fields you are encoding

    for (size_t i = 0; i < count; i++)
    {
        if (!pb_encode_tag_for_field(stream, field))
        {
            return false;
        }
        if (!pb_encode_varint(stream, fields[i]))
        {
            return false;
        }
    }
    return true;
}

bool encode_entries_callback(pb_ostream_t *stream, const pb_field_t *field, void *const *arg)
{
    kuksa_val_v1_EntryRequest *request = (kuksa_val_v1_EntryRequest *)*arg;
    if (!pb_encode_tag_for_field(stream, field))
        return false;
    if (!pb_encode_submessage(stream, kuksa_val_v1_EntryRequest_fields, request))
        return false;
    return true;
}

bool encode_entry_request_callback(pb_ostream_t *stream, const pb_field_t *field, void *const *arg)
{
    kuksa_val_v1_EntryRequest *entry_requests = *arg;
    int num_entries = 1; // Example number of entries

    for (int i = 0; i < num_entries; i++)
    {
        if (!pb_encode_tag_for_field(stream, field))
            return false;
        if (!pb_encode_submessage(stream, kuksa_val_v1_EntryRequest_fields, &entry_requests[i]))
            return false;
    }
    return true;
}

bool callback_string_encoder(pb_ostream_t *stream, const pb_field_t *field, void *const *arg)
{
    const char *str = (char *)*arg;
    if (!pb_encode_tag_for_field(stream, field))
    {
        return false;
    }
    return pb_encode_string(stream, (const uint8_t *)str, strlen(str));
}

kuksa_val_v1_EntryRequest init_entry_request(EntryRequest *req)
{
    kuksa_val_v1_EntryRequest request = kuksa_val_v1_EntryRequest_init_zero;

    // Properly initialize 'path'
    request.path.funcs.encode = callback_string_encoder;
    request.path.arg = strdup(req->path); // Allocate and set path
    if (!request.path.arg)
    {
        ESP_LOGE(TAG, "Failed to allocate memory for path");
        abort();
    }

    request.view = req->view;

    // TODO: Set up fields values

    return request;
}

// Set requests
// ------------------------------------------------------------------------------------------

bool encode_updates_array(pb_ostream_t *stream, const pb_field_t *field, void *const *arg)
{
    kuksa_val_v1_EntryUpdate *updates = (kuksa_val_v1_EntryUpdate *)*arg;
    for (size_t i = 0; i < 1; i++)
    { // Change 1 to the number of updates if dynamic
        if (!pb_encode_tag_for_field(stream, field))
        {
            return false;
        }
        if (!pb_encode_submessage(stream, kuksa_val_v1_EntryUpdate_fields, &updates[i]))
        {
            return false;
        }
    }
    return true;
}

kuksa_val_v1_Datapoint create_datapoint(float value)
{
    kuksa_val_v1_Datapoint datapoint = kuksa_val_v1_Datapoint_init_zero;
    datapoint.which_value = kuksa_val_v1_Datapoint__float_tag;
    datapoint.value._float = value;
    return datapoint;
}

kuksa_val_v1_DataEntry create_data_entry(const char *path, kuksa_val_v1_Datapoint datapoint)
{
    kuksa_val_v1_DataEntry data_entry = kuksa_val_v1_DataEntry_init_zero;
    data_entry.path.funcs.encode = callback_string_encoder;
    data_entry.path.arg = (void *)path;
    data_entry.has_value = true;
    data_entry.value = datapoint;
    return data_entry;
}

kuksa_val_v1_EntryUpdate create_entry_update(kuksa_val_v1_DataEntry data_entry)
{
    kuksa_val_v1_EntryUpdate entry_update = kuksa_val_v1_EntryUpdate_init_zero;
    entry_update.has_entry = true;
    entry_update.entry = data_entry;

    static uint32_t fields_array[] = {2}; // Assuming FIELD_VALUE is '2' -> This translates to current_value
    entry_update.fields.funcs.encode = encode_fields_array;
    entry_update.fields.arg = fields_array;

    return entry_update;
}

kuksa_val_v1_SetRequest create_set_request(kuksa_val_v1_EntryUpdate *updates)
{
    kuksa_val_v1_SetRequest set_request = kuksa_val_v1_SetRequest_init_zero;
    set_request.updates.funcs.encode = encode_updates_array;
    set_request.updates.arg = updates;
    return set_request;
}

bool encode_set_request(const kuksa_val_v1_SetRequest *set_request, uint8_t *buffer, size_t buffer_size, size_t *message_length)
{
    pb_ostream_t stream = pb_ostream_from_buffer(buffer, buffer_size);
    bool status = pb_encode(&stream, kuksa_val_v1_SetRequest_fields, set_request);
    if (!status)
    {
        printf("Encoding failed: %s\n", PB_GET_ERROR(&stream));
        return false;
    }
    *message_length = stream.bytes_written;
    return true;
}
