# KUKSA Persistence Provider

All data in KUKSA is ephemeral. However, in a car there is often data that does not change over the lifetime of a vehicle, and data where you want changes to be persisted over ignition cycles.

This provider can achieve this. It can restore certain values upon startup, either sensor (current) values, or actuations.

An example for one-time restoration of current values are attributes that are maybe not set in a default VSS model deployed to ALL cars of a specific variant, but nevertheless are constant of a specific car, such as the VIN or the vehicle color.

This provider can also watch (subscribe) certain current or actuation values. This is useful when interacting with components that do not provide their own persistence management. Assume a climate control UI that can react on user input and interact with the HVAC system, but is otherwise stateless. By watching and restoring the desired target temperature, the user's preference is saved and restored, without the HVAC UI needing any specific code to handle this.

## Configuration: config.json

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

### section restore-only

These elements will be restored from the state store upon startup, but their values will not be watched and updated for changes. You can define whether the current values (values) will be restored or whether a target value is set (actuators).

### section restore-and-watch

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
  * Check out the repo with the depenable json library at the same level as the parent folder or project folder of this repo.
  * Edit `Cargo.toml` and uncomment the following lines

    ```toml
    djson = { path="../../platform/modules/json-al/djson_rust/" ,  optional = true } # Uncommment to use djson

    # ...

    json_djson = [ "dep:djson","dep:tinyjson"  ]   # Uncommment to use djson
    ```

  * Install rust nightly which is needed for djson at the moment [11/2024]

  * ```bash
    rustup toolchain install nightly
    rustup default nightly
    ```

  * Enable with ```cargo build --features json_djson --no-default-features```

## Build

```bash
cargo build
```

alternatively, with features: see above for prerequisites

```bash
cargo build --features json_djson --no-default-features
```

## Test

1) Check for empty data point after startup

    * In Terminal A): Start kuksa databroker with:

        ```bash
        docker run -it --rm --net=host ghcr.io/eclipse-kuksa/kuksa-databroker:latest --port 55556
        ```

    * In Terminal B) Start kuksa sdk with:

        ```bash
        docker run -it --rm --net=host ghcr.io/eclipse-kuksa/kuksa-python-sdk/kuksa-client:latest grpc://127.0.0.1:55556
        # Check Vales in cmd
        getValue Vehicle.Cabin.HVAC.Station.Row4.Passenger.FanSpeed
        ```

    * In Terminal C) Start kuksa persistency provider with:

        ```bash
        cargo run
        ```

2) Set data point which is stored by persistency provider
    * In Terminal B) :

        ```bash
        # Set Values in cmd
        setValue Vehicle.Cabin.HVAC.Station.Row4.Passenger.FanSpeed 40
        ```

3) Restart and check if datapoint with old value is restored by persistency provider
