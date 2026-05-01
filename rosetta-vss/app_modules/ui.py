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

# app_modules/ui.py
import streamlit as st
import requests
import json
import pandas as pd
import yaml
from .config import COVESA_URLS
from vss_mapper import generate_kuksa_config


def render_sidebar():
    st.sidebar.title("⚙️ Initial Setup")

    # API Tokens
    token = st.sidebar.text_input("Hugging Face Token", type="password", value=st.session_state.hf_token)
    if token:
        st.session_state.hf_token = token

    st.sidebar.divider()
    source = st.sidebar.radio("VSS Source:", ["Official COVESA Spec", "Upload Custom Spec"])

    # Load Data Logic
    try:
        if source == "Official COVESA Spec":
            ver = st.sidebar.selectbox("Version:", list(COVESA_URLS.keys()))
            if st.sidebar.button("Fetch Standard"):
                with st.spinner("Downloading..."):
                    resp = requests.get(COVESA_URLS[ver], timeout=30)
                    if resp.status_code == 200:
                        st.session_state.vss_data = resp.json()
                        st.sidebar.success(f"✅ Loaded {len(st.session_state.vss_data)} nodes")
                    else:
                        st.sidebar.error(f"Download failed: {resp.status_code}")
        else:
            f = st.sidebar.file_uploader("Upload Spec", type=["json", "yaml"])
            if f:
                content = f.getvalue().decode("utf-8")
                st.session_state.vss_data = json.loads(content) if f.name.endswith('.json') else yaml.safe_load(content)
                st.sidebar.success("✅ Custom Loaded")
    except Exception as e:
        st.sidebar.error(f"Error: {e}")


