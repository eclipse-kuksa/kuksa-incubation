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

import pandas as pd

# 1. Create a Sample Excel File (The "Messy" Supplier Spec)
df = pd.DataFrame([
    {"SignalName": "Whl_Rot_Vel_FL", "Description": "Front Left Wheel Rotation Velocity", "Unit": "rad/s"},
    {"SignalName": "Whl_Rot_Vel_FR", "Description": "Front Right Wheel Rotation Velocity", "Unit": "rad/s"},
    {"SignalName": "Batt_Stack_Temp_Avg", "Description": "Average temperature of the HV battery stack", "Unit": "C"},
    {"SignalName": "Amb_Air_T", "Description": "External ambient temperature sensor", "Unit": "C"},
    {"SignalName": "Dr_Sts_FL", "Description": "Driver door open/close status (1=Open)", "Unit": "bool"},
    {"SignalName": "Lidar_Obj_List_Cnt", "Description": "Count of objects tracked by front Lidar",
        "Unit": "cnt"}  # This should trigger a PROPOSAL
])
df.to_excel("supplier_spec.xlsx", index=False)
print("Created 'supplier_spec.xlsx'")

# 2. Create a Sample C File (The "Legacy" Code)
c_code = """
#include "vehicle_bus.h"

// Global Signal Definitions
float Eng_Coolant_T_In; // Engine coolant inlet temp
float Eng_Coolant_T_Out; // Engine coolant outlet temp
float Veh_Spd_Kmh;       // Vehicle speed derived from ABS
bool  Ign_Sts_Run;       // Ignition status

void read_sensors() {
    // Legacy sensor reading
    float t_val = get_signal("Batt_Lvl_Pct"); // HV Battery State of Charge
}
"""
with open("legacy_code.c", "w") as f:
    f.write(c_code)
print("Created 'legacy_code.c'")

# 3. Create a Sample DBC File (The "Network" Definition)
# This is a minimal valid DBC format
dbc_content = """VERSION ""

NS_ :
  NS_DESC_
  CM_
  BA_DEF_
  BA_

BS_:

BU_: ECU ABS

BO_ 100 EngineData: 8 ECU
 SG_ Eng_RPM : 0|16@1+ (1,0) [0|8000] "rpm" Vector__XXX
 SG_ Trq_Req : 16|16@1+ (1,0) [0|1000] "Nm" Vector__XXX

BO_ 200 ABS_Data: 8 ABS
 SG_ Whl_Spd_Avg : 0|16@1+ (0.01,0) [0|250] "km/h" Vector__XXX
"""
with open("network.dbc", "w") as f:
    f.write(dbc_content)
print("Created 'network.dbc'")

content = """<?xml version="1.0" encoding="UTF-8"?>
<AUTOSAR xmlns="http://autosar.org/schema/r4.0">
  <AR-PACKAGES>
    <AR-PACKAGE>
      <SHORT-NAME>VehicleInterfaces</SHORT-NAME>
      <ELEMENTS>
        <!-- Interface 1: Powertrain Info -->
        <SENDER-RECEIVER-INTERFACE>
          <SHORT-NAME>PowertrainData_I</SHORT-NAME>
          <DATA-ELEMENTS>
            <VARIABLE-DATA-PROTOTYPE>
              <SHORT-NAME>EngineSpeed_rpm</SHORT-NAME>
              <DESC>
                  <L-2 L="EN">Current rotation speed of the internal combustion engine.</L-2>
              </DESC>
            </VARIABLE-DATA-PROTOTYPE>
            <VARIABLE-DATA-PROTOTYPE>
              <SHORT-NAME>TransGear_Current</SHORT-NAME>
            </VARIABLE-DATA-PROTOTYPE>
          </DATA-ELEMENTS>
        </SENDER-RECEIVER-INTERFACE>

        <!-- Interface 2: Body Control -->
        <SENDER-RECEIVER-INTERFACE>
          <SHORT-NAME>DoorStatus_I</SHORT-NAME>
          <DATA-ELEMENTS>
            <VARIABLE-DATA-PROTOTYPE>
              <SHORT-NAME>DrvrDoor_Open</SHORT-NAME>
              <DESC>
                  <L-2 L="EN">Boolean status of driver door.</L-2>
              </DESC>
            </VARIABLE-DATA-PROTOTYPE>
          </DATA-ELEMENTS>
        </SENDER-RECEIVER-INTERFACE>
      </ELEMENTS>
    </AR-PACKAGE>
  </AR-PACKAGES>
</AUTOSAR>
"""

with open("test_autosar.arxml", "w") as f:
    f.write(content)

print("Created 'test_autosar.arxml'")
