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

"""Domain-specific rules for automotive signal mapping.

This module contains:
1. Expanded abbreviation dictionary for CAN/automotive signals
2. Domain rules for common patterns (wheel positions, brake types, etc.)
3. Signal category detection for better matching

These rules address specific failure patterns observed in evaluation:
- Wheel_FL maps to Row2 instead of Row1 (position disambiguation)
- BrakePressed maps to brake lights instead of pedal
- KickDownPressed maps to seat switches
- Volume_UP maps to Navigation.Volume instead of Infotainment.Volume
"""

from __future__ import annotations
from typing import Dict, List, Optional, Tuple
import re


# =============================================================================
# Expanded Abbreviation Dictionary
# =============================================================================

ABBREVIATIONS: Dict[str, List[str]] = {
    # Position abbreviations (very common in CAN)
    "fl": ["front", "left", "row1"],
    "fr": ["front", "right", "row1"],
    "rl": ["rear", "left", "row2"],
    "rr": ["rear", "right", "row2"],
    "f": ["front"],
    "r": ["rear"],
    "l": ["left"],
    # German position abbreviations (common in European OEMs)
    "vl": ["front", "left", "vorn", "links"],
    "vr": ["front", "right", "vorn", "rechts"],
    "hl": ["rear", "left", "hinten", "links"],
    "hr": ["rear", "right", "hinten", "rechts"],
    "v": ["front", "vorn"],
    "h": ["rear", "hinten"],

    # Common signal type abbreviations
    "spd": ["speed"],
    "vel": ["velocity", "speed"],
    "pos": ["position"],
    "ang": ["angle"],
    "tmp": ["temperature"],
    "temp": ["temperature"],
    "prs": ["pressure"],
    "press": ["pressure"],
    "trq": ["torque"],
    "torq": ["torque"],
    "accel": ["acceleration", "accelerator"],
    "acc": ["acceleration", "accelerator"],
    "rpm": ["revolutions", "engine", "speed"],
    "pct": ["percent", "percentage"],
    "pwr": ["power"],
    "curr": ["current"],
    "volt": ["voltage"],
    "lvl": ["level"],
    "sts": ["status"],
    "stat": ["status", "state"],
    "cnt": ["count", "counter"],
    "flg": ["flag"],
    "sw": ["switch"],
    "btn": ["button"],
    "cmd": ["command"],
    "req": ["request"],
    "tar": ["target"],
    "act": ["actual", "active"],
    "avl": ["available"],
    "max": ["maximum"],
    "min": ["minimum"],

    # Vehicle system abbreviations
    "veh": ["vehicle"],
    "eng": ["engine"],
    "mot": ["motor"],
    "trn": ["transmission"],
    "trans": ["transmission"],
    "brk": ["brake"],
    "whl": ["wheel"],
    "tir": ["tire", "tyre"],
    "str": ["steering", "steer"],
    "steer": ["steering"],
    "susp": ["suspension"],
    "chass": ["chassis"],
    "bdy": ["body"],
    "cab": ["cabin"],
    "drvr": ["driver"],
    "psgr": ["passenger"],
    "soc": ["state", "of", "charge", "battery"],
    "batt": ["battery"],
    "bat": ["battery"],
    "ign": ["ignition"],
    "ecu": ["electronic", "control", "unit"],
    "abs": ["antilock", "braking"],
    "esc": ["stability", "control"],
    "dsc": ["stability", "control"],  # BMW term
    "tcs": ["traction", "control"],
    "dtc": ["diagnostic", "trouble", "code"],
    "mil": ["malfunction", "indicator", "lamp"],
    "obd": ["onboard", "diagnostics"],
    "hvac": ["heating", "ventilation", "air", "conditioning"],
    "ac": ["air", "conditioning"],
    "adas": ["advanced", "driver", "assistance"],
    "lka": ["lane", "keep", "assist"],
    "lda": ["lane", "departure"],
    "fcw": ["forward", "collision", "warning"],
    "aeb": ["automatic", "emergency", "braking"],
    "pcw": ["pedestrian", "collision", "warning"],
    "bsd": ["blind", "spot", "detection"],
    "rcta": ["rear", "cross", "traffic", "alert"],

    # Specific signal patterns
    "kickdown": ["kickdown", "transmission", "downshift"],
    "autopilot": ["autonomous", "driving"],
    "cruise": ["cruise", "control"],
    "wipers": ["wiper", "wiping"],
    "wiper": ["wiper", "wiping", "windshield"],
    "turn": ["turn", "signal", "indicator", "blinker"],
    "blink": ["blinker", "indicator", "turn"],
    "horn": ["horn"],
    "hazard": ["hazard", "warning"],
    "headlight": ["headlight", "beam", "high", "low"],
    "taillight": ["taillight", "rear", "light"],
    "fog": ["fog", "light"],
    "mirror": ["mirror"],
    "window": ["window"],
    "door": ["door"],
    "lock": ["lock"],
    "seat": ["seat"],
    "belt": ["seatbelt", "belt"],
    "airbag": ["airbag"],
    "odometer": ["odometer", "distance", "traveled"],
    "fuel": ["fuel"],
    "oil": ["oil"],
    "coolant": ["coolant", "engine", "temperature"],
    "ambient": ["ambient", "outside", "exterior"],
}


