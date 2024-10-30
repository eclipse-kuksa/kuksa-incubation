# KUKSA Persistence Provider

All data in KUKSA is ephemereal. However, in a car there is often data that does not change over the lifetime of a vehicle, and data where you want changes to be persisted over ignition cycles.

This provider can achieve this. It can restore certain values upon startup, either sensor (current) values, or actuations.

An example for one-time restoration of current values are attributes that are maybe not set in a default VSS model deployed to ALL cars of a specific variant, but nevertheless are constant of a specific car, such as the VIN or the Vehicle Color.

This provider can also watch (subscribe) certain current or actuation values. This is useful when interacting with components that do not provide their own persistence management. Assume a climate control UI that can react on unser input and interact with the HVAC system, but is otherwise stateless. By watching and restoring the desired target temperature, the user's preference is saved and restored, without the HVAC UI needing any specific code to handle this.

## Configuration: config,json

Main configuration is in config.json, and example may look like this

```json
{
    "restore-only": {
        "values": [
            "Vehicle.VehicleIdentification.VIN",
            "Vehicle.VehicleIdentification.VehicleInteriorColor"
        ],
        "actuators": [
            "Vehicle.Cabin.Infotainment.HMI.TemperatureUnit"
        ]
    },

    "restore-and-watch": {
        "values": [
            "Vehicle.Cabin.Infotainment.HMI.TemperatureUnit",
            "Vehicle.Cabin.HVAC.Station.Row4.Passenger.FanSpeed"
        ],
        "actuators": [
            "Vehicle.Cabin.Infotainment.Media.Volume"
        ]
    },
    "state-storage": {
        "type": "file",
        "path": "statestore.json"
    }
}
```

## restore-only section

These elements will be restored from the state store upon startup, but their values will not be watched and updated for changes. You can define whether the current values (values) will be restored or whether a target value is set (actuators).

## restore-and-watch

These elements will be restored from the state store upon startup. It is the intention to also monitor their state and update it in the state store. You can define whether the current values (values) will be restored and watched or whether a target value is set (actuators) and watched. As restore-and-watch includes restore, there is no need to add paths in restore-and-watch to restore-only as well.

## state-storage

Configures the state sotrage used to retrieve values. Currently supported: file

## File storage: statestore.json

This is a valid state store for the file storage.
*Note: ALl VALUES NEED TO BE STORED AS STRING*.
As the statestore does not make a difference between current and target (actuation) values it is currently not possible to watch or restore both for a single VSS path.

```json

{
    "Vehicle.VehicleIdentification.VIN": {
        "value": "DEADBEEF"
    },
    "Vehicle.VehicleIdentification.VehicleInteriorColor": {
        "value": "Black"
    }
}
```

## Features

* djson: Use a dependable json library. 
  Enable with ```cargo build --features json_djson --no-default-features```

## Build

```bash
cargo build --features json_djson --no-default-features
```

## Test

```bash
docker run -it --rm --net=host ghcr.io/eclipse-kuksa/kuksa-databroker:latest --port 55556
```

```bash
docker run -it --rm --net=host ghcr.io/eclipse-kuksa/kuksa-python-sdk/kuksa-client:latest grpc://127.0.0.1:55556
```

# TODO 
* config mit djson parsen