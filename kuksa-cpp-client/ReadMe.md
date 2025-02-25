# CPP based Kuksa client

A pluggable Cpp based library to talk to [kuksa-databroker](https://github.com/eclipse-kuksa/kuksa-databroker)
based on v2 API

APIs

- `connect("127.0.0.1:55555")`
    Establish connection to a databroker end-point
- `get("Vehicle.Speed")`
    Get a datapoint value
- `set("Vehicle.Speed",value)`
    Set a datapoint with the corresponding value

## How to build

This repo contains a [justfile](https://github.com/casey/just) to help in
setting up and installing dependencies

```shell
# Install conan and setup remote
just prepare
# pull in the conan deps
just configure
# build the project
just build
```

## Proto

Proto sources are available in the submodule (kuksa-common)[../kuksa-common/]

## Examples

There are two examples provided

- based on kuksa::v1 API [example_v1](example/example_v1.cpp)
- based on kuksa::v2 API [example_v2](example/example_v2.cpp)

### Running examples

```shell
just run-example-v2
```

### Debug hints

```shell
export GRPC_TRACE=all
export GRPC_VERBOSITY=DEBUG
```
