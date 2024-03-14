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

## Contribution

For contribution guidelines see [CONTRIBUTING.md](CONTRIBUTING.md)

## Pre-commit set up
This repository is set up to use [pre-commit](https://pre-commit.com/) hooks.
Use `pip install pre-commit` to install pre-commit.
After you clone the project, run `pre-commit install` to install pre-commit into your git hooks.
Pre-commit will now run on every commit.
Every time you clone a project using pre-commit running pre-commit install should always be the first thing you do.
