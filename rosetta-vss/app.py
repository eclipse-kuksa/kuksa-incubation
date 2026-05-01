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

from app_modules.ui import render_sidebar, render_integration_workspace, render_debug_tools
from app_modules.logic import process_file
from app_modules.state import init_session_state
from app_modules.config import PAGE_CONFIG
from vss_mapper import VSSKnowledgeBase, create_generator
import streamlit as st
import logging
from pathlib import Path

# Setup file-based logging
LOG_DIR = Path("logs")
LOG_DIR.mkdir(exist_ok=True)
LOG_FILE = LOG_DIR / "app.log"

logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s | %(levelname)-7s | %(message)s',
    handlers=[
        logging.FileHandler(LOG_FILE),
        logging.StreamHandler()
    ]
)

# UI Components


# 1. Setup and Config of UI
st.set_page_config(**PAGE_CONFIG)
init_session_state()

# 2. Sidebar
render_sidebar()

# Stop execution if no standard is loaded yet
if not st.session_state.vss_data:
    st.info("👈 Please Load a VSS Standard in the sidebar to begin.")
    st.stop()

# 3. Init Knowledge Base


@st.cache_resource(hash_funcs={dict: lambda d: str(d)})
def load_models(data):
    return VSSKnowledgeBase(data)


with st.spinner("Initializing Knowledge Base ..."):
    kb = load_models(st.session_state.vss_data)

st.success(f"✅ Knowledge Base ready: {len(kb.signals)} VSS signals indexed")

# Initialize LLM Wrapper
llm = create_generator(hf_token=st.session_state.hf_token)

# 4. Main UI Space
st.title("🗿 Rosetta VSS")
st.caption("LLM-based Automotive Signal to VSS Mapper using hybrid retrieval + reranking")

# Inputs & Controls
st.subheader("1. Upload Signal Artefact")
uploaded = st.file_uploader(
    "Upload your signal definition file",
    help="Supported: CAN DBC, Excel, CSV, AUTOSAR ARXML, ROS YAML, C/C++ source",
    type=["dbc", "xlsx", "xls", "csv", "arxml", "yaml", "yml", "c", "cpp", "h", "hpp"],
)

col1, col2 = st.columns([3, 1])
with col1:
    threshold = st.slider(
        "Proposal Threshold",
        min_value=0.0,
        max_value=1.0,
        value=kb.config.low_confidence_threshold,
        step=0.05,
        help="Below cutoff → proposal/manual review"
    )
with col2:
    st.metric("Threshold", f"{threshold:.1f}")

if st.button("🚀 Run Mapping", type="primary", width="stretch"):
    if uploaded:
        with st.spinner("Processing signals..."):
            process_file(uploaded, kb, llm, threshold)
        st.success("✅ Mapping complete!")
    else:
        st.warning("Please upload a file first.")

# --- Section 2: Integration Workspace ---
render_integration_workspace(kb)

# --- Section 3: Debug Tools ---
render_debug_tools(kb)

# --- Footer ---
st.divider()
st.caption("Anonymous for now | Hybrid RAG + LLM-as-Judge Architecture")
