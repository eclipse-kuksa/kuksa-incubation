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

#include "common.h"
#include "grpc.h"
#include "nvs_flash.h"
#include "protocol_examples_common.h"
#include "esp_netif.h"
#include "esp_wifi.h"
#include "esp_event.h"
#include <sys/time.h>
#include <pb_encode.h>
#include <pb_decode.h>
#include "generated/val.pb.h"
#include "generated/types.pb.h"
#include "esp_log.h"
#include "encoder.h"
#include "decoder.h"

// ------------------------------------------------------------------------------------------
static const char *TAG = "MAIN";
bool session_test = false;
const char *grpc_uri = "https://<grpc-server_ip:port>";

// ------------------------------------------------------------------------------------------

// Get Reqeust
// ------------------
#define MESSAGEPB_PATH "/kuksa.val.v1.VAL"
#define MESSAGEPB_REQUEST "Get"
// ------------------

// Set Reqeust
// ------------------
// #define MESSAGEPB_PATH "/kuksa.val.v1.VAL"
// #define MESSAGEPB_REQUEST "Set"
// ------------------

void app_main()
{

    ESP_ERROR_CHECK(nvs_flash_init());
    ESP_ERROR_CHECK(esp_netif_init());
    ESP_ERROR_CHECK(esp_event_loop_create_default());

    /* This helper function configures Wi-Fi or Ethernet, as selected in menuconfig.
     * Read "Establishing Wi-Fi or Ethernet Connection" section in
     * examples/protocols/README.md for more information about this function.
     */
    ESP_ERROR_CHECK(example_connect());

    ESP_LOGI(TAG, "Initializing the gRPC connection...");

    grpc_init_t grpc_cfg = {
        .grpc_core = 1,
        .grpc_stack_size = 8000,
        .grpc_prio = 10,
        .http2_core = 1,
        .http2_stack_size = 22000,
        .http2_prio = 11,
    };

    grpc_init(grpc_cfg);
    ESP_LOGI(TAG, "completed the configuration");

    grpc_conn_data_t grpc_dat = {
        .ca = "",
        .uri = grpc_uri,
    };

    ESP_LOGD(TAG, "conn data: %s", grpc_dat.uri);

    grpc_configure_connection(grpc_dat);

    grpc_connect();

    // @TEST: GRPC
    for (;;)
    {
        static bool pinged = false;
        static bool conn_prior = false;

        bool conn = grpc_connected();

        if (conn && !conn_prior)
        {
            pinged = true;
            session_test = false;
        }

        if (conn)
        {
            if (!pinged)
            {
                int64_t rtt = 0;
                bool ret = grpc_ping(1000, &rtt);
                if (ret)
                {
                    pinged = true;
                    int rtt_ms = rtt / 1000;
                    ESP_LOGI(TAG, "ping time: %d", rtt_ms);
                }
            }

            if (!session_test)
            {

                // -------------------------------------------------------------------------------------------------------
                // See get request example below:
                // -------------------------------------------------------------------------------------------------------

                uint8_t buffer[256];
                size_t message_length;

                EntryRequest message = {
                    .path = "Vehicle.Speed",
                    .view = kuksa_val_v1_View_VIEW_CURRENT_VALUE,
                };

                kuksa_val_v1_EntryRequest entry_request = init_entry_request(&message);

                kuksa_val_v1_GetRequest get_request;
                get_request.entries.funcs.encode = encode_entries_callback;
                get_request.entries.arg = &entry_request;

                pb_ostream_t stream = pb_ostream_from_buffer(buffer, sizeof(buffer));

                // Encode the GetRequest
                if (!pb_encode(&stream, kuksa_val_v1_GetRequest_fields, &get_request))
                {
                    ESP_LOGE(TAG, "Encoding failed: %s", PB_GET_ERROR(&stream));
                    free(entry_request.path.arg);
                    return;
                }

                message_length = stream.bytes_written;
                ESP_LOGI(TAG, "Encoded GetRequest with length %zu bytes\n", message_length);

                free(entry_request.path.arg);

                grpc_call_proc(MESSAGEPB_PATH, MESSAGEPB_REQUEST, buffer, message_length);

                vTaskDelay(2000);

                uint8_t *resp_buf = grpc_get_buffer_data();     // Buffer for response
                size_t resp_buf_len = grpc_get_buffer_length(); // Populate with actual response length

                if (decode_GetResponse(resp_buf, resp_buf_len))
                {
                    ESP_LOGI(TAG, "Decoding was successful");
                }
                else
                {
                    ESP_LOGE(TAG, "Decoding failed");
                }

                // -------------------------------------------------------------------------------------------------------
                // See set request example below:
                // -------------------------------------------------------------------------------------------------------

                // uint8_t buffer[128];  // Buffer to hold the encoded data
                // size_t message_length;  // Variable to store the message length after encoding
                //  kuksa_val_v1_Datapoint datapoint = create_datapoint(62.0);
                // kuksa_val_v1_DataEntry data_entry = create_data_entry("Vehicle.Speed", datapoint);
                // kuksa_val_v1_EntryUpdate entry_update = create_entry_update(data_entry);
                // kuksa_val_v1_EntryUpdate updates[] = {entry_update};

                // kuksa_val_v1_SetRequest set_request = create_set_request(updates);

                // if (encode_set_request(&set_request, buffer, sizeof(buffer), &message_length)) {
                //     printf("SetRequest encoded successfully, length = %zu\n", message_length);
                // }

                // log_buffer_content(buffer, message_length);

                // grpc_call_proc(MESSAGEPB_PATH,MESSAGEPB_REQUEST, buffer, message_length);

                session_test = true;
                conn_prior = conn;
            }
        }

        conn_prior = conn;
        vTaskDelay(100);
    }
}
