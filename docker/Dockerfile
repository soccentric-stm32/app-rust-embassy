#
# Dockerfile - Rust (Embassy) build environment on top of stm32-dev
#
# Author:  Sandesh Ghimire <sandesh@soccentric.com>
# Target:  NUCLEO-F767ZI (STM32F767ZI, Cortex-M7 with FPU)
#
# Adds a pinned Rust toolchain with the thumbv7em-none-eabihf target to the
# shared stm32-dev image, so OpenOCD, GDB, picocom and arm-none-eabi-objcopy
# work exactly as for the C projects. Use it through scripts/docker-dev.sh.
#

ARG BASE_IMAGE=stm32-dev:latest
FROM ${BASE_IMAGE}

LABEL org.opencontainers.image.title="stm32-dev-rust" \
      org.opencontainers.image.description="STM32 Rust/Embassy build, flash and debug environment" \
      org.opencontainers.image.authors="Sandesh Ghimire <sandesh@soccentric.com>"

ARG DEBIAN_FRONTEND=noninteractive
# gcc + libc6-dev: host linker for Cargo build scripts and proc macros.
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl gcc libc6-dev \
    && rm -rf /var/lib/apt/lists/*

# Toolchain in /opt, read-only at run time. Builds set CARGO_HOME to a
# directory in the project (scripts/build.sh) so the crate cache survives
# between container runs and is owned by the calling user.
ARG RUST_VERSION=1.99.1
ENV RUSTUP_HOME=/opt/rustup \
    PATH=/opt/cargo/bin:${PATH}
RUN curl -sSf https://sh.rustup.rs \
      | CARGO_HOME=/opt/cargo sh -s -- -y --no-modify-path --profile minimal \
          --default-toolchain "${RUST_VERSION}" --target thumbv7em-none-eabihf \
          --component clippy --component rustfmt \
    && chmod -R a+rX /opt/rustup /opt/cargo \
    && printf '%s\n' "${RUST_VERSION}" > /opt/rustup/VERSION

LABEL org.opencontainers.image.version="rust-${RUST_VERSION}"

WORKDIR /work
CMD ["bash"]
