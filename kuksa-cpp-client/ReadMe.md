# CPP based Kuksa client

A pluggable C++ based library to talk to [kuksa-databroker](https://github.com/eclipse-kuksa/kuksa-databroker)
based on kuksa-proto v2 API

Supported APIs from proto files

| Kuksa v2 API       | C++ client status  |
| ------------------ | -------------------|
| GetValue           | :white_check_mark: |
| GetValues          | :white_check_mark: |
| Subscribe          | :white_check_mark: |
| SubscribeById      | :x:|
| Actuate            | :white_check_mark: |
| BatchActuate       | :x:|
| ListMetadata       | :x:|
| PublishValue       | :white_check_mark: |
| OpenProviderStream | :x:|
| GetServerInfo      | :white_check_mark: |

> [!NOTE]
> Refer [example_v2](./example/example_v2.cpp) for API usage

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

Proto sources are available in the submodule kuksa-common

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
