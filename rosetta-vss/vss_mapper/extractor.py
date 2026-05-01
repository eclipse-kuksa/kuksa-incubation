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
LLM-based Signal Extractor - Unified extraction across C/C++, YAML formats.
Any other format would be just taken as text.
"""
from __future__ import annotations
import json
import logging
from pathlib import Path
from typing import List, Dict, Any, Optional
from dataclasses import dataclass, asdict

logger = logging.getLogger(__name__)


@dataclass
class ExtractedSignal:
    """Signal extracted by LLM."""
    name: str
    description: str = ""
    unit: str = ""
    datatype: str = ""
    context: str = ""
    min_value: Optional[float] = None
    max_value: Optional[float] = None

    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)


class LLMExtractor:
    """
    Unified LLM-based signal extractor.
    Currently optimal for code and yaml files (i.e. ROS-based),
    but ideally should work for other formats also.
    We use only for code and YAML as the other formats follow stricter rules and can be deterministically extracted.
    """

    SYSTEM_PROMPT = """You are an automotive signal extraction expert.
    Your task is to identify ALL vehicle/ECU signals from the provided data.

A signal is any variable that represents:
- Sensor readings (temperature, pressure, speed, voltage, etc.)
- Actuator commands (motor control, valve positions, etc.)
- Vehicle state (door status, gear position, ignition state, etc.)
- Diagnostic values (fault codes, counters, flags, etc.)

Rules:
1. Extract EVERY signal, even if abbreviated or encoded
2. Infer description from context, comments, or naming convention
3. Infer unit from naming (e.g., _kmh, _degC, _pct) or context
4. Infer datatype (float, int, bool, enum)
5. DO NOT hallucinate signals that don't exist in the data
6. Return ONLY valid JSON array"""

    OUTPUT_SCHEMA = """Return a JSON array of signals:
[
  {
    "name": "signal_name",
    "description": "what this signal represents",
    "unit": "physical unit or empty",
    "datatype": "float|int|bool|enum|string",
    "context": "where found (message name, struct, file section)"
  }
]"""

    def __init__(self, llm_generator):
        self.llm = llm_generator
        # temperature=0 for reproducibility
        if hasattr(self.llm, 'config'):
            self.llm.config.temperature = 0.0

    def extract(self, filepath: str | Path) -> List[ExtractedSignal]:
        """Extract signals from any supported file format."""
        filepath = Path(filepath)
        ext = filepath.suffix.lower()

        logger.info("LLM extracting from: %s (type: %s)", filepath.name, ext)

        # Read and format content based on file type
        if ext in (".yaml", ".yml"):
            content = self._read_yaml(filepath)
        elif ext in (".c", ".cpp", ".h", ".hpp"):
            content = self._read_code(filepath)
        else:
            content = self._read_text(filepath)

        # Extract via LLM
        signals = self._extract_with_llm(content, filepath.name)
        logger.info("LLM extracted %d signals from %s", len(signals), filepath.name)

        return signals

    def _extract_with_llm(self, content: str, source_name: str) -> List[ExtractedSignal]:
        """Send content to LLM and parse response."""
        # Truncate if too long (keep beginning and end for context)
        max_chars = 12000
        if len(content) > max_chars:
            half = max_chars // 2
            content = content[:half] + "\n\n[... truncated ...]\n\n" + content[-half:]

        prompt = f"""{self.SYSTEM_PROMPT}

{self.OUTPUT_SCHEMA}

Source: {source_name}

Data:
```
{content}
```

Extract ALL signals as JSON array:"""

        try:
            # Use direct call for more control
            response = self._call_llm(prompt)
            if not response:
                logger.warning("LLM returned empty response")
                return []

            # Parse JSON from response
            signals_data = self._parse_json_response(response)

            # Convert to ExtractedSignal objects
            signals = []
            for item in signals_data:
                if isinstance(item, dict) and item.get("name"):
                    signals.append(ExtractedSignal(
                        name=item["name"],
                        description=item.get("description", ""),
                        unit=item.get("unit", ""),
                        datatype=item.get("datatype", ""),
                        context=item.get("context", source_name),
                        min_value=item.get("min_value"),
                        max_value=item.get("max_value"),
                    ))

            return signals

        except Exception as e:
            logger.error("LLM extraction failed: %s", e)
            return []

    def _call_llm(self, prompt: str) -> Optional[str]:
        try:
            if hasattr(self.llm, 'client') and self.llm.client:
                if hasattr(self.llm, 'model_id'):
                    response = self.llm.client.chat_completion(
                        model=self.llm.model_id,
                        messages=[{"role": "user", "content": prompt}],
                        max_tokens=4000,
                        temperature=0.0,
                    )
                    return response.choices[0].message.content

            # Fallback to _call_model
            return self.llm._call_model(prompt)

        except Exception as e:
            logger.error("LLM call failed: %s", e)
            return None

    def _parse_json_response(self, response: str) -> List[Dict]:
        """Extract JSON array from LLM response."""
        # Try direct parse
        try:
            return json.loads(response)
        except json.JSONDecodeError:
            pass

        # Extract from markdown code block
        if "```json" in response:
            start = response.find("```json") + 7
            end = response.find("```", start)
            if end > start:
                try:
                    return json.loads(response[start:end].strip())
                except json.JSONDecodeError:
                    pass

        # Extract from generic code block
        if "```" in response:
            start = response.find("```") + 3
            # Skip language identifier if present
            newline = response.find("\n", start)
            if newline > start:
                start = newline + 1
            end = response.find("```", start)
            if end > start:
                try:
                    return json.loads(response[start:end].strip())
                except json.JSONDecodeError:
                    pass

        # Try to find array in response
        start = response.find("[")
        end = response.rfind("]") + 1
        if start >= 0 and end > start:
            try:
                return json.loads(response[start:end])
            except json.JSONDecodeError:
                pass

        logger.warning("Failed to parse JSON from LLM response")
        return []

    def _read_yaml(self, filepath: Path) -> str:
        """Read YAML and format for LLM."""
        try:
            with open(filepath, "r") as f:
                content = f.read()

            lines = [
                f"YAML/ROS Config: {filepath.name}",
                "",
                content[:10000]  # First 10k chars
            ]
            return "\n".join(lines)

        except Exception as e:
            logger.warning("YAML read error: %s", e)
            return ""

    def _read_code(self, filepath: Path) -> str:
        """Read C/C++ code as-is for LLM."""
        return self._read_text(filepath)

    def _read_text(self, filepath: Path) -> str:
        """Read any file as text."""
        try:
            with open(filepath, "r", encoding="utf-8", errors="ignore") as f:
                return f.read()
        except Exception as e:
            logger.error("File read error: %s", e)
            return ""
