# CAN Protocol Adapter Configuration

This configuration file is used to manage the settings for a CAN  interface with socket-can and ISO-TP (ISO-15765 Transport Protocol) support. The file is structured to allow configuration of both general settings and specific CAN-related settings, as well as the configuration of various parameter requests (PIDs) to fetch vehicle or CAN data. 

This setup can be used for any CAN network supporting ISO-TP communication, not limited to OBD-II systems. Below are the details of the configuration file sections and their usage.

### General Configuration (general_config)

This section defines basic settings required for communication with the CAN interface and the message broker:

    "general_config": { 
      "broker_ip": "localhost", 
      "broker_port": "55555", 
      "dbcfile": "/dbc/obd2_isotp.dbc" 
    }
    
### broker_ip:
 Specifies the IP address of the kuksa databroker. 


### broker_port:
 Specifies the port number for kuksa databroker. 


### dbcfile: 
Path to the DBC (CAN database) file, which contains signal definitions for the CAN network. This file defines how the CAN data should be interpreted, including signal names, units, and scaling.

### CAN Configuration (can_config)
This section configures the CAN interface for communication using socket-can:

    "can_config": {
       "can_interface": "can0", 
       "use_extended_id": false,
       "tx_id": "0x7DF",
       "rx_id": "0x7E8",
       "socket_can_type": "SOCK_DGRAM", 
      "socket_can_protocol": "CAN_ISOTP"
     }
     
### can_interface: 
Specifies the CAN interface to be used.

### use_extended_id:
A boolean setting that indicates whether extended CAN IDs should be used. The default is false, meaning standard CAN IDs will be used.

### tx_id:
The CAN ID used for transmitting isotp messages. 

### rx_id:
The CAN ID used for receiving messages. 

### socket_can_type:
Specifies the socket type for CAN communication, set to "SOCK_DGRAM", which is typical for datagram-based communication.

### socket_can_protocol:
Defines the protocol for CAN communication. The default is "CAN_ISOTP", which refers to the ISO-15765 standard for transport protocols, used to handle large messages across the CAN bus.

## PID Table (pid_table)
This section defines the list of parameter requests (PIDs) that the system will use to fetch specific data over the CAN bus. The system supports generic CAN data requests, not just OBD-II PIDs.

    "pid_table": [
    { 
     "request_pid": "01 0D",
     "response_pid": "41 0D",
     "response_timeout_ms": 100,
     "description": "Vehicle Speed", 
     "expected_response_length": 4,
     "interval_ms": 500,
     "dbc_signal_name": "S01PID0D_VehicleSpeed",
     "vss_signal": { 
     "signal_name": "Vehicle.CAN.Speed", 
     "datatype": "float", 
     "unit": "km/h" 
    } 
      },
      {
     //other PID definitions
      }
    ]

### request_pid: 
 The PID used to request data.
 
### response_pid:
 The PID returned in the response. 
  
### response_timeout_ms:
 Timeout in milliseconds for waiting for a response. 
 
### description:
 A brief description of what data the PID request and responses.
  
### expected_response_length:
 The expected number of bytes in the response. This helps validate the response format.
  
### interval_ms:
 The interval in milliseconds between repeated PID requests. 
  
### dbc_signal_name: 
 The name of the signal as defined in the DBC file. This helps map the raw CAN message data to a VSS signal datapoint.
  
### vss_signal:
 Defines the VSS signal datapoint details should be as per the VSS data used by kuksa databroker.
  
### signal_name:
 The name of the signal used in the system.
  
### datatype:
 The data type for the signal.
  
### unit:
 The unit of the signal.













