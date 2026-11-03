#!/usr/bin/env bash
#
# build.sh - Build the Rust demo application inside the stm32-dev-rust
#            container (RCON-001)
#
# Author:  Sandesh Ghimire <sandesh@soccentric.com>
# Usage:   scripts/build.sh            cargo build --release
#          scripts/build.sh clean      remove build/ and target/ first
#
# Outputs: build/rust-app.elf, build/rust-app.bin, build/rust-app.hex
#
# The crate cache lives in .cargo-home/ (git-ignored) so it survives between
# container runs and belongs to the calling user.
#

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
DEV="$ROOT/scripts/docker-dev.sh"
cd "$ROOT"

if [[ "${1:-}" == "clean" ]]; then
    rm -rf build target
fi

"$DEV" run env CARGO_HOME=/work/.cargo-home cargo build --release
"$DEV" run bash -c '
    set -e
    mkdir -p build
    cp target/thumbv7em-none-eabihf/release/rust-app build/rust-app.elf
    arm-none-eabi-objcopy -O binary build/rust-app.elf build/rust-app.bin
    arm-none-eabi-objcopy -O ihex   build/rust-app.elf build/rust-app.hex
    arm-none-eabi-size build/rust-app.elf'
