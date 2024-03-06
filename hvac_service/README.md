# HVAC service example

The HVAC service is a service dummy allowing to control the state of the A/C and the desired cabin temperature.
"Dummy" means, that changes of those two states are just forwarded and reflected as two respective data points in the data broker.

```text
                      +----------------+
                      |                |
         ----( O------|  Data broker   |-----O )------ 
         |    Broker  |                |  Collector  |
         |            +----------------+             |
         |                                           |
         |                                           |
+-----------------+                         +----------------+
|                 |                         |                |
|   HVAC client   |-----------( O-----------|  HVAC service  |
|                 |            HVAC         |                |
+-----------------+           service       +----------------+
```

## Configuration

| parameter      | default value         | Env var                                                                          | description                     |
|----------------|-----------------------|----------------------------------------------------------------------------------|---------------------------------|
| listen_address | `"127.0.0.1:50052"`   | `HVAC_ADDR`                                                                      | Listen for rpc calls            |
| broker_address | `"127.0.0.1:55555"`   | `"127.0.0.1:$DAPR_GRPC_PORT"` (if DAPR_GRPC_PORT is set)<br>`VDB_ADDRESS` (else) | Connect to data broker instance |
| broker_app_id  | `"vehicledatabroker"` | `VEHICLEDATABROKER_DAPR_APP_ID`                                                  | Connect to data broker instance |

Configuration options have the following priority (highest at top):
1. environment variable
1. default value

## Running HVAC service

Databroker must have been started.

```bash
$ pip install -r requirements.txt
$ ./hvacservice.py 
INFO:hvac_service:Connecting to Data Broker [127.0.0.1:55555]
INFO:hvac_service:Starting HVAC Service on 0.0.0.0:50052
INFO:hvac_service:Using gRPC metadata: None
INFO:hvac_service:[127.0.0.1:55555] Connectivity changed to: ChannelConnectivity.IDLE
INFO:hvac_service:Connected to data broker
INFO:hvac_service:Try register datapoints
INFO:hvac_service:datapoints are registered.
...
```

### Devcontainer

A dev-container exist, you still need to do `pip install` before starting hvac_service.
