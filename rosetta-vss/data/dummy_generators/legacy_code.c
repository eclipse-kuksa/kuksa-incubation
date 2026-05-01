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