# =============================================================================
# Domain Rules
# =============================================================================

# Patterns that indicate specific VSS domains
DOMAIN_PATTERNS: Dict[str, List[Tuple[str, float]]] = {
    # Wheel speed signals -> specific axle/wheel
    "wheel_speed": [
        (r"(?:wheel|whl|rad).*(?:speed|spd|vel|rpm)", 1.0),
        (r"(?:fl|fr|rl|rr|vl|vr|hl|hr).*(?:speed|spd|vel)", 0.9),
    ],
    # Brake pedal vs brake lights vs parking brake
    "brake_pedal": [
        (r"(?:brake|brk).*(?:pedal|press|push|active|actuating)", 1.0),
        (r"(?:brake|brk).*(?:pressure|prs)", 0.8),
        (r"(?:driver).*(?:brake|brk)", 0.9),
    ],
    "brake_light": [
        (r"(?:brake|brk).*(?:light|lamp|led)", 1.0),
        (r"(?:stop).*(?:light|lamp)", 0.9),
    ],
    "parking_brake": [
        (r"(?:park|hand).*(?:brake|brk)", 1.0),
        (r"(?:epb|hpb)", 0.9),  # Electronic/Hand Parking Brake
    ],
    # Steering
    "steering_angle": [
        (r"(?:steer|str).*(?:angle|ang|position|pos)", 1.0),
        (r"(?:swa|sas)", 0.9),  # Steering Wheel Angle, Steering Angle Sensor
    ],
    "steering_torque": [
        (r"(?:steer|str).*(?:torque|trq)", 1.0),
        (r"(?:eps).*(?:torque|trq)", 0.9),  # Electric Power Steering
    ],
    # Transmission
    "transmission_kickdown": [
        (r"(?:kick.*down)", 1.0),
        (r"(?:downshift).*(?:force|request)", 0.8),
    ],
    "transmission_gear": [
        (r"(?:gear).*(?:tar|target|sel|selected|current|act)", 1.0),
        (r"(?:prnd|prndl)", 0.9),
    ],
    # Cruise control
    "cruise_active": [
        (r"(?:cruise).*(?:active|on|enable)", 1.0),
        (r"(?:acc|adaptive).*(?:active|on)", 0.9),
    ],
    "cruise_speed": [
        (r"(?:cruise).*(?:speed|set|target)", 1.0),
    ],
    # Lights/indicators
    "turn_signal": [
        (r"(?:turn|blink|indicator).*(?:left|right|signal)", 1.0),
        (r"(?:left|right).*(?:turn|blink|indicator)", 0.9),
    ],
    # Volume (infotainment, not cargo)
    "audio_volume": [
        (r"(?:volume|vol).*(?:up|down|level|set)", 1.0),
        (r"(?:audio|media|sound).*(?:volume|vol)", 0.9),
    ],
    # Wipers
    "front_wiper": [
        (r"(?:front|frt|windshield).*(?:wiper|wiping)", 1.0),
        (r"(?:wiper|wiping).*(?:front|frt|auto)", 0.8),
    ],
    "rear_wiper": [
        (r"(?:rear|rr|back).*(?:wiper|wiping)", 1.0),
    ],
    # Engine
    "engine_speed": [
        (r"(?:engine|eng|motor).*(?:speed|rpm|revs)", 1.0),
        (r"(?:rpm|revs).*(?:engine|eng)", 0.9),
    ],
    "engine_torque": [
        (r"(?:engine|eng|motor).*(?:torque|trq)", 1.0),
        (r"(?:torque|trq).*(?:engine|eng)", 0.9),
    ],
    "engine_temp": [
        (r"(?:engine|eng|coolant|ect).*(?:temp|temperature)", 1.0),
        (r"(?:water|coolant).*(?:temp)", 0.9),
    ],
}


