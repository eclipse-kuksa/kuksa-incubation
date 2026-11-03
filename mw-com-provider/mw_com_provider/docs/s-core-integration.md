# Getting Started: S-Core Integration

This guide explains how to connect an S-Core `mw::com` application to Kuksa Databroker using `mw_com_provider`. It is written for users who are new to this integration and need to understand what to build, what to configure, and how the pieces work together.

The vehicle dynamics application used in this repository is a reference integration. Your own S-Core application can use the same pattern with its own services, instances, events, payload types, and VSS mappings. In an S-Core communication checkout, example applications live under `score/mw/com/example/`. You can modify the contents of these based on your configuration.

## What You Are Integrating

There are three logical parts:

```text
S-Core application
    <-> S-Core LoLa / mw::com runtime
    <-> mw_com_provider
    <-> Kuksa Databroker
    <-> Kuksa gRPC client or application
```

The roles are:

| Component | Role |
| --- | --- |
| S-Core application | Publishes or consumes `mw::com` events. This can be the vehicle dynamics example or your own application. |
| `mw_com_provider` | Databroker provider that maps between `mw::com` service members and VSS entries. |
| `databroker-mw-com` | Composition binary that starts Databroker and `mw_com_provider` in one process. |
| Kuksa client/application | Reads, subscribes, or actuates VSS entries through Kuksa gRPC. The CLI is only one possible client. |

Starting `databroker-mw-com` is straightforward because it is only the composition process: it creates Databroker, starts the provider, loads the generated provider config, connects to the S-Core LoLa runtime, and opens the Kuksa gRPC server.

User can replace the CLI with their own Kuksa gRPC application. For example, instead of running `databroker-cli get or actuate`, an application can call Kuksa `val.v2` `Actuate` or subscribe/read using the standard Kuksa APIs.

## Required Checkout Layout

Create one integration root directory, referred to below as `<integration-root>`, and clone the required repositories directly beneath it:

```text
<integration-root>/
├── kuksa-databroker/                     # This repository, including mw_com_provider
├── communication/                         # S-Core communication repository
├── baselibs/                              # S-Core Baselibs repository
└── vsps_sdk_score_config_codegen-main/    # Optional: required only to regenerate artifacts
```

The `score_com_cargo` manifests intentionally resolve S-Core Rust sources from `<integration-root>/communication` and `<integration-root>/baselibs`. Do not move those repositories below `kuksa-databroker` or rename their checkout directories. The code-generator checkout is only needed for artifact regeneration; set `MW_COM_CODEGEN_ROOT` or pass `--codegen-root` when it is stored elsewhere.

## Integration Model

For every signal or actuator you want to expose through Databroker, define a mapping:

```text
VSS path
    <-> mw::com service
    <-> instance specifier
    <-> event/member name
    <-> payload field and data type
```

Examples from the reference vehicle dynamics setup:

| VSS path | Direction | S-Core service/member |
| --- | --- | --- |
| `Vehicle.Speed` | Datapoint | `VehicleDynamicsService.speed` |
| `Vehicle.ADAS.CruiseControl.IsEnabled` | Datapoint | `VehicleDynamicsService.cruise_control_enabled` |
| `Vehicle.ADAS.CruiseControl.TargetSpeed` | Actuator | `VehicleDynamicsService.target_speed` |

Directions mean:

- `datapoint`: S-Core publishes a sample, provider updates Databroker.
- `actuator`: Kuksa client actuates a VSS entry, provider publishes to S-Core.
- `bidirectional`: both directions are enabled for the same entry.

## What Must Match Between S-Core And The Provider

The S-Core application and provider must use compatible generated artifacts and runtime manifests. These values must match:

- service name
- service ID
- event/member names and event IDs
- instance specifiers
- payload struct names, field names, and data types
- application IDs
- allowed consumer/provider application lists
- LoLa queue/slot capacity

For the reference vehicle dynamics integration:

```text
S-Core application id: 4001
provider application id: 4002
Service1 instance: /Vehicle/Service1/Instance
Service2 instance: /Vehicle/Service2/Instance
receive events: speed, cruise_control_enabled
actuator event: target_speed
```

If service discovery succeeds but samples do not flow, first check application IDs, instance specifiers, and allowed consumer/provider lists in the LoLa manifests.

## Files Kept In This Repository For Now

Until the required generated artifacts and scripts are available directly from the S-Core repository, this provider keeps the integration artifacts locally:

