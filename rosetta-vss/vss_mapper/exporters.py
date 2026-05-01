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

"""
Export generators for VSS mapping artifacts.
- KUKSA CAN Provider JSON config
- VSS overlay YAML (create or UPDATE existing)
- CSV/JSON reports
"""
from __future__ import annotations
import json
import logging
from typing import Dict, List, Any

logger = logging.getLogger(__name__)

# -------------- KUKSA CONFIG GENERATION ------------------


def generate_kuksa_config(
    mappings: List[Dict[str, Any]],
    dbc_file: str = "",
    canport: str = "vcan0",
    dbc_default_file: str = ""
) -> str:
    """
    Generate KUKSA CAN Provider JSON config.

    Format compatible with:
    https://github.com/eclipse-kuksa/kuksa-can-provider
    """
    config = {
        "general": {
            "dbc_file": dbc_default_file or dbc_file,
            "canport": canport,
            "mapping": []
        }
    }

    for m in mappings:
        source = m.get("source") or m.get("Source") or m.get("name", "")
        target = m.get("target") or m.get("VSS Match") or m.get("vss_path", "")

        if not source or not target:
            continue

        mapping_entry = {
            "vss_path": target,
            "can_signal": source,
        }

        # Add transform if provided
        if m.get("scale") or m.get("offset"):
            mapping_entry["transform"] = {
                "scale": m.get("scale", 1.0),
                "offset": m.get("offset", 0.0)
            }

        # Add interval if provided
        if m.get("interval_ms"):
            mapping_entry["interval_ms"] = m["interval_ms"]

        config["general"]["mapping"].append(mapping_entry)

    logger.info("Generated KUKSA config with %d mappings", len(config["general"]["mapping"]))
    return json.dumps(config, indent=2)
