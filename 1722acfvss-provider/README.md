# Open1722 ACF-VSS to KUKSA Databroker Provider

Two binaries that bridge Open1722 ACF-VSS to a KUKSA Databroker:

- **`open1722-kuksa-provider`** — listens for ACF-VSS frames on UDP/raw Ethernet and
  publishes received values to a KUKSA Databroker via the `kuksa.val.v2` gRPC API.
- **`acf-vss-gateway`** — replays a CSV signal log as ACF-VSS frames over UDP or
  raw Ethernet. Useful for feeding recorded data into the provider.

## Prerequisites

- **Open1722** (version 0.9.x) installed with libraries at `/usr/local/lib/`
- **KUKSA Databroker** (version 0.7.0) running and reachable
- **Rust** toolchain (1.94+)
- **protoc** (protobuf compiler, required by kuksa-rust-sdk)

Open1722 must be built and installed:

```bash
git clone https://github.com/COVESA/Open1722.git
cd Open1722
mkdir build && cd build
cmake ..
make
sudo make install
```

The Open1722 headers declare several functions as `static inline` (for performance
on microcontrollers). A thin C wrapper library exports them as callable symbols
for FFI:

```bash
# From the provider repository root:
mkdir  -p build-ffi && cd build-ffi
cmake ../ffi-wrapper/
make
cp ./libopen1722_ffi.so* ../
# Optionally install:
#sudo cp libopen1722_ffi.so /usr/local/lib/ && sudo ldconfig
```

## Building

```bash
LD_LIBRARY_PATH=./:$LD_LIBRARY_PATH cargo build
```

(Adjust `LD_LIBRARY_PATH` depending on where you installed/copied the .so)

## open1722-kuksa-provider

### Usage

#### UDP mode (default)

```bash
LD_LIBRARY_PATH=./:$LD_LIBRARY_PATH cargo run --bin open1722-kuksa-provider  -- --kuksa-host http://localhost:55555  --udp-port 17220
```

#### Raw Ethernet mode

```bash
.LD_LIBRARY_PATH=./:$LD_LIBRARY_PATH cargo run --bin open1722-kuksa-provider -- \
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
LD_LIBRARY_PATH=./:$LD_LIBRARY_PATH cargo run --bin acf-vss-gateway -- \
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

- `ffi-wrapper/` — Thin C shared library exporting Open1722 `static inline`
  functions as callable symbols for Rust FFI
- `src/open1722/ffi.rs` — Unsafe C FFI bindings for libopen1722, libopen1722custom,
  and libopen1722_ffi (shared by both binaries)
- `src/open1722_vss.rs` — Safe wrapper for parsing ACF-VSS PDU frames (provider)
- `src/open1722_listener.rs` — Raw socket / UDP socket listener (provider)
- `src/provider.rs` — KUKSA Databroker provider integration via gRPC (provider)
- `src/config.rs` — CLI configuration for the provider
- `src/main.rs` — Provider entry point
- `src/gateway.rs` — ACF-VSS frame construction + UDP/raw Ethernet send
- `src/gateway_config.rs` — CLI configuration for the gateway
- `src/csv_reader.rs` — CSV parsing and typed value conversion (gateway)
- `src/main_gateway.rs` — Gateway entry point
- `build.rs` — Linker directives for all three shared libraries

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
