/********************************************************************************
* Copyright (c) 2026 Contributors to the Eclipse Foundation
*
* This program and the accompanying materials are made available under the
* terms of the Apache License 2.0 which is available at
* http://www.apache.org/licenses/LICENSE-2.0
*
* SPDX-License-Identifier: Apache-2.0
********************************************************************************/

/*
 * Thin wrapper exporting static inline functions from Open1722 as callable symbols.
 *
 * Open1722 declares many performance-critical functions as `static inline` in
 * headers. They are compiled into the calling translation unit and never
 * appear as exported symbols in the shared libraries. This wrapper provides
 * exported trampolines so that foreign language bindings (Rust FFI, Python
 * ctypes) can call them without reimplementing the wire-format logic.
 *
 * Build: see CMakeLists.txt in this directory.
 */

#include "avtp/CommonHeader.h"
#include "avtp/acf/AcfCommon.h"
#include "avtp/acf/Ntscf.h"
#include "avtp/acf/Tscf.h"
#include "avtp/acf/custom/Vss.h"

// -- PDU decoding (used by the listener) --

uint8_t open1722_ffi_common_header_get_subtype(const Avtp_CommonHeader_t *pdu) {
    return Avtp_CommonHeader_GetSubtype(pdu);
}

Avtp_AcfMsgType_t open1722_ffi_acf_common_get_acf_msg_type(const Avtp_AcfCommon_t *pdu) {
    return Avtp_AcfCommon_GetAcfMsgType(pdu);
}

uint16_t open1722_ffi_ntscf_get_data_length(const Avtp_Ntscf_t *pdu) {
    return Avtp_Ntscf_GetNtscfDataLength(pdu);
}

uint16_t open1722_ffi_tscf_get_stream_data_length(const Avtp_Tscf_t *pdu) {
    return Avtp_Tscf_GetStreamDataLength(pdu);
}

// -- PDU encoding (used by the gateway / talker) --

void open1722_ffi_ntscf_init(Avtp_Ntscf_t *pdu) {
    Avtp_Ntscf_Init(pdu);
}

void open1722_ffi_ntscf_set_sequence_num(Avtp_Ntscf_t *pdu, uint8_t value) {
    Avtp_Ntscf_SetSequenceNum(pdu, value);
}

void open1722_ffi_ntscf_set_stream_id(Avtp_Ntscf_t *pdu, uint64_t value) {
    Avtp_Ntscf_SetStreamId(pdu, value);
}

void open1722_ffi_ntscf_set_data_length(Avtp_Ntscf_t *pdu, uint16_t value) {
    Avtp_Ntscf_SetNtscfDataLength(pdu, value);
}
