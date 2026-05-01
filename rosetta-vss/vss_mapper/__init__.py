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

"""Rosetta VSS - Automotive Signal to VSS Mapper.
"""

from .core import (
    VSSKnowledgeBase,
    VSSSignal,
    QuerySignal,
    SearchResult,
    HybridConfig,
)

from .parsers import (
    auto_parse,
    parse_dbc,
    parse_excel,
    parse_csv,
    parse_arxml,
    ParsedSignal,
    ParserError,
)

from .llm import (
    HuggingFaceGenerator,
    OfflineGenerator,
    create_generator,
    LLMConfig,
)

from .exporters import generate_kuksa_config

from .extractor import LLMExtractor, ExtractedSignal

from .domain_rules import (
    ABBREVIATIONS,
    expand_abbreviations,
    detect_signal_domain,
    compute_domain_bonus,
    position_matches_vss_path,
)

__version__ = "1.2.0"

__all__ = [
    "VSSKnowledgeBase",
    "VSSSignal",
    "QuerySignal",
    "SearchResult",
    "HybridConfig",
    "auto_parse",
    "parse_dbc",
    "parse_excel",
    "parse_csv",
    "parse_arxml",
    "ParsedSignal",
    "ParserError",
    "HuggingFaceGenerator",
    "OfflineGenerator",
    "create_generator",
    "LLMConfig",
    "generate_kuksa_config",
    # Extractor
    "LLMExtractor",
    "ExtractedSignal",
    # Domain rules
    "ABBREVIATIONS",
    "expand_abbreviations",
    "detect_signal_domain",
    "compute_domain_bonus",
    "position_matches_vss_path",
]
