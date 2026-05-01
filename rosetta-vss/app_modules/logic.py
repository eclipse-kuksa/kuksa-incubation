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

# app_modules/logic.py
import streamlit as st
import tempfile
import os
import logging
from vss_mapper import QuerySignal, auto_parse
from vss_mapper.extractor import LLMExtractor

logger = logging.getLogger(__name__)


def process_file(uploaded_file, kb, llm, proposal_threshold):
    """
    Parses file and runs the RAG pipeline. Updates Session State directly.
    """
    # Initialize logs
    if 'logs' not in st.session_state:
        st.session_state.logs = []

    # 1) Save file
    ext = os.path.splitext(uploaded_file.name)[1].lower()

    # Persist upload metadata for export gating (DBC-only KUKSA mapping)
    st.session_state.last_uploaded_ext = ext
    st.session_state.last_uploaded_filename = uploaded_file.name
    st.session_state.last_uploaded_is_dbc = (ext == ".dbc")

    with tempfile.NamedTemporaryFile(delete=False, suffix=ext) as tmp:
        tmp.write(uploaded_file.getvalue())
        tpath = tmp.name

    try:
        # 2) Extract signals - Deterministic for structured, LLM for code/YAML
        # only as these can be a bit random
        if ext in (".c", ".cpp", ".h", ".hpp", ".yaml", ".yml"):
            if st.session_state.hf_token:
                st.session_state.logs.append("INFO: Using LLM extraction for code/YAML...")
                logger.info("Using LLM extraction for code/YAML...")
                extractor = LLMExtractor(llm)
                signals = extractor.extract(tpath)
            else:
                st.session_state.logs.append("ERROR: Cannot extract signals from "
                                             "code without LLM...")
                logger.info("ERROR: Cannot extract signals from code without LLM...")
        else:
            st.session_state.logs.append("INFO: Using deterministic parser...")
            logger.info("Using deterministic parser...")
            signals = auto_parse(tpath)

        st.session_state.logs.append(f"INFO: Extracted {len(signals)} signals")
        logger.info("Extracted %d signals", len(signals))
    finally:
        os.unlink(tpath)

    # Reset state
    extraction_logs = st.session_state.get("logs", [])[-5:]
    st.session_state.results = []
    st.session_state.proposals = []
    st.session_state.logs = extraction_logs

    if not signals:
        st.session_state.logs.append("WARNING: No signals extracted.")
        logger.warning("No signals extracted.")
        return

    # 3) Build query objects for batch processing into the embedding model
    queries = [
        QuerySignal(
            name=sig.name,
            description=sig.description,
            unit=sig.unit,
            context=sig.context,
        )
        for sig in signals
    ]

    # 4a) Tokenization diagnostic
    try:
        kb.dump_tokenization_diagnostic(queries, output_path="tokenization_diagnostic")
        st.session_state.logs.append("INFO: Tokenization diagnostic written to "
                                     "tokenization_diagnostic.csv / .md")
    except Exception as e:
        st.session_state.logs.append(f"WARN: Tokenization diagnostic failed: {e}")
        logger.warning("Tokenization diagnostic failed: %s", e)

    # 4b) Batch retrieval from the embedding model
    # define how many matches you need from the embedding model.
    st.session_state.logs.append(f"INFO: Processing {len(signals)} signals...")
    logger.info("Processing %d signals...", len(signals))
    all_matches = kb.search_batch(queries, top_k=10)

    # 5) Decide which items go to LLM judge (mid-confidence band)
    # Low confidence (<= threshold) => proposal flow
    # Very high confidence (>= high_confidence_threshold) => auto-accept (skip LLM)
    llm_judge_set = set()
    if st.session_state.hf_token:
        try:
            llm_idxs = kb.select_llm_indices(
                all_matches,
                proposal_threshold,
            )
            llm_judge_set = set(llm_idxs)
            st.session_state.logs.append(
                f"INFO: LLM-judge routing: {len(llm_judge_set)}/{len(all_matches)} signals"
            )
            logger.info(
                "LLM-judge routing: %d/%d signals", len(llm_judge_set), len(all_matches)
            )
        except Exception as e:
            st.session_state.logs.append(f"WARN: LLM-judge routing failed: {e}")
            logger.warning("LLM-judge routing failed: %s", e)

    # 6) Per-signal processing
    prog = st.progress(0)
    for i, (sig, matches) in enumerate(zip(signals, all_matches)):
        if prog is not None:
            prog.progress((i + 1) / len(signals))
        st.session_state.logs.append(f"INFO: Processing signal '{sig.name}'...")
        logger.debug("Processing signal '%s'...", sig.name)

        # Take best match, based on retrieval model output for judging its
        # quality or suitability
        match_best = matches[0] if matches else None
        judge_vetoed = False
        judge_reason = None
        judge_llm_conf = None
        retrieval_path_for_display = match_best.vss_signal.path if match_best else None

        # Validation Logic: Here just a basic check. Not accurate.
        validation_msg = "✅ OK"
        vss_type = "unknown"
        if match_best:
            vss_unit = match_best.vss_signal.unit or ""
            vss_type = match_best.vss_signal.type or "unknown"
            src_unit = sig.unit or ""
            if src_unit and vss_unit and src_unit.lower() != vss_unit.lower():
                validation_msg = f"⚠️ Unit Mismatch: {src_unit} -> {vss_unit}"

        # ---------------------------
        # Explicit three-way routing:
        #   (A) Auto-accept  : score > high_confidence_threshold
        #   (B) LLM judge    : score < high_confidence_threshold and > proposal_threshold
        #   (C) Proposal     : no match or score <= proposal_threshold (extend VSS)
        # ---------------------------
        route = "proposal"
        if match_best:
            if match_best.score < proposal_threshold:
                route = "proposal"
            elif match_best.score >= kb.config.high_confidence_threshold:
                route = "accept"
            else:
                route = "judge"

        # -------- accept / judge path --------
        if route in ("accept", "judge"):
            final_path = match_best.vss_signal.path
            llm_proposal = ""
            reason = "Vector Search Confidence"
            llm_conf = None
            status_flag = "✅ Match"

            # Auto status from retrieval confidence if we're not judging
            if route == "accept":
                lvl = kb.get_confidence_level(match_best.score)
                status_flag = {
                    "high": "✅ High Conf",
                    "medium": "⚠️ Med Conf",
                    "low": "📉 Low Conf",
                    "very_low": "📉 Low Conf",
                }.get(lvl, "✅ Match")

            # LLM judge (mid-confidence only)
            if route == "judge" and st.session_state.hf_token:
                sig_dict = sig.to_dict()
                matches_dict = [
                    {"vss_signal": m.vss_signal.to_dict(), "score": m.score} for m in matches
                ]
                res = llm.generate_justification(sig_dict, matches_dict)

                if res and ("best_vss_path" in res):
                    llm_path = res.get("best_vss_path")
                    llm_conf = res.get("confidence", None)
                    reason = res.get("justification", reason)
                    match_type = res.get("match_type", "")

                    # Constrain to candidate set - reject hallucinated paths
                    cand_paths = {m.vss_signal.path for m in matches}
                    if llm_path and llm_path not in cand_paths:
                        st.session_state.logs.append(
                            f"⚠️ LLM hallucinated '{llm_path}' not in candidates. Veto."
                        )
                        logger.warning("LLM hallucinated path '%s' not in candidates", llm_path)
                        llm_path = None  # Treat as veto

                    # Judge can veto: treat as "no match" -> proposal flow
                    if (llm_path is None) or (match_type == "none") or (
                        isinstance(llm_conf, (int, float)) and llm_conf < 0.30
                    ):
                        judge_vetoed = True
                        judge_reason = reason
                        judge_llm_conf = llm_conf
                        route = "proposal"
                    else:
                        # Status purely from judge confidence (more interpretable)
                        if isinstance(llm_conf, (int, float)):
                            if llm_conf >= 0.7:
                                status_flag = "✅ High Conf"
                            elif llm_conf >= 0.5:
                                status_flag = "⚠️ Med Conf"
                            else:
                                status_flag = "📉 Low Conf"

                        # If judge disagrees, APPLY the judge decision (this is the system output),
                        # and show retrieval result for transparency.
                        if llm_path != final_path:
                            status_flag = "⚠️ LLM Override"
                            retrieval_path = final_path  # original vector suggestion
                            final_path = llm_path       # judge-chosen output
                            llm_proposal = f"Retrieval: {retrieval_path}"
                            st.session_state.logs.append(
                                f"🔁 LLM override: Retrieval '{retrieval_path}' -> LLM '{final_path}'"
                            )
                            logger.warning(
                                "LLM override: Retrieval '%s' -> LLM '%s'",
                                retrieval_path, final_path
                            )

            # If judge vetoed, fall through to proposal block below.
            if route != "proposal":
                st.session_state.results.append(
                    {
                        "Source": sig.name,
                        "Source Unit": sig.unit or "-",
                        "VSS Match": final_path,
                        "LLM Suggestion": llm_proposal,
                        "VSS Type": vss_type,
                        "Validation": validation_msg,
                        "AI Status": status_flag,
                        "Reason": reason,
                        "Raw Confidence": round(match_best.score, 3),
                        "LLM Confidence": round(llm_conf, 3)
                        if isinstance(llm_conf, (int, float))
                        else None,
                    }
                )
                continue  # next signal

        # -------- proposal path --------
        st.session_state.logs.append("   📉 Low/No Match. Generating Proposal...")
        logger.info("Low/No Match. Generating Proposal...")

        res = None
        if st.session_state.hf_token:
            # Better conditioning than only top-level keys: include top candidates too.
            candidate_paths = [m.vss_signal.path for m in (matches or [])[:8]]
            top_level = list(st.session_state.vss_data.keys())[:200]
            ctx = (
                "Top candidates:\n"
                + "\n".join(candidate_paths)
                + "\n\nTop-level VSS keys:\n"
                + str(top_level)
            )
            res = llm.generate_proposal(sig.to_dict(), ctx)

        if not res or "error" in res:
            res = {
                "proposed_path": "MANUAL_REVIEW",
                "type": "unknown",
                "unit": sig.unit or "",
                "description": "AI Offline",
            }

        res["source"] = sig.name
        st.session_state.proposals.append(res)

        status = "🚫 LLM Veto" if judge_vetoed else "📉 Proposal"
        reason_text = judge_reason if judge_vetoed and judge_reason else res.get("description", "")
        suggestion = f"Retrieval: {retrieval_path_for_display}" if judge_vetoed and retrieval_path_for_display else ""

        st.session_state.results.append(
            {
                "Source": sig.name,
                "Source Unit": sig.unit or "-",
                "VSS Match": res.get("proposed_path", "Unknown"),
                "LLM Suggestion": suggestion,
                "VSS Type": "Proposal",
                "Validation": "⚠️ New Signal",
                "AI Status": status,
                "Reason": reason_text,
                "Raw Confidence": round(match_best.score, 3) if match_best else 0.0,
                "LLM Confidence": round(judge_llm_conf, 3) if isinstance(judge_llm_conf, (int, float)) else None,
            }
        )

    prog.empty()
    st.session_state.logs.append(
        f"INFO: Complete. {len(st.session_state.results)} mapped, {len(st.session_state.proposals)} proposals."
    )
    logger.info(
        "Complete. %d mapped, %d proposals.",
        len(st.session_state.results),
        len(st.session_state.proposals),
    )
