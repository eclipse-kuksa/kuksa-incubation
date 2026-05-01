# ********************************************************************************
# Copyright (c) 2026 Contributors to the Eclipse Foundation
#
# See the NOTICE file(s) distributed with this work for additional
# information regarding copyright ownership.
#
# This program and the accompanying materials are made available under the
# terms of the Apache License 2.0 which is available at
# http://www.apache.org/licenses/LICENSE-2.0
#
# SPDX-License-Identifier: Apache-2.0
# *******************************************************************************/

import streamlit as st


def init_session_state():
    """
    Initializes all session state variables if they don't exist.
    """
    defaults = {
        'vss_data': None,
        'results': [],
        'proposals': [],
        'hf_token': "",
        'logs': [],
    }

    for key, value in defaults.items():
        if key not in st.session_state:
            st.session_state[key] = value
