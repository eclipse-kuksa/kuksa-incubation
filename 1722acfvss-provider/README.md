# Open1722 ACF-VSS to KUKSA Databroker Provider

Two binaries that bridge Open1722 ACF-VSS to a KUKSA Databroker:

- **`open1722-kuksa-provider`** — listens for ACF-VSS frames on UDP/raw Ethernet and
  publishes received values to a KUKSA Databroker via the `kuksa.val.v2` gRPC API.
- **`acf-vss-gateway`** — replays a CSV signal log as ACF-VSS frames over UDP or
  raw Ethernet. Useful for feeding recorded data into the provider.

## Prerequisites

- **KUKSA Databroker** (version 0.7.0) running and reachable
- **Rust** toolchain (1.94+)
- **protoc** (protobuf compiler, required by kuksa-rust-sdk)
- **libclang** (required by `bindgen` to generate the Open1722 FFI bindings,
  e.g. Debian/Ubuntu package `libclang-dev`)

The IEEE 1722 / AVTP parsing is provided by the upstream
[`open1722`](https://crates.io/crates/open1722) crate. It vendors and compiles
the COVESA Open1722 C library itself, so no system Open1722 installation is
needed.

The `Cargo.toml` currently redirects `open1722` to a local development
checkout via `[patch.crates-io]`. That checkout must have its vendored C
library initialized first:

```bash
git -C ../../open1722-rs submodule update --init
```

Once `open1722` 0.9.7 is published, the `[patch.crates-io]` section can be
removed and the crates.io release used instead.

## Building

```bash
cargo build
```

## open1722-kuksa-provider

### Usage

#### UDP mode (default)

```bash
cargo run --bin open1722-kuksa-provider -- --kuksa-host http://localhost:55555 --udp-port 17220
```

#### Raw Ethernet mode

```bash
cargo run --bin open1722-kuksa-provider -- \
    --kuksa-host http://localhost:55555 \
    --interface eth0 \
    --mac-address 01:00:5e:00:00:01
```

#### CLI options

```
--kuksa-host <URL>    KUKSA Databroker gRPC endpoint [env: KUKSA_HOST]
--udp-port <PORT>     UDP port to listen on (default: 17220)
--interface <IFNAME>  Network interface for raw Ethernet mode
--mac-address <MAC>   Destination MAC for raw Ethernet (required with --interface)
```

## acf-vss-gateway

Replays a CSV signal log (format described below) as ACF-VSS frames. This uses the same log foramt as the [kuksa-csv-provider](https://github.com/eclipse-kuksa/kuksa-csv-provider).

### Usage

```bash
cargo run --bin acf-vss-gateway -- \
    --csv-file signals-extended.csv \
    --udp-port 17220
```

#### CLI options

```
--csv-file <PATH>           CSV file to replay [env: GATEWAY_CSV_FILE]
--infinite                  Loop over CSV from start after reaching end
--include-actuation         Process target/actuation rows (default: skip)
--delay-multiplier <F64>    Multiply each row's delay (default: 1.0)
--udp-port <PORT>           Destination UDP port [default: 17220]
--destination <IP>          Destination IP (UDP mode) [default: 127.0.0.1]
--interface <IFNAME>        Network interface for raw Ethernet mode
--mac-address <MAC>         Destination MAC for raw Ethernet (required with --interface)
```

### CSV format

Same format as the [KUKSA CSV Provider](https://github.com/eclipse-kuksa/kuksa-csv-provider):

```
field,signal,value,delay,datatype
current,Vehicle.Speed,80.0,0.001,FLOAT
current,Vehicle.Powertrain.Transmission.CurrentGear,1,0.018,INT8
current,Vehicle.Powertrain.Transmission.IsParkLockEngaged,False,0.002,BOOLEAN
```

| Column | Description |
|---|---|
| `field` | `current` (publish) or `target` (actuation; skipped unless `--include-actuation`) |
| `signal` | VSS path |
| `value` | Signal value as a string |
| `delay` | Seconds to wait after sending this row |
| `datatype` | VSS type name: FLOAT, DOUBLE, INT8–INT64, UINT8–UINT64, BOOLEAN, STRING |

### End-to-end example

```bash
# Terminal 1: start provider
./target/release/open1722-kuksa-provider --udp-port 17220

# Terminal 2: replay a CSV log
./target/release/acf-vss-gateway \
    --csv-file signals-extended.csv \
    --udp-port 17220

# Terminal 3: verify
grpcurl -plaintext -d '{"signal_id":{"path":"Vehicle.Speed"}}' \
    localhost:55555 kuksa.val.v2.VAL/GetValue
```

## Architecture

### Pipeline

```
[CSV / Talker] --ACF-VSS (UDP/Ethernet)--> [Provider] --gRPC--> [KUKSA Databroker]
```

The gateway sends, the provider listens. They can run on the same or different
machines — the UDP transport decouples them.

### Crate structure

- `src/open1722_vss.rs` — Safe wrapper around the `open1722` crate for parsing
  ACF-VSS PDU frames (provider)
- `src/open1722_listener.rs` — Raw socket / UDP socket listener (provider)
- `src/provider.rs` — KUKSA Databroker provider integration via gRPC (provider)
- `src/config.rs` — CLI configuration for the provider
- `src/main.rs` — Provider entry point
- `src/gateway.rs` — ACF-VSS frame construction + UDP/raw Ethernet send
- `src/gateway_config.rs` — CLI configuration for the gateway
- `src/csv_reader.rs` — CSV parsing and typed value conversion (gateway)
- `src/main_gateway.rs` — Gateway entry point

### VSS ACF message type detection

VSS is a custom ACF format (`AVTP_ACF_TYPE_VSS = 0x42`) that is not part of
IEEE Std 1722, so it has no `AcfMsgType` variant. The listener dispatches on
the raw ACF message-type field via `AcfCommon::acf_msg_type_raw()`, comparing
it against the local `ACF_TYPE_VSS` constant (see
`src/open1722_listener.rs`).

This replaces the earlier raw-bit (`first_octet >> 1`) workaround, which was
needed before `open1722` exposed an `AcfCommon` accessor. See
[`docs/vss-acf-type-workaround.md`](docs/vss-acf-type-workaround.md) for the
history and
[`docs/open1722-acf-type-access-proposal.md`](docs/open1722-acf-type-access-proposal.md)
for the upstream API proposal.

The listener accepts both version 0 (IEEE 1722-2016) and version 1
(IEEE 1722-2025) NTSCF/TSCF stream headers; the gateway continues to emit
version 0 frames.

## ACF-VSS Data Type Mapping

| ACF-VSS Type | KUKSA VSS Type |
|---|---|
| Bool | Bool |
| Int8/Int16/Int32 | Int32 |
| Int64 | Int64 |
| Uint8/Uint16/Uint32 | Uint32 |
| Uint64 | Uint64 |
| Float | Float |
| Double | Double |
| String | String |

## License

Apache-2.0