def render_integration_workspace(kb):
    st.divider()
    st.subheader("2. Integration Workspace")

    if not st.session_state.results and not st.session_state.proposals:
        st.info("Run mapping above to see results here.")
        return

    df = pd.DataFrame(st.session_state.results)

    # Get all standard paths + any new paths from proposals
    standard_paths = sorted([s.path for s in kb.signals])
    proposed_paths = [p['VSS Match'] for p in st.session_state.results if p['VSS Type'] == 'Proposal']
    all_vss_paths = sorted(list(set(standard_paths + proposed_paths)))

    st.info("👇 Review Mappings. Correct any errors using the dropdowns before exporting.")

    edited_df = st.data_editor(
        df,
        column_config={
            "Source": st.column_config.TextColumn("Source Signal", disabled=True),
            "Source Unit": st.column_config.TextColumn("Src Unit", disabled=True, width="small"),
            "VSS Match": st.column_config.SelectboxColumn("Mapped VSS Path by Rosetta", width="large",
                                                          options=all_vss_paths, required=True),
            "LLM Suggestion": st.column_config.TextColumn("Retrieval Override (if LLM vetos)", disabled=True,
                                                          width="medium"),
            "VSS Type": st.column_config.TextColumn("Type", disabled=True, width="small"),
            "Validation": st.column_config.TextColumn("Status", disabled=True),
            "AI Status": st.column_config.TextColumn("AI makes", disabled=True, width="small"),
            "Reason": st.column_config.TextColumn("AI Reasoning", disabled=True),
            "Raw Confidence": st.column_config.TextColumn("Confidence (from retrieval)", disabled=True),
            "LLM Confidence": st.column_config.TextColumn("Confidence Value (LLM)", disabled=True),
        },
        hide_index=True,
        width="stretch",
        num_rows="fixed"
    )

    # --- Proposal Review Section ---
    edited_proposals = None
    if st.session_state.proposals:
        st.divider()
        st.markdown("#### Extension of VSS (New Signals)")
        st.warning("The AI proposed new signals. Review and **Check** the ones you want to "
                   "officially add to VSS.")

        prop_df = pd.DataFrame(st.session_state.proposals)

        if "Accept" not in prop_df.columns:
            prop_df.insert(0, "Accept", False)

        edited_proposals = st.data_editor(
            prop_df,
            key="proposal_editor",
            column_config={
                "Accept": st.column_config.CheckboxColumn("Approve?", help="Check to include in VSpec Patch"),
                "source": st.column_config.TextColumn("Source Origin", disabled=True),
                "proposed_path": st.column_config.TextColumn("Proposed VSS Path", required=True),
                "description": st.column_config.TextColumn("Description", required=True),
                "type": st.column_config.SelectboxColumn("Type", options=["sensor", "actuator", "attribute"]),
                "unit": st.column_config.TextColumn("Unit")
            },
            hide_index=True,
            width="stretch"
        )

    # --- Export Section ---
    st.divider()
    st.subheader("Final Actions")
    col_a, col_b = st.columns(2)

    # COLUMN A: Extended VSS Standard Download button
    with col_a:
        st.markdown("**1. Update Standard**")
        extended_vss_data = json.loads(json.dumps(st.session_state.vss_data))
        merged_proposals = 0
        if edited_proposals is not None and not edited_proposals.empty:
            accepted_proposals = edited_proposals[edited_proposals["Accept"] == True]   # noqa: E712

            if not accepted_proposals.empty:
                for _, row in accepted_proposals.iterrows():
                    parts = row["proposed_path"].split(".")
                    current = extended_vss_data
                    for i, part in enumerate(parts):
                        is_leaf = (i == len(parts) - 1)
                        if is_leaf:
                            current[part] = {
                                "type": row["type"],
                                "description": row["description"],
                                "unit": row.get("unit", ""),
                                "datatype": "float",
                            }
                        else:
                            if part not in current:
                                current[part] = {
                                    "type": "branch",
                                    "description": "Auto-generated",
                                    "children": {},
                                }
                            if "children" in current[part]:
                                current = current[part]["children"]
                            else:
                                current = current[part]
                merged_proposals = len(accepted_proposals)

        # ---- (B) Add high-confidence matches (retrieval+reranker auto-accept) ----
        # We only take the retrieval "accept" path by requiring:
        #   - AI Status == ✅ High Conf
        #   - VSS Type != Proposal
        #   - Validation == ✅ OK
        #   - LLM Confidence is null (i.e., not judged;
        # judge band never reaches retrieval high-conf anyway)
        added_matches = 0
        try:
            high_conf_rows = edited_df[
                (edited_df["AI Status"] == "✅ High Conf")
                & (edited_df["VSS Type"] != "Proposal")
                & (edited_df["Validation"] == "✅ OK")
                & (edited_df["LLM Confidence"].isna())
            ]
        except Exception:
            high_conf_rows = pd.DataFrame()

        for _, r in high_conf_rows.iterrows():
            vss_path = str(r["VSS Match"])
            src_name = str(r["Source"])
            conf_val = r.get("Raw Confidence", None)

            parts = vss_path.split(".")
            current = extended_vss_data
            node = None

            for i, part in enumerate(parts):
                if part not in current:
                    node = None
                    break

                node = current[part]

                # descend until leaf
                if i < len(parts) - 1:
                    if isinstance(node, dict) and "children" in node and isinstance(node["children"], dict):
                        current = node["children"]
                    else:
                        current = node

            if node is None or not isinstance(node, dict):
                continue

            rosetta_meta = node.setdefault("rosetta", {})
            src_list = rosetta_meta.setdefault("sources", [])

            # avoid duplicates
            if not any(x.get("source") == src_name for x in src_list):
                src_list.append({"source": src_name, "confidence": conf_val})
                added_matches += 1

        # ---- Export ----
        if merged_proposals == 0 and added_matches == 0:
            st.info("Nothing to export yet (no accepted proposals, no high-confidence matches).")
        else:
            st.success(f"Export ready: merged {merged_proposals} proposals;"
                       f"added {added_matches} high-confidence matches."
                       )
            st.download_button(
                "📥 Download Extended Standard (.json)",
                json.dumps(extended_vss_data, indent=2),
                "vss_extended.json",
                "application/json",
                help="Extended VSS + Rosetta high-confidence match annotations"
            )

    # COLUMN B: Mapping Config
    with col_b:
        st.markdown("**2. Generate Mapping**")
        # to-do: automate the connection to other signals
        # Only generate vss_dbc.json when the uploaded source was a DBC file.
        # KUKSA CAN Provider requires a DBC decoder context
        is_dbc = st.session_state.get("last_uploaded_is_dbc", False)
        dbc_filename = st.session_state.get("last_uploaded_filename", "")

        if not is_dbc:
            st.info("KUKSA CAN mapping (vss_dbc.json) is only applicable for .dbc inputs.")
            st.button("🚀 Generate Verified KUKSA Config", disabled=True)
        else:
            if st.button("🚀 Generate Verified KUKSA Config"):
                final_map = dict(zip(edited_df["Source"], edited_df["VSS Match"]))

                # Filtering Logic
                valid_map = {}
                dropped_signals = []

                standard_set = set(s.path for s in kb.signals)
                accepted_set = set()

                if edited_proposals is not None and not edited_proposals.empty:
                    accepted_set = set(edited_proposals
                                       [edited_proposals["Accept"] == True]['proposed_path'])   # noqa: E712

                for source_sig, target_path in final_map.items():
                    if target_path in standard_set or target_path in accepted_set:
                        valid_map[source_sig] = target_path
                    else:
                        dropped_signals.append(f"{source_sig} -> {target_path}")

                # Generate config (DBC-only)
                mapping_list = [{"source": k, "target": v} for k, v in valid_map.items()]
                # Populate dbc_file so CAN provider can decode frames (user must provide file at runtime path).
                json_str = generate_kuksa_config(mapping_list, dbc_file=dbc_filename)

                st.download_button(
                    "📥 Download vss_dbc.json",
                    json_str,
                    "vss_dbc.json",
                    "application/json",
                    help="DBC-only CAN mapping. Includes matched + accepted proposal mappings."
                )

                if dropped_signals:
                    st.warning(f"⚠️ Generated config for {len(valid_map)} signals. {len(dropped_signals)} excluded.")
                    with st.expander("View Excluded Signals"):
                        for s in dropped_signals:
                            st.write(f"❌ {s}")
                else:
                    st.success(f"✅ All {len(valid_map)} signals included.")


