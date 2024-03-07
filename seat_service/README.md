# Seat Service Example

- [Seat Service Example](#seat-service-example)
  - [Overview](#overview)
    - [Context](#context)
    - [Internals](#internals)
  - [Development environment](#development-environment)
    - [Building/testing in Github codespaces](#buildingtesting-in-github-codespaces)
    - [Local development](#local-development)
      - [Prerequisites](#prerequisites)
      - [Usage on CLI](#usage-on-cli)
        - [Building on Ubuntu 20.04](#building-on-ubuntu-2004)
      - [Building in DevContainer](#building-in-devcontainer)
        - [Build Seat Service binaries](#build-seat-service-binaries)
        - [Build Seat Service container](#build-seat-service-container)
    - [Usage in Visual Studio Code](#usage-in-visual-studio-code)
  - [Configuration](#configuration)
    - [Command line arguments](#command-line-arguments)
    - [Environment variables](#environment-variables)
    - [Entrypoint script variables](#entrypoint-script-variables)
    - [Seat Controller configuration](#seat-controller-configuration)
  - [Seat Controller Documentation](#seat-controller-documentation)
  - [Generate documentation](#generate-documentation)

## Overview

This represents the example seat control service. More elaborate or completely differing implementations are target of particular projects providing a vehicle abstraction layer.

### Context

![SeatService_context](docs/assets/SeatService_context.svg)

### Internals

![SeatService_internal](docs/assets/SeatService_internal.svg)

## Development environment

### Building/testing in Github codespaces

For `aarch64` hosts or for quickly testing a pull request it is easier to use [Github codespaces](https://docs.github.com/en/codespaces/overview).

- Create a codespace from [here](https://github.com/eclipse/kuksa.val.services/codespaces)
- Choose: `Configure and create codespace`
- Choose a branch to run on. If you just see `main` and need to test a PR, you can select `Files changed` in the PR, then `Review in codespace`
- Wait several minutes for building codespace devcontainer.
- If everything is OK, you'll see several `vscode` tasks running sequentially (e.g `run-<component>`)
- `run-seatservice` will build the Seat Service from sources using [build-release.sh](./build-release.sh)
- If you need to start another `vscode` task / or stop them all, use `F1`, `Tasks: Run Task`, select desired task
- There are 2 `integration-test` tasks for testing local versions or released containers

### Local development

#### Prerequisites

Most existing build scripts rely on that you have Docker installed.
Some scripts also assumes that you use Ubuntu 20.04 as development environment,
by for example building binaries locally and then copying to a docker environment based on Ubuntu 20.04.

1. Install and configure (if needed) local authentication proxy e.g. CNTLM or Px
1. Install and configure docker: [Get Docker](https://docs.docker.com/get-docker/)

#### Usage on CLI

**NOTE:** Building Seat Service on `aarch64` host is not supported at the moment.

##### Building on Ubuntu 20.04

You can use dedicated build docker script [docker-build.sh](./docker-build.sh) if host environment matches target (Ubuntu 20.04).
Note that you may need to install dependencies - use [.devcontainer/Dockerfile](.devcontainer/Dockerfile) as reference.

``` bash
# Linux: [Ubuntu 20.04]
./seat_service/docker-build.sh

USAGE: ./docker-build.sh [OPTIONS] TARGETS
Standalone build helper for seat_service docker image.

OPTIONS:
  -l, --local      local docker import (does not export tar)
  -v, --verbose    enable plain docker output and disable cache
      --help       show help

TARGETS:
  x86_64|amd64, aarch64|amd64    Target arch to build for, if not set - defaults to multiarch
```

##### Creating Docker container for build

If you are using different distro / version, you may use the devcontainer to compile seat service binaries.
First build the Docker container

``` bash
$ cd seat_service/.devcontainer
$ docker build -f Dockerfile -t seat_service_env:latest .
```

From the seat_service folder, use Docker like below where `<build-command>`is the command you intend to run

``` bash
cd seat_service

# Linux: [x86_64, any version]
docker run --rm -it -v $(pwd):/workspace seat_service_env:latest <build-command>

# Windows (cmd)
docker run --rm -it -v %cd%:/workspace seat_service_env:latest <build-command>

# Windows (Powershell)
docker run --rm -it -v ${PWD}:/workspace seat_service_env:latest <build-command>
```

##### Build Seat Service binaries

Building the seat service via dev container must be triggered from the seat_service root folder, e.g.:

``` bash
# Linux

cd seat_service

# Cleanup any build artifacts
rm -rf bin_vservice-seat_*.tar.gz target/

# Generate bin_vservice-seat_*.tar.gz files for packing seat service container
docker run --rm -it -v $(pwd):/workspace oci_kuksa-val-services-ci:latest /bin/bash -c \
  "./build-release.sh --pack"

# Check if release package is build
ls -la bin_vservice-seat_*.tar.gz
```

It shall now be possible to start the service

``` bash
$ docker run --rm -it -v $(pwd):/workspace xxx:latest target/x86_64/release/install/bin/seat_service
Usage: target/x86_64/release/install/bin/seat_service CAN_IF_NAME [LISTEN_ADDRESS [PORT]]

Environment: SEAT_DEBUG=1 to enable SeatDataFeeder dumps
```

##### Build Seat Service container

Build the container using pre-built binaries: `seat_service/bin_vservice-seat_*.tar.gz`

``` bash
# Linux
ce seat_service
docker build -t seat_service -f Dockerfile .
```

### Usage in Visual Studio Code

It is also possible to open the seat_service directory as a remote container in VScode using the approach
[Developing inside a Container](https://code.visualstudio.com/docs/remote/containers).
All needed tools for VScode are automatically installed in this case

1. Install VScode extension with ID  ```ms-vscode-remote.remote-containers```
1. Hit `F1` and type `Remote-Containers: Reopen in Container`


``` bash
root@aeefe5ca40f5:/workspaces/incubation3/seat_service# ./build-release.sh
root@aeefe5ca40f5:/workspaces/incubation3/seat_service# target/x86_64/release/install/bin/seat_service 
Usage: target/x86_64/release/install/bin/seat_service CAN_IF_NAME [LISTEN_ADDRESS [PORT]]

Environment: SEAT_DEBUG=1 to enable SeatDataFeeder dumps
```
## Configuration

### Command line arguments

``` console
./seat_service can_if_name [listen_address [listen_port]]
```

| cli parameter  | default value     | description                    |
|----------------|-------------------|--------------------------------|
| can_if_name    | -                 | Use socketCAN device           |
| listen_address | `"localhost"`     | Listen address for grpc calls  |
| listen_port    | `50051`           | Listen port for grpc calls     |

### Environment variables

| Environment variable            | default value         | description                       |
|---------------------------------|-----------------------|-----------------------------------|
| `BROKER_ADDR`                   | `"localhost:55555"`   | Connect to databroker `host:port` |
| `VSS`                           | `4`                   | VSS compatibility mode [`3`, `4`] |
| `DAPR_GRPC_PORT`                | `55555`               | Dapr mode: override databroker port replacing `port` value in `$BROKER_ADDR` |
| `VEHICLEDATABROKER_DAPR_APP_ID` | `"vehicledatabroker"` | Dapr app id for databroker        |
| `SEAT_DEBUG`                    | `1`                   | Seat Service debug: 0=ERR, 1=INFO, ...     |
| `DBF_DEBUG`                     | `1`                   | DatabrokerFeeder debug: 0=ERR, 1=INFO, ... |

### Entrypoint script variables

There is dedicated entry point script [val_start.sh](./src/lib/seat_adjuster/seat_controller/tools/val_start.sh)
that runs seat service with additional Environment variable configuration:

| Environment variable            | default value         | description                             |
|---------------------------------|-----------------------|-----------------------------------------|
| `CAN`                           | `"can0"`              | Overrides `can_if_name` cli argument    |
| `SERVICE_HOST`                  | `"0.0.0.0"`           | Overrides `listen_address` cli argument |
| `SERVICE_PORT`                  | `50051`               | Overrides `listen_port` cli argument    |
| `SC_RESET`                      | -                     | If != "0", executes `ecu-reset` script to calibrate seat motors |

**NOTE:** Check `val_start.sh` script comments for less-important Environment variables.

### Seat Controller configuration

Further configuration of the seat controller see [Seat Controller Documentation](#seat-controller-documentation).

## Seat Controller Documentation

Seat Controller module handles SocketCAN messaging and provides Control Loop for moving a seat to desired position.
It also provides `cansim` module for simulating a HW Seat ECU even without `vcan` support (e.g. CI pipeline).

For more details about Seat Controller, Seat CAN Simulator and related tools,
check [SeatController README](./src/lib/seat_adjuster/seat_controller/README.md)

## Generate documentation

- Run Doxygen:
  doxygen is able to run with the following command from the main directory:

  ``` bash
  doxygen ./docs/doxygen/doxyfile
  ```

  or using:

  ``` bash
  build-docu.sh
  ```

- The output will be stored to ``./docs/out``. You can watch the documentation with open the following file in the browser:
  `./docs/doxygen/out/html/index.html`

## KUKSA.val and VSS version dependency

The service examples and related tests in this repository use VSS signals. VSS signals may change over time,
and backward incompatible changes may be introduced as part of major releases.
Some of the tests in this repository relies on using latest version
of [KUKSA.val Databroker](https://github.com/eclipse/kuksa.val/pkgs/container/kuksa.val%2Fdatabroker) and
[KUKSA CAN Provider](https://github.com/eclipse-kuksa/kuksa-can-provider).
Some code in the repository (like [Proto](proto) definitions)
have been copied from [KUKSA.val](https://github.com/eclipse/kuksa.val).

This means that regressions may occur when KUKSA.val or KUKSA.val Feeders are updated. The intention for other KUKSA.val
repositories is to use the latest official VSS release as default. There is a script for manual updating of KUKSA.val proto files:
[update-protobuf.sh](./integration_test/update-protobuf.sh).

```bash
cd integration_test/
./update-protobuf.sh --force
```

Seat Service currently supports 2 modes: (VSS 3.X and 4.0).
As part of VSS 4.0 the instance scheme for seat positions was changed to be based on
`DriverSide/Middle/PassengerSide` rather than `Pos1, Pos2, Pos3`.

By default Seat Service uses VSS 4.0 seat position, but for older dependencies it can be changed to
VSS 3.X compatible by setting Environment variable `VSS=3` for seat service container / cmdline.

### Known locations where an explicit VSS or KUKSA.val version is mentioned

- In [integration_test.yml](./.github/workflows/integration_test.yml)
Uncomment the following line to force VSS 3.X version support in databroker.
`# KDB_OPT: "--vss vss_release_3.1.1.json"`

- In [run-databroker.sh](./.vscode/scripts/run-databroker.sh)
The script gets both VSS3 and 4 json files from KUKSA.val master and starts databroker with the correct version, based on environment variable `USE_VSS3=1`:

    ```bash
    wget -q "https://raw.githubusercontent.com/eclipse/kuksa.val/master/data/vss-core/vss_release_3.0.json" -O "$DATABROKER_BINARY_PATH/vss3.json"
    wget -q "https://raw.githubusercontent.com/eclipse/kuksa.val/master/data/vss-core/vss_release_4.0.json" -O "$DATABROKER_BINARY_PATH/vss4.json"
    ```

- In [prerequisite_settings.json](./prerequisite_settings.json)
hardcoded versions are mentioned for KUKSA.val Databroker, KUKSA.val DBC Feeder and KUKSA.val Client.

- In [test_val_seat.py](./integration_test/test_val_seat.py). Tests for proper seat position datapoint, according to environment variable `USE_VSS3`.

- In Seat Service [main.cc](./seat_service/src/bin/seat_service/main.cc): 2 different Datapoint sets are registered, based on environment variable `VSS` (`3` or `4`).

**NOTE:** Above mentioned locations should be checked for breaking VSS changes on master.