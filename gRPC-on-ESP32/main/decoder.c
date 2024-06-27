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
#include <stddef.h>
#include <stdbool.h>
#include <pb_encode.h>
#include <pb_decode.h>
#include "generated/val.pb.h"
#include "generated/types.pb.h"
#include "esp_log.h"

static const char *TAG = "DECODER";

void print_datapoint(const kuksa_val_v1_Datapoint *datapoint)
{

    switch (datapoint->which_value)
    {
    case 11:
        printf("String value: %s\n", (char *)(datapoint->value.string.arg));
        break;
    case 12:
        printf("Boolean value: %s\n", datapoint->value._bool ? "true" : "false");
        break;
    case 13:
        printf("Int32 value: %d\n", datapoint->value.int32);
        break;
    case 14:
        printf("Int64 value: %lld\n", datapoint->value.int64);
        break;
    case 15:
        printf("Uint32 value: %u\n", datapoint->value.uint32);
        break;
    case 16:
        printf("Uint64 value: %llu\n", datapoint->value.uint64);
        break;
    case 17:
        printf("Float value: %f\n", datapoint->value._float);
        break;
    case 18: // double
        printf("Double value: %f\n", datapoint->value._double);
        break;
    // TODO: implement functions to print the array data format
    default:
        printf("Unknown or uninitialized value type.\n");
        break;
    }
}

// Callback for encoding/decoding a dynamically allocated string
bool print_string(pb_istream_t *stream, const pb_field_t *field, void **arg)
{
    uint8_t buffer[1024] = {0};

    /* We could read block-by-block to avoid the large buffer... */
    if (stream->bytes_left > sizeof(buffer) - 1)
        return false;

    if (!pb_read(stream, buffer, stream->bytes_left))
        return false;

    printf((char *)*arg, buffer);
    return true;
}

bool decode_DataEntry(pb_istream_t *stream, const pb_field_t *fields, void **arg)
{
    if (arg == NULL)
    {
        ESP_LOGE(TAG, "Error: 'arg' pointer is NULL.");
        return false;
    }

    kuksa_val_v1_DataEntry *entry = *arg;

    if (entry == NULL)
    {
        ESP_LOGE(TAG, "Error: 'entry' pointer is NULL.");
        return false;
    }

    ESP_LOGI(TAG, "decoding data entry");
    if (!pb_decode(stream, kuksa_val_v1_DataEntry_fields, entry))
    {
        ESP_LOGE(TAG, "Failed to decode entry.");
        return false;
    }

    if (entry->has_value)
    {
        print_datapoint(&entry->value);
    }

    return true;
}

bool decode_GetResponse(const uint8_t *buffer, size_t buffer_len)
{
    pb_istream_t stream = pb_istream_from_buffer(buffer, buffer_len);
    kuksa_val_v1_GetResponse response = kuksa_val_v1_GetResponse_init_zero;
    kuksa_val_v1_DataEntry entry = {};

    entry.path.funcs.decode = &print_string;
    entry.path.arg = "Path: \"%s\" \n";

    // Set up a decode callback for the repeated DataEntry field
    response.entries.funcs.decode = &decode_DataEntry;
    response.entries.arg = &entry;

    // Decode the message
    if (!pb_decode(&stream, kuksa_val_v1_GetResponse_fields, &response))
    {
        ESP_LOGE(TAG, "Failed to decode GetResponse");

        return false;
    }

    if (response.has_error)
    {
        ESP_LOGE(TAG, "Global Error Code: %u, Reason: %s", response.error.code, response.error.reason);
    }

    return true;
}
