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

from __future__ import annotations
import json
import logging
import time
from dataclasses import dataclass
from typing import Optional, Dict, List
from abc import ABC, abstractmethod

logger = logging.getLogger(__name__)


@dataclass
class LLMConfig:
    max_tokens: int = 1024
    temperature: float = 0
    max_retries: int = 3
    retry_delay: float = 1.0


class BaseLLMGenerator(ABC):
    """Abstract LLM generator."""

    def __init__(self, config: Optional[LLMConfig] = None):
        self.config = config or LLMConfig()

    @abstractmethod
    def _call_model(self, prompt: str) -> Optional[str]:
        pass

    def _clean_json(self, text: str) -> Optional[Dict]:
        if not text:
            return None

        import re

        # Strip reasoning/thinking tokens
        text = re.sub(r'<think>.*?</think>', '', text, flags=re.DOTALL)

        # Extract from markdown
        if "```json" in text:
            text = text.split("```json")[1].split("```")[0]
        elif "```" in text:
            parts = text.split("```")
            if len(parts) >= 2:
                text = parts[1]

        # Try direct parse
        try:
            return json.loads(text.strip())
        except json.JSONDecodeError:
            pass

        # Regex fallback: extract first JSON object
        match = re.search(r'\{[^{}]*(?:\{[^{}]*\}[^{}]*)*\}', text, re.DOTALL)
        if match:
            try:
                return json.loads(match.group())
            except json.JSONDecodeError as e:
                logger.warning("JSON regex fallback failed: %s", e)

        logger.warning("JSON parse failed entirely for response: %s", text[:200])
        return None

    def _call_with_retry(self, prompt: str) -> Optional[Dict]:
        for attempt in range(self.config.max_retries):
            try:
                response = self._call_model(prompt)
                if response:
                    result = self._clean_json(response)
                    if result:
                        return result
            except Exception as e:
                logger.warning("LLM call %d failed: %s", attempt + 1, e)

            if attempt < self.config.max_retries - 1:
                time.sleep(self.config.retry_delay * (attempt + 1))
        return None

    def generate_justification(
        self,
        signal: Dict,
        vss_matches: List[Dict]
    ) -> Optional[Dict]:
        """
        LLM-as-Judge: Validate and select best VSS match.
        """
        prompt = f"""You are an automotive signal mapping expert validating VSS (Vehicle Signal Specification) mappings.

TASK: Evaluate if the source signal correctly maps to one of the VSS candidates.

SOURCE SIGNAL:
- Name: {signal.get('name', '')}
- Unit: {signal.get('unit', 'unknown')}
- Description: {signal.get('description', '')}

VSS CANDIDATES (ranked by retrieval score):
{json.dumps(vss_matches[:5], indent=2)}

EVALUATION CRITERIA:
1. SEMANTIC MATCH: Does the signal measure the same physical quantity?
   - "WHEEL_SPEED_FL" --> "Vehicle.Chassis.Axle.Row1.Wheel.Left.Speed" --> (front-left wheel speed)
   - "ENGINE_RPM" --> "Vehicle.Speed" --> (engine speed --> vehicle speed)

2. UNIT COMPATIBILITY: Are units convertible or compatible?
   - km/h --> m/s  (convertible)
   - degC --> K (convertible)
   - rpm --> km/h  (incompatible)

3. CONTEXT MATCH: Does the VSS path context match the signal's domain?
   - Battery signals --> Vehicle.Powertrain.TractionBattery.*
   - Engine signals --> Vehicle.Powertrain.CombustionEngine.*
   - Cabin signals --> Vehicle.Cabin.*

4. NAMING CONVENTIONS: Common CAN abbreviations:
   - FL/FR/RL/RR = Front-Left/Front-Right/Rear-Left/Rear-Right
   - SOC = State of Charge, ECT = Engine Coolant Temp
   - RPM = Revolutions per Minute, SWA = Steering Wheel Angle

CONFIDENCE SCORING:
- 0.9-1.0: Exact semantic match, correct domain, compatible units
- 0.7-0.89: Strong match with minor ambiguity (e.g., multiple valid paths)
- 0.5-0.69: Partial match, related but not exact (e.g., parent/child path)
- 0.3-0.49: Weak match, same domain but different measurement
- 0.0-0.29: No valid match exists in candidates

Return ONLY valid JSON:
{{
  "best_vss_path": "selected VSS path or null if no match",
  "confidence": 0.0-1.0,
  "justification": "1-2 sentence explanation of match quality",
  "match_type": "exact|partial|domain_only|none"
}}"""

        return self._call_with_retry(prompt)

    def generate_proposal(
        self,
        signal: Dict,
        vss_context: str
    ) -> Optional[Dict]:
        """
        Propose new VSS path for unmapped signal.
        """
        prompt = f"""You are a VSS (Vehicle Signal Specification) architect.
        A source signal has no suitable match in the existing VSS tree.

TASK: Propose a new VSS-compliant path following COVESA conventions.

SOURCE SIGNAL:
- Name: {signal.get('name', '')}
- Unit: {signal.get('unit', '')}
- Description: {signal.get('description', '')}

EXISTING VSS BRANCHES (for reference):
{vss_context[:20000]}

VSS NAMING CONVENTIONS:
1. STRUCTURE: Vehicle.<Domain>.<Subdomain>.<Property>
   - Domains: Powertrain, Chassis, Body, Cabin, ADAS, Exterior, OBD, CurrentLocation

2. COMMON PATTERNS:
   - Wheel positions: Vehicle.Chassis.Axle.Row[1|2].Wheel.[Left|Right].*
   - Seats: Vehicle.Cabin.Seat.Row[1|2|3].[DriverSide|PassengerSide|Left|Right].*
   - Doors: Vehicle.Body.Door.Row[1|2].[Left|Right].*
   - HVAC zones: Vehicle.Cabin.HVAC.[Front|Rear|Row*].*

3. SIGNAL TYPES:
   - sensor: Measured values (speed, temperature, pressure)
   - actuator: Controllable values (target temperature, light commands)
   - attribute: Static properties (VIN, model year, capacity)

4. NAMING RULES:
   - PascalCase for all path segments
   - No abbreviations (use "Temperature" not "Temp")
   - Boolean properties: Is* prefix (IsActive, IsOpen, IsEnabled)
   - State properties: Use enums or Status suffix

EXAMPLE MAPPINGS:
- OEM_TURBO_BOOST_PSI --> Vehicle.Powertrain.CombustionEngine.TurbochargerBoostPressure
- SEAT_HEAT_LVL_RL --> Vehicle.Cabin.Seat.Row2.Left.Heating
- CUSTOM_AMBIENT_LIGHT_R --> Vehicle.Cabin.Light.AmbientLight.Red

Return ONLY valid JSON:
{{
  "proposed_path": "Vehicle.Domain.Subdomain.Property",
  "type": "sensor|actuator|attribute",
  "datatype": "float|int|boolean|string|uint8|int16|etc",
  "unit": "SI unit or empty",
  "description": "Clear description of what this signal represents",
  "rationale": "Why this path structure was chosen"
}}"""

        return self._call_with_retry(prompt)

    def generate_direct_mapping(
        self,
        signal: Dict,
        vss_paths_context: str,
    ) -> Optional[Dict]:
        """
        LLM-only baseline: Map signal directly to VSS without retrieval.
        The LLM receives the full VSS catalog and must pick the correct path.
        """
        prompt = f"""You are an automotive signal mapping expert.

TASK: Propose a new or exisitng VSS-compliant path following COVESA conventions for the source signal.

SOURCE SIGNAL:
- Name: {signal.get('name', '')}
- Unit: {signal.get('unit', 'unknown')}
- Description: {signal.get('description', '')}

ALL VSS PATHS (catalog):
{vss_paths_context[:20000]}

VSS NAMING CONVENTIONS:
1. STRUCTURE: Vehicle.<Domain>.<Subdomain>.<Property>
   - Domains: Powertrain, Chassis, Body, Cabin, ADAS, Exterior, OBD, CurrentLocation

2. COMMON PATTERNS:
   - Wheel positions: Vehicle.Chassis.Axle.Row[1|2].Wheel.[Left|Right].*
   - Seats: Vehicle.Cabin.Seat.Row[1|2|3].[DriverSide|PassengerSide|Left|Right].*
   - Doors: Vehicle.Body.Door.Row[1|2].[Left|Right].*
   - HVAC zones: Vehicle.Cabin.HVAC.[Front|Rear|Row*].*

3. SIGNAL TYPES:
   - sensor: Measured values (speed, temperature, pressure)
   - actuator: Controllable values (target temperature, light commands)
   - attribute: Static properties (VIN, model year, capacity)

4. NAMING RULES:
   - PascalCase for all path segments
   - No abbreviations (use "Temperature" not "Temp")
   - Boolean properties: Is* prefix (IsActive, IsOpen, IsEnabled)
   - State properties: Use enums or Status suffix

EXAMPLE MAPPINGS:
- OEM_TURBO_BOOST_PSI --> Vehicle.Powertrain.CombustionEngine.TurbochargerBoostPressure
- SEAT_HEAT_LVL_RL --> Vehicle.Cabin.Seat.Row2.Left.Heating
- CUSTOM_AMBIENT_LIGHT_R --> Vehicle.Cabin.Light.AmbientLight.Red

Return ONLY valid JSON:
{{
  "best_vss_path": "exact VSS path from catalog or null",
  "confidence": 0.0-1.0,
  "justification": "1-2 sentence explanation"
}}"""

        return self._call_with_retry(prompt)