# VSS path patterns for domain matching
VSS_DOMAIN_PATHS: Dict[str, List[str]] = {
    "wheel_speed": [
        "Vehicle.Chassis.Axle.Row1.Wheel.Left.Speed",
        "Vehicle.Chassis.Axle.Row1.Wheel.Right.Speed",
        "Vehicle.Chassis.Axle.Row2.Wheel.Left.Speed",
        "Vehicle.Chassis.Axle.Row2.Wheel.Right.Speed",
    ],
    "brake_pedal": [
        "Vehicle.Chassis.Brake.IsDriverActuating",
        "Vehicle.Chassis.Brake.PedalPosition",
        "Vehicle.Chassis.Brake.IsDriverEmergencyBrakingDetected",
    ],
    "brake_light": [
        "Vehicle.Body.Lights.Brake.IsActive",
    ],
    "parking_brake": [
        "Vehicle.Chassis.ParkingBrake.IsEngaged",
        "Vehicle.Chassis.ParkingBrake.IsAutoApplyEnabled",
    ],
    "steering_angle": [
        "Vehicle.Chassis.SteeringWheel.Angle",
        "Vehicle.Chassis.Axle.Row1.SteeringAngle",
    ],
    "steering_torque": [
        "Vehicle.Chassis.SteeringWheel.Torque",
    ],
    "transmission_kickdown": [
        "Vehicle.Powertrain.Transmission.KickDown",
    ],
    "transmission_gear": [
        "Vehicle.Powertrain.Transmission.CurrentGear",
        "Vehicle.Powertrain.Transmission.SelectedGear",
        "Vehicle.Powertrain.Transmission.GearChangeMode",
    ],
    "cruise_active": [
        "Vehicle.ADAS.CruiseControl.IsActive",
        "Vehicle.ADAS.CruiseControl.IsEnabled",
    ],
    "cruise_speed": [
        "Vehicle.ADAS.CruiseControl.SpeedSet",
        "Vehicle.ADAS.CruiseControl.TargetSpeed",
    ],
    "turn_signal": [
        "Vehicle.Body.Lights.DirectionIndicator.Left.IsSignaling",
        "Vehicle.Body.Lights.DirectionIndicator.Right.IsSignaling",
    ],
    "audio_volume": [
        "Vehicle.Cabin.Infotainment.Volume",
        "Vehicle.Cabin.Infotainment.Media.Volume",
    ],
    "front_wiper": [
        "Vehicle.Body.Windshield.Front.Wiping.Mode",
        "Vehicle.Body.Windshield.Front.Wiping.System.IsWiping",
    ],
    "rear_wiper": [
        "Vehicle.Body.Windshield.Rear.Wiping.Mode",
        "Vehicle.Body.Windshield.Rear.Wiping.System.IsWiping",
    ],
    "engine_speed": [
        "Vehicle.Powertrain.CombustionEngine.Speed",
        "Vehicle.OBD.EngineSpeed",
    ],
    "engine_torque": [
        "Vehicle.Powertrain.CombustionEngine.Torque",
        "Vehicle.Powertrain.CombustionEngine.MaxTorque",
    ],
    "engine_temp": [
        "Vehicle.Powertrain.CombustionEngine.ECT",
        "Vehicle.Powertrain.CombustionEngine.EOT",
    ],
}


