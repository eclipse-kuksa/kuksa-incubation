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
Signal parsers for automotive data formats based on Regex.
Rule-based signal extraction from DBC, ARXML and Excel.
"""
from __future__ import annotations
import logging
import tempfile
from pathlib import Path
from typing import List, Dict, Any, Optional
from dataclasses import dataclass

import pandas as pd

try:
    import cantools
    HAS_CANTOOLS = True
except ImportError:
    HAS_CANTOOLS = False

try:
    from lxml import etree
    HAS_LXML = True
except ImportError:
    HAS_LXML = False

logger = logging.getLogger(__name__)


@dataclass
class ParsedSignal:
    """Parsed signal from any input format."""
    name: str
    description: str = ""
    unit: str = ""
    context: str = ""
    min_value: Optional[float] = None
    max_value: Optional[float] = None
    scale: Optional[float] = None
    offset: Optional[float] = None

    def to_dict(self) -> Dict[str, Any]:
        return {
            "name": self.name, "description": self.description,
            "unit": self.unit, "context": self.context,
            "min": self.min_value, "max": self.max_value,
            "scale": self.scale, "offset": self.offset
        }


class ParserError(Exception):
    pass


def auto_parse(filepath: str | Path, llm_generator=None) -> List[ParsedSignal]:
    """Auto-detect format and parse. Optionally use LLM for code files."""
    filepath = Path(filepath)
    ext = filepath.suffix.lower()

    logger.info("Auto-parsing file: %s (type: %s)", filepath.name, ext)

    if ext == ".dbc":
        signals = parse_dbc(filepath)
    elif ext in (".xlsx", ".xls"):
        signals = parse_excel(filepath)
    elif ext == ".csv":
        signals = parse_csv(filepath)
    elif ext == ".arxml":
        signals = parse_arxml(filepath)
    else:
        logger.warning("Unknown file type %s, attempting code parser", ext)
        return "Unknown File Type. Cannot be Parsed."

    logger.info("Extracted %d signals from %s", len(signals), filepath.name)
    return signals


# DBC PARSER - Enhanced with full signal metadata
def parse_dbc(filepath: str | Path) -> List[ParsedSignal]:
    """Parse CAN DBC files with full metadata extraction."""
    if not HAS_CANTOOLS:
        raise ParserError("cantools required: pip install cantools")

    signals = []
    filepath = Path(filepath)

    try:
        logger.debug("Loading DBC: %s", filepath)
        try:
            db = cantools.database.load_file(str(filepath))
        except Exception as e:
            # Some public DBCs contain non-standard/invalid comment strings.
            # For mapping, comments are helpful but not required, so we fall back
            # to a sanitized variant that strips CM_ comment records.
            logger.warning("DBC parse failed (%s). Sanitizing DBC by stripping CM_ lines and retrying...", e)
            raw = filepath.read_text(errors="ignore").splitlines()
            filtered = [ln for ln in raw if not ln.lstrip().startswith("CM_ ")]
            with tempfile.NamedTemporaryFile("w", delete=False, suffix=".dbc",
                                             encoding="utf-8", errors="ignore") as tmp:
                tmp.write("\n".join(filtered) + "\n")
                tmp_path = tmp.name
            try:
                db = cantools.database.load_file(tmp_path)
            finally:
                try:
                    Path(tmp_path).unlink(missing_ok=True)
                except Exception:
                    pass

        for msg in db.messages:
            logger.debug("Processing message: %s (0x%X) with %d signals",
                         msg.name, msg.frame_id, len(msg.signals))

            for sig in msg.signals:
                choices_str = ""
                if hasattr(sig, 'choices') and sig.choices:
                    choices_str = f" Values: {dict(list(sig.choices.items())[:5])}"

                description = sig.comment or ""
                if choices_str:
                    description = f"{description}{choices_str}".strip()

                signals.append(ParsedSignal(
                    name=sig.name,
                    description=description,
                    unit=sig.unit or "",
                    context=f"CAN Message: {msg.name} (0x{msg.frame_id:X})",
                    min_value=getattr(sig, 'minimum', None),
                    max_value=getattr(sig, 'maximum', None),
                    scale=getattr(sig, 'scale', None),
                    offset=getattr(sig, 'offset', None),
                ))

        logger.info("DBC parsed: %d messages, %d signals from %s",
                    len(db.messages), len(signals), filepath.name)

    except Exception as e:
        logger.error("DBC parse failed for %s: %s", filepath, e)
        raise ParserError(f"DBC parse error: {e}")

    return signals


def parse_excel(filepath: str | Path) -> List[ParsedSignal]:
    try:
        df = pd.read_excel(filepath)
        signals = _parse_dataframe(df, f"Excel: {Path(filepath).name}")
        logger.info("Excel parsed: %d signals", len(signals))
        return signals
    except Exception as e:
        logger.error("Excel parse failed: %s", e)
        raise ParserError(f"Excel parse error: {e}")


def parse_csv(filepath: str | Path) -> List[ParsedSignal]:
    try:
        df = pd.read_csv(filepath)
        signals = _parse_dataframe(df, f"CSV: {Path(filepath).name}")
        logger.info("CSV parsed: %d signals", len(signals))
        return signals
    except Exception as e:
        logger.error("CSV parse failed: %s", e)
        raise ParserError(f"CSV parse error: {e}")


def _parse_dataframe(df: pd.DataFrame, context: str) -> List[ParsedSignal]:
    df.columns = [str(c).lower().strip() for c in df.columns]

    name_col = next((c for c in ["signalname", "signal_name", "name",
                    "signal", "id", "parameter"] if c in df.columns), None)
    if not name_col:
        raise ParserError(f"No signal name column found. Available: {list(df.columns)}")

    desc_col = next((c for c in ["description", "desc", "comment"] if c in df.columns), None)
    unit_col = next((c for c in ["unit", "units"] if c in df.columns), None)

    signals = []
    for _, row in df.iterrows():
        name = str(row[name_col]).strip()
        if not name or name.lower() in ("nan", "none", ""):
            continue
        desc = str(row.get(desc_col, "")) if desc_col else ""
        unit = str(row.get(unit_col, "")) if unit_col else ""
        signals.append(ParsedSignal(
            name=name, description="" if desc.lower() in ("nan", "none") else desc,
            unit="" if unit.lower() in ("nan", "none") else unit, context=context
        ))
    return signals


def parse_arxml(filepath: str | Path) -> List[ParsedSignal]:
    if not HAS_LXML:
        raise ParserError("lxml required: pip install lxml")

    signals: List[ParsedSignal] = []
    filepath = Path(filepath)

    try:
        tree = etree.parse(str(filepath))
        root = tree.getroot()

        # Precompile XPaths (avoid reparsing expressions in the inner loop)
        xp_short_name = etree.XPath("./*[local-name()='SHORT-NAME']")
        xp_desc_l2 = etree.XPath(".//*[local-name()='L-2']")
        xp_unit_ref = etree.XPath(".//*[local-name()='UNIT-REF']")

        for elem_type in ['VARIABLE-DATA-PROTOTYPE', 'I-SIGNAL', 'SYSTEM-SIGNAL', 'DATA-PROTOTYPE']:
            for elem in root.xpath(f"//*[local-name()='{elem_type}']"):
                name_nodes = xp_short_name(elem)
                if not name_nodes or not (name_nodes[0].text and name_nodes[0].text.strip()):
                    continue
                name = name_nodes[0].text.strip()

                desc = ""
                desc_nodes = xp_desc_l2(elem)
                if desc_nodes and desc_nodes[0].text:
                    desc = desc_nodes[0].text.strip()

                unit = ""
                unit_nodes = xp_unit_ref(elem)
                if unit_nodes and unit_nodes[0].text:
                    unit = unit_nodes[0].text.split("/")[-1].strip()

                signals.append(
                    ParsedSignal(
                        name=name,
                        description=desc,
                        unit=unit,
                        context=f"AUTOSAR {elem_type}",
                    )
                )

        logger.info("ARXML parsed: %d signals from %s", len(signals), filepath.name)

    except Exception as e:
        logger.error("ARXML parse failed: %s", e)
        raise ParserError(f"ARXML parse error: {e}") from e

    return signals