class HuggingFaceGenerator(BaseLLMGenerator):
    """HuggingFace Inference API."""

    def __init__(
        self,
        api_key: str,
        model_id: str = "Qwen/Qwen2.5-7B-Instruct",
        config: Optional[LLMConfig] = None
    ):
        super().__init__(config)
        self.api_key = api_key
        self.model_id = model_id
        self._client = None

    @property
    def client(self):
        if self._client is None and self.api_key:
            from huggingface_hub import InferenceClient
            self._client = InferenceClient(token=self.api_key)
        return self._client

    def _call_model(self, prompt: str) -> Optional[str]:
        if not self.api_key:
            return None
        try:
            response = self.client.chat_completion(
                model=self.model_id,
                messages=[{"role": "user", "content": prompt}],
                max_tokens=self.config.max_tokens,
                temperature=self.config.temperature
            )
            return response.choices[0].message.content
        except Exception as e:
            logger.error("HF API error: %s", e)
            raise


class OfflineGenerator(BaseLLMGenerator):
    """Fallback when no API available."""

    def _call_model(self, prompt: str) -> Optional[str]:
        return None

    def generate_justification(self, signal: Dict, vss_matches: List[Dict]) -> Optional[Dict]:
        if vss_matches:
            return {
                "best_vss_path": vss_matches[0].get("vss_signal", {}).get("path", ""),
                "confidence": vss_matches[0].get("score", 0.5),
                "justification": "Offline: using top retrieval match"
            }
        return None

    def generate_proposal(self, signal: Dict, vss_context: str) -> Optional[Dict]:
        return {
            "proposed_path": "MANUAL_REVIEW",
            "type": "unknown",
            "unit": signal.get("unit", ""),
            "description": f"Offline: review needed for {signal.get('name', '')}"
        }

    def generate_direct_mapping(self, signal: Dict, vss_paths_context: str) -> Optional[Dict]:
        return None


def create_generator(
    hf_token: str = None,
    model_id: str = "openai/gpt-oss-20b",
    config: Optional[LLMConfig] = None
) -> BaseLLMGenerator:
    """Factory for LLM generator."""
    if hf_token:
        return HuggingFaceGenerator(hf_token, model_id=model_id, config=config)
    else:
        return OfflineGenerator(config=config)