def expand_abbreviations(tokens: List[str]) -> List[str]:
    """Expand abbreviations in token list."""
    expanded = []
    for t in tokens:
        t_lower = t.lower()
        expanded.append(t_lower)
        if t_lower in ABBREVIATIONS:
            expanded.extend(ABBREVIATIONS[t_lower])
    return expanded


def detect_signal_domain(signal_name: str, description: str = "") -> Optional[str]:
    """Detect the domain/category of a signal based on name and description."""
    text = f"{signal_name} {description}".lower()

    best_domain = None
    best_score = 0.0

    for domain, patterns in DOMAIN_PATTERNS.items():
        for pattern, weight in patterns:
            if re.search(pattern, text, re.IGNORECASE):
                if weight > best_score:
                    best_score = weight
                    best_domain = domain

    return best_domain


def get_preferred_vss_paths(domain: str) -> List[str]:
    """Get preferred VSS paths for a domain."""
    return VSS_DOMAIN_PATHS.get(domain, [])


def compute_domain_bonus(
    signal_name: str,
    signal_description: str,
    vss_path: str,
    base_bonus: float = 0.10,
) -> float:
    """Compute bonus for domain-compatible VSS paths."""
    domain = detect_signal_domain(signal_name, signal_description)
    if not domain:
        return 0.0

    preferred = get_preferred_vss_paths(domain)
    if not preferred:
        return 0.0

    # Exact match
    if vss_path in preferred:
        return base_bonus

    # Partial match (same branch)
    for p in preferred:
        # Find common prefix
        parts_p = p.split(".")
        parts_v = vss_path.split(".")
        common = 0
        for a, b in zip(parts_p, parts_v):
            if a == b:
                common += 1
            else:
                break
        # If at least 3 levels match (e.g., Vehicle.Chassis.Brake)
        if common >= 3:
            return base_bonus * 0.5

    return 0.0


def extract_position_from_name(name: str) -> Dict[str, Optional[str]]:
    """Extract position information from signal name.

    Returns dict with keys: axle (row1/row2), side (left/right)
    """
    name_lower = name.lower()

    # Direct patterns
    position = {"axle": None, "side": None}

    # Front/rear -> Row1/Row2
    if any(p in name_lower for p in ["_fl", "_fr", "front", "_vl", "_vr", "_f_"]):
        position["axle"] = "row1"
    elif any(p in name_lower for p in ["_rl", "_rr", "rear", "_hl", "_hr", "_r_"]):
        position["axle"] = "row2"

    # Left/right
    if any(p in name_lower for p in ["_fl", "_rl", "_vl", "_hl", "left", "_l_", "_l"]):
        position["side"] = "left"
    elif any(p in name_lower for p in ["_fr", "_rr", "_vr", "_hr", "right", "_r_"]):
        position["side"] = "right"

    return position


def position_matches_vss_path(signal_name: str, vss_path: str) -> Tuple[bool, float]:
    """Check if signal position matches VSS path position.

    Returns (matches, penalty) where penalty is 0 if match, negative if mismatch.
    """
    signal_pos = extract_position_from_name(signal_name)
    vss_lower = vss_path.lower()

    penalty = 0.0

    # Check axle (row)
    if signal_pos["axle"]:
        if signal_pos["axle"] == "row1" and "row2" in vss_lower:
            penalty -= 0.15
        elif signal_pos["axle"] == "row2" and "row1" in vss_lower:
            penalty -= 0.15

    # Check side
    if signal_pos["side"]:
        if signal_pos["side"] == "left" and "right" in vss_lower and "left" not in vss_lower:
            penalty -= 0.15
        elif signal_pos["side"] == "right" and "left" in vss_lower and "right" not in vss_lower:
            penalty -= 0.15

    return (penalty == 0.0, penalty)