def render_debug_tools(kb):
    """Renders the Logs and Knowledge Base Inspector sections."""
    if st.session_state.results or st.session_state.proposals:
        st.divider()
        with st.expander("🕵️ View Process Logs & Intermediate Outputs", expanded=False):
            st.markdown("### Execution Log")
            log_text = "\n".join(st.session_state.logs)
            st.text_area("Step-by-step processing:", value=log_text, height=200)

            st.markdown("### Intermediate Data Structures (JSON)")
            tab_mapped, tab_proposed = st.tabs(["Mapped Data", "Proposal Data"])

            with tab_mapped:
                st.json(st.session_state.results)

            with tab_proposed:
                st.json(st.session_state.proposals)

    st.divider()
    st.subheader("3. Knowledge Base Inspector")
    st.markdown("Transparency Tool: See exactly what text the AI reads to create vectors.")

    with st.expander("🔍 View Internal Search Index"):
        if hasattr(kb, 'signals') and hasattr(kb, 'corpus_text'):
            index_data = []
            for i, sig in enumerate(kb.signals):
                index_data.append({
                    "ID": i,
                    "VSS Path": sig.path,
                    "Raw Description": sig.description,
                    "AI Search String": kb.corpus_text[i]
                })

            st.info(f"The AI has indexed **{len(index_data)}** signals.")

            st.dataframe(
                pd.DataFrame(index_data),
                width="stretch",
                column_config={
                    "AI Search String": st.column_config.TextColumn(
                        "Vector Input Context",
                        help="This combined string is what gets embedded into the Vector DB.",
                        width="large"
                    )
                }
            )
        else:
            st.warning("Knowledge Base not loaded or empty.")
