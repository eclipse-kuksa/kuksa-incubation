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
#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>

typedef struct
{
    int grpc_core;
    int grpc_stack_size;
    int grpc_prio;

    int http2_core;
    int http2_stack_size;
    int http2_prio;
} grpc_init_t;

typedef struct
{
    const char *ca;
    const char *uri;
} grpc_conn_data_t;

// https://grpc.github.io/grpc/core/md_doc_statuscodes.html
typedef enum
{
    GRPC_SC_OK,
    GRPC_SC_CANCELLED,
    GRPC_SC_UNKNOWN,
    GRPC_SC_INVALID_ARGUMENT,
    GRPC_SC_DEADLINE_EXCEEDED,
    GRPC_SC_NOT_FOUND,
    GRPC_SC_ALREADY_EXISTS,
    GRPC_SC_PERMISSION_DENIED,
    GRPC_SC_RESOURCE_EXHAUSTED,
    GRPC_SC_FAILED_PRECONDITION,
    GRPC_SC_ABORTED,
    GRPC_SC_OUT_OF_RANGE,
    GRPC_SC_UNIMPLEMENTED,
    GRPC_SC_INTERNAL,
    GRPC_SC_UNAVAILABLE,
    GRPC_SC_DATA_LOSS,
    GRPC_SC_UNAUTHENTICATED
} GRPCStatusCode;

bool grpc_init(grpc_init_t config);
bool grpc_configure_connection(grpc_conn_data_t connection_data);

bool grpc_connect();
bool grpc_connected();
bool grpc_wait_for_connection(int timeout_ms);
bool gprc_send_message_pending();
bool grpc_call_proc(char *path, char *proc, uint8_t *data, uint32_t len);
bool grpc_ping(int timeout_ms, int64_t *_ping_time);
const char *grpc_status_code_to_str(GRPCStatusCode sc);

uint8_t *grpc_get_buffer_data(void);
size_t grpc_get_buffer_length(void);