- `mw_com_provider/models/vehicle_dynamics.json`
- `mw_com_provider/templates/reference-codegen/`
- `mw_com_provider/generated/mw_com_provider_config.json`
- `mw_com_provider/generated/vehicle_dynamics_lola_config.json`
- `mw_com_provider/generated/mw_com_provider_lola_adapter.rs`
- `mw_com_provider/generated/mw_com_provider_interfaces.rs`
- `mw_com_provider/generated/mw_com_provider_instances.rs`
- `mw_com_provider/generated/mw_com_provider_metadata.rs`
- `mw_com_provider/generated/mw_com_provider_types.rs`
- `mw_com_provider/score_com_cargo/`

When S-Core owns these generated bindings, manifests, and Rust crate definitions upstream, replace the local generated files and shim crates with the upstream artifacts.

## Step 1: Build Or Prepare The S-Core Application

Start from an S-Core application that uses `mw::com` events. The application should either:

- publish events that Databroker should expose as VSS datapoints, or
- consume events that Databroker should expose as VSS actuators, or
- do both.

In the reference setup, this repository uses the S-Core vehicle dynamics example. It is not special to the provider; it is simply a concrete application that publishes speed and cruise-control state and consumes target-speed actuation.

Reference command:

```sh
cd <integration-root>/communication
bazel build //score/mw/com/example/vehicle-dynamics-example:vehicle-dynamics-example
```

For your own S-Core application, build the Bazel target that produces your application binary and matching LoLa/native artifacts.

## Step 2: Generate Or Provide Provider Artifacts

The provider needs generated Rust types, interfaces, metadata, LoLa adapter code, and provider config matching the S-Core model.

Default regeneration for the checked-in reference model:

```sh
cd <integration-root>/kuksa-databroker/mw_com_provider
bazel run //:generate_mw_com_provider_artifacts
bazel run //:verify_mw_com_provider_codegen
```

If your S-Core communication checkout is local and not available through the configured Bazel registry, pass it as a local module override instead of editing `MODULE.bazel`:

```sh
bazel --override_module=score_communication=../../communication \
  run //:generate_mw_com_provider_artifacts
```

Generate from a custom model:

```sh
cd <integration-root>/kuksa-databroker
cargo run --manifest-path mw_com_provider/xtask/Cargo.toml -- codegen \
  --model /path/to/your-s-core-model.json \
  --templates mw_com_provider/templates/reference-codegen \
  --out mw_com_provider/generated \
  --prefix mw_com
```

Use a custom SCORE config generator checkout:

```sh
cargo run --manifest-path mw_com_provider/xtask/Cargo.toml -- codegen \
  --model /path/to/your-s-core-model.json \
  --templates mw_com_provider/templates/reference-codegen \
  --out mw_com_provider/generated \
  --prefix mw_com \
  --codegen-root /path/to/vsps_sdk_score_config_codegen-main
```

After generation, review `mw_com_provider/generated/mw_com_provider_config.json`. It is the runtime mapping between VSS paths and S-Core service members.

## Step 3: Build The Provider And Composition Binary

From the Kuksa Databroker workspace root:

```sh
cd <integration-root>/kuksa-databroker
cargo test -p mw_com_provider provider::tests
cargo test -p mw_com_provider score_bindings::tests
cargo test -p mw_com_provider --test e2e_stubbed_score
cargo check -p mw_com_provider
```

Build the S-Core target first, then make its exact Bazel output explicit before compiling the LoLa-enabled packages:

```sh
cd <integration-root>/communication
export SCORE_COM_BAZEL_BIN="$(bazel info bazel-bin)"
export SCORE_COM_LINK_PARAMS="$(find "$SCORE_COM_BAZEL_BIN" -name 'vehicle-dynamics-example-0.params' | head -n 1)"
echo "$SCORE_COM_BAZEL_BIN"
echo "$SCORE_COM_LINK_PARAMS"
```
```sh
cd <integration-root>/kuksa-databroker
SCORE_COM_BAZEL_BIN=<value-from-above> \
SCORE_COM_LINK_PARAMS=<value-from-above> \
cargo check -p mw_com_provider --features score-lola
cargo check -p databroker-mw-com
```

`SCORE_COM_LINK_PARAMS` points to the S-Core target link params file. `SCORE_COM_BAZEL_BIN` points to the Bazel output directory reported by `bazel info bazel-bin` from the S-Core communication checkout.

## Step 4: Run The S-Core Application

For the reference vehicle dynamics application:

