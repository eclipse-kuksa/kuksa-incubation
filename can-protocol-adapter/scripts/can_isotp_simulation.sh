#!/bin/bash

# Set colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'

# Configuration 
SRC_ADDR="7e8"
DST_ADDR="7df"
CAN_INTERFACE="vcan0"

# Command to run
command_to_run="stdbuf -oL isotprecv -s ${SRC_ADDR} -d ${DST_ADDR} -l ${CAN_INTERFACE}"

# Message counters
declare -A message_sent
for service in "0D" "0C" "04" "05" "0F" "0A" "23"; do
  message_sent[$service]=0
done

# Messages for each service 
declare -A messages
messages[0D]="41 0D 12|41 0D 22|41 0D 32|41 0D 18"
messages[0C]="41 0C 0A 2C|41 0C 21 EA|41 0C 2B 02|41 0C 0E 0A"
messages[05]="41 05 22 23|41 05 28 32|41 05 35 37|41 05 25 28"

# Function to send ISO-TP messages 
send_isotp_message() {
  local service=$1
  if [[ -z "${messages[$service]+_}" ]]; then  
    echo "Error: No messages defined for service $service" >&2
    return 1
  fi
  IFS='|' read -r -a service_messages <<< "${messages[$service]}"  
  local index=$((message_sent[service] % ${#service_messages[@]}))
  local message=${service_messages[index]}

  echo "$message" | isotpsend -s "${SRC_ADDR}" -d "${DST_ADDR}" "${CAN_INTERFACE}"
  echo -e "$GREEN$timestamp >>>   $message\033[0m"

  message_sent[service]=$((index + 1))
}

# Main loop 
$command_to_run | while IFS= read -r line; do
  timestamp=$(date +"%Y-%m-%d %H:%M:%S.%3N")
  echo -e "$BLUE$timestamp <<<   $line"

  for service in "0D" "0C" "04" "05" "0F" "0A" "23"; do
    if echo "$line" | grep -q "01 ${service}"; then
      send_isotp_message "${service}"
      break
    fi
  done
done


