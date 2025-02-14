# KUKSA Incubation

![KUKSA Logo](./assets/logo.png)

This is a KUKSA repository for incubation components.
That is components that as of today is not intended for production purposes.
They could however be useful as design-base or for demo-purposes.

Some implications on that this is a repository for incubation components

* Vulnerability Management (Dependabot) is not enabled for this repository.
* Limited effort spent on artifacts like Bill-of-Material and Open Source Scanning
* Limited effort spent on testing before creating tags and releases
* Semantic meaning of tags is not well defined

Incubation components may be promoted to "real components" in a separate repository if there
is a strong interest in the component and active maintainers/contributors willing to maintain the component.

## Content

Component |  Content | Comment/Status
----------|----------|---------------
[HVAC Service](hvac_service) | Python service example
[Seat Service](seat_service) | C++ service example
[eCAL Provider](ecal2val) | Python provider for [eCAL](https://projects.eclipse.org/projects/automotive.ecal)
[PS4/PS5 - 2021 Formula Provider](./fone2val) | F1 Telemetrydata source for [KUKSA Databroker](https://github.com/eclipse/kuksa.val/tree/master/kuksa_databroker)
[KUKSA GO Client](kuksa_go_client)   | Example client written in the [GO](https://go.dev/) programming language for easy interaction with KUKSA Databroker and Server
[ESP32 gRPC provider](gRPC-on-ESP32)   | Example for interacting with the [KUKSA Databroker](https://github.com/eclipse/kuksa.val/tree/master/kuksa_databroker) with ESP32-based microcontrollers
[Zenoh Kuksa Provider](zenoh-kuksa-provider)   | Bridge component between the [KUKSA Databroker](https://github.com/eclipse/kuksa.val/tree/master/kuksa_databroker) and [Eclipse Zenoh](https://github.com/eclipse-zenoh/zenoh)
[CAN Protocol Adapter](https://github.com/eclipse-kuksa/kuksa-incubation/tree/main/can-protocol-adapter)   | Rust module to communicate between CAN devices and the Kuksa DataBroker based on request/response mode.

## Contribution

For contribution guidelines see [CONTRIBUTING.md](CONTRIBUTING.md)

## Pre-commit set up

This repository is set up to use [pre-commit](https://pre-commit.com/) hooks.
Use `pip install pre-commit` to install pre-commit.
After you clone the project, run `pre-commit install` to install pre-commit into your git hooks.
Pre-commit will now run on every commit.
Every time you clone a project using pre-commit running pre-commit install should always be the first thing you do.
