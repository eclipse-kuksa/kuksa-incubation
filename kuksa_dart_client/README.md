# Running the KUKSA Dart Client

An example client written in the [Dart](https://dart.dev/) programming language
for easy interaction with KUKSA Databroker. It is the Dart counterpart to the
sibling [`kuksa_go_client`](../kuksa_go_client) and
[`kuksa-cpp-client`](../kuksa-cpp-client), and is built on the published
[`kuksa_dart_sdk`](https://pub.dev/packages/kuksa_dart_sdk) — so, unlike the Go
and C++ clients, no protobuf compiler or code generation step is required; the
SDK ships the generated `kuksa.val.v2` stubs.

## Execute the example

### Set up Dart

- If you do not have the Dart SDK installed, follow
  <https://dart.dev/get-dart> (Dart `3.0` or above).

### Start KUKSA Databroker

The example talks to a running databroker over `kuksa.val.v2` gRPC. Start one:

```
> cargo run --bin databroker
```

Or start the appropriate docker container:

```
> docker run -it --rm --net=host ghcr.io/eclipse-kuksa/kuksa-databroker:main
```

*Note: only insecure mode is exercised by this sample; TLS is not configured.*

### Run the Dart client

From this directory:

```
> dart pub get
> dart run bin/kuksa_dart_client.dart
```

The sample connects to the databroker and exercises each core `kuksa.val.v2`
API — server info, read, write, metadata listing, and subscribe — then exits.

### Configuration

Connection and signal settings come from `kuksa-client.json` if present
(an example `kuksa-client-grpc.json` is provided), with these defaults:

| Key    | Default         | Meaning                          |
|--------|-----------------|----------------------------------|
| `host` | `localhost`     | databroker host                  |
| `port` | `55555`         | databroker gRPC port             |
| `path` | `Vehicle.Speed` | signal the sample read/writes    |

Any of these can be overridden on the command line (command-line flags take
precedence over the config file):

```
> dart run bin/kuksa_dart_client.dart --host localhost --port 55555 --path Vehicle.Speed
```

To use the provided config, copy or link it to `kuksa-client.json`:

```
> cp kuksa-client-grpc.json kuksa-client.json
```

## Tests

The configuration handling is covered by a hermetic unit test (no databroker
required):

```
> dart pub get
> dart test
```

## Linting

```
> dart analyze
```
