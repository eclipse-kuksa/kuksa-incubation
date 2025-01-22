########################################################################
# Copyright (c) 2024 Contributors to the Eclipse Foundation
#
# See the NOTICE file(s) distributed with this work for additional
# information regarding copyright ownership.
#
# This program and the accompanying materials are made available under the
# terms of the Apache License 2.0 which is available at
# http://www.apache.org/licenses/LICENSE-2.0
#
# SPDX-License-Identifier: Apache-2.0
########################################################################

FROM ghcr.io/rust-cross/rust-musl-cross:x86_64-musl AS builder-amd64
ENV BUILDTARGET="x86_64-unknown-linux-musl"


FROM ghcr.io/rust-cross/rust-musl-cross:aarch64-musl AS builder-arm64
ENV BUILDTARGET="aarch64-unknown-linux-musl"

FROM builder-$TARGETARCH AS builder
ARG TARGETARCH

# This will speed up fetching the crate.io index in the future, see
# https://blog.rust-lang.org/2022/06/22/sparse-registry-testing.html
ENV CARGO_UNSTABLE_SPARSE_REGISTRY=true

RUN echo "Building for $TARGETARCH" \
    mkdir build
COPY . build/
WORKDIR /home/rust/src/build

RUN cargo build --package can-protocol-adapter --release --target $BUILDTARGET
RUN mv target/${BUILDTARGET}/release/can-protocol-adapter /home/rust

FROM scratch AS final

COPY --from=builder /home/rust/can-protocol-adapter can-protocol-adapter

ENTRYPOINT [ "/can-protocol-adapter" ]