```sh
cd <integration-root>/communication
rm -rf /tmp/mw_com_lola
bazel run //score/mw/com/example/vehicle-dynamics-example:vehicle-dynamics-example -- --run-mode both
```
After the s-core build is done and application starts running. stop it and fetch the neccesary information from the below commands and then re-run the application with above command

```sh
cd <integration-root>/communication
export SCORE_COM_BAZEL_BIN="$(bazel info bazel-bin)"
export SCORE_COM_LINK_PARAMS="$(find "$SCORE_COM_BAZEL_BIN" -name 'vehicle-dynamics-example-0.params' | head -n 1)"
echo "$SCORE_COM_BAZEL_BIN"
echo "$SCORE_COM_LINK_PARAMS"
```

For your own S-Core application, run your application binary with the LoLa config that matches the generated provider artifacts. Make sure the S-Core application and the provider use different application IDs and are allowed to communicate with each other.

## Step 5: Start Databroker With The Provider

In a second terminal, start the composition binary:

```sh
cd <integration-root>/kuksa-databroker
SCORE_COM_BAZEL_BIN=<value-from-above> \
SCORE_COM_LINK_PARAMS=<value-from-above> \
cargo run -p databroker-mw-com
```

This starts:

- Kuksa Databroker
- the in-process `mw_com_provider`
- Kuksa `val.v1` and `val.v2` gRPC APIs on `127.0.0.1:55555`

Default inputs:

```text
provider config: mw_com_provider/generated/mw_com_provider_config.json
LoLa config: mw_com_provider/generated/vehicle_dynamics_lola_config.json
address: 127.0.0.1:55555
```

You can override these when needed:

```sh
cargo run -p databroker-mw-com -- \
  --address 127.0.0.1:55555 \
  --provider-config mw_com_provider/generated/mw_com_provider_config.json \
  --score-config mw_com_provider/generated/vehicle_dynamics_lola_config.json
```

## Step 6: Connect A Kuksa Client Or Application

For a quick manual check, use the existing Kuksa CLI:

```sh
cargo run -p databroker-cli -- actuate Vehicle.ADAS.CruiseControl.TargetSpeed 88.0
```

Expected reference S-Core output:

```text
1 target speed actuation samples received. target_speed=88
```

The CLI is only a demo client. Product applications can use Kuksa gRPC directly:

- use `val.v2` `Actuate` or `BatchActuate` for actuator commands
- use read/subscribe APIs for datapoints
- keep the same VSS paths defined in the provider config

The provider handles both paths:

- `val.v2` actuation reaches the Databroker actuation provider callback directly
- `val.v1` actuator target writes are bridged by the provider into the same S-Core actuation path

## Queue And Rate Configuration

The reference vehicle dynamics LoLa skeleton has small event slot limits. The provider therefore defaults to:

```text
SCORE_SUBSCRIPTION_QUEUE_CAPACITY=3
SCORE_RECEIVE_BATCH_SIZE=min(16, SCORE_SUBSCRIPTION_QUEUE_CAPACITY)
```

For a production S-Core manifest with larger event slots, raise both values together:

```sh
SCORE_SUBSCRIPTION_QUEUE_CAPACITY=16 \
SCORE_RECEIVE_BATCH_SIZE=16 \
cargo run -p databroker-mw-com
```

`SCORE_SUBSCRIPTION_QUEUE_CAPACITY` is per receive event subscription. It is not a global provider queue and it is not the actuator transmit queue.

## Troubleshooting

If Terminal 2 reports slot overflow, the provider requested more LoLa queue slots than the S-Core skeleton allows. Use the default queue capacity or increase the S-Core event slot capacity.

If Terminal 2 reports `SampleCountOutOfBounds`, the receive batch size is larger than the subscription queue capacity. The generated adapter clamps this by default; if overriding env vars, keep `SCORE_RECEIVE_BATCH_SIZE <= SCORE_SUBSCRIPTION_QUEUE_CAPACITY`.

If CLI actuation prints `[actuate] OK` but S-Core receives nothing, confirm that `databroker-mw-com` is running and that the provider-local v1 actuator-target bridge is active. For product applications, prefer Kuksa `val.v2` actuation.

If discovery works but no samples flow, check instance specifiers, application IDs, allowed consumer/provider IDs, and event IDs in the S-Core and provider LoLa configs.

If S-Core logs repeated runtime initialization warnings during reconnect loops, stop `databroker-mw-com`, clean `/tmp/mw_com_lola`, restart the S-Core application, then restart `databroker-mw-com`.
