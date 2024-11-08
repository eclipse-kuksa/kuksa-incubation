# Zenoh Kuksa Provider

The aim of this project is to provide a bridge component between an [Eclipse Zenoh](https://github.com/eclipse-zenoh/zenoh)
network and an [Eclipse Kuksa Databroker](https://github.com/eclipse-kuksa/kuksa-databroker).

It can be utilized to subscribe to a list of actuator
[VSS signals](https://covesa.github.io/vehicle_signal_specification/rule_set/data_entry/sensor_actuator/index.html)
and propagate their state from the Kuksa Databroker to the Zenoh network on the respective key expressions and back.

The provider currently supports peer discovery in the network via UDP multicast,
which can be useful for connecting to a nearby
[zenoh router](https://github.com/eclipse-zenoh/zenoh/tree/main/zenohd).
If peer discovery is not suitable for your use case, you can also create a
direct configured connection.

The basic operation is as follows: \
The provider connects to a Kuksa Databroker and subscribes to changes on the configured
target values of the configured signals. When there is a change in state, the provider will publish the
change to the zenoh network. When a device's target value changes and it publishes its current value
to the Zenoh network, the provider will detect this change and update the corresponding
current state in the Databroker.

> Note: Please note that no datapoint updates for array types can be processed at this time. See
> [kuksa_utils.rs](src/utils/kuksa_utils.rs) for reference.

## Configuring the provider

The main entry point for configuration is the [DEFAULT_CONFIG.json5](DEFAULT_CONFIG.json5)
template configuration in the root of the project.
To adjust the configuration, simply copy and rename the commented example:

```bash
cp DEFAULT_CONFIG.json5 provider-config.json5
```

Here you can set the following things:

- URL of your Kuksa Databroker
- List of VSS paths in the Databroker to subscribe to
- [Key expression](https://zenoh.io/docs/manual/abstractions/) for the provider to subscribe to in the Zenoh network
(e.g. Vehicle/Body/Horn/IsActive)
- Zenoh (client) configuration, including
  - setting the connection _mode_ (`client`, `peer`)
  - enabling or disabling peer discovery (scouting)
  - endpoints to connect to (when using _client_ mode), e.g. a Zenoh router

## Build and run the application

Use cargo to compile the project:

```bash
cargo build --release
```

The compiled binary is built as `target/release/zenoh-kuksa-provider`.

In order to run the provider you need to pass the provider-config.json5 file as an argument like this:

```bash
./target/release/zenoh-kuksa-provider -c provider-config.json5
```

To enable logging use

```bash
RUST_LOG=DEBUG ./target/release/zenoh-kuksa-provider -c provider-config.json5
```

## Docker image

Use

```bash
docker build -t zenoh-kuksa-provider.
```

to build the image and tag it as zenoh-kuksa-provider.

### Usage

The following command will run the image and mount your provider-config.json5 file into the container,
setting the PROVIDER_CONFIG environment variable:

```bash
docker run \
  -e "PROVIDER_CONFIG=/provider-config.json5" \
  -v "$(pwd)/provider-config.json5:/provider-config.json5" \
  zenoh-kuksa-provider
```

> 🚧 Remember to follow the steps from [Configuring the provider](#configuring-the-provider) in order to
> have a properly composed `provider-config.json5` file.
