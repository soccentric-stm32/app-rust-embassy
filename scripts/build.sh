#!/usr/bin/env bash
#
# build.sh - Build the Rust demo application inside the stm32-dev-rust
#            container (RCON-001)
#
# Author:  Sandesh Ghimire <sandesh@soccentric.com>
# Usage:   scripts/build.sh            cargo build --release
#          scripts/build.sh clean      remove build/ and target/ first
#
# Outputs: build/embassy-app.elf, build/embassy-app.bin, build/embassy-app.hex,
#          build/embassy-app.img (BOOT image header + binary)
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

# RGEN-002: use one version, commit and timestamp for the firmware and for
# the BOOT image header.  SOURCE_DATE_EPOCH also makes the build reproducible.
VERSION="$(sed -n 's/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' Cargo.toml | head -n 1)"
COMMIT="$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || true)"
STAMP="${SOURCE_DATE_EPOCH:-$(date +%s)}"

export SOURCE_DATE_EPOCH="$STAMP"

"$DEV" run env CARGO_HOME=/work/.cargo-home SOURCE_DATE_EPOCH="$STAMP" cargo build --release
"$DEV" run bash -c '
    set -e
    mkdir -p build
    cp target/thumbv7em-none-eabihf/release/embassy-app build/embassy-app.elf
    arm-none-eabi-objcopy -O binary build/embassy-app.elf build/embassy-app.bin
    arm-none-eabi-objcopy -O ihex   build/embassy-app.elf build/embassy-app.hex
    arm-none-eabi-size build/embassy-app.elf'
"$DEV" run bash -c 'python3 tools/mkimage.py build/embassy-app.bin build/embassy-app.img --version "$1" --commit "$2" --timestamp "$3"' _ "$VERSION" "$COMMIT" "$STAMP"
