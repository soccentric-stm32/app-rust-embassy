# Docker Development Environment

| | |
|---|---|
| **Author** | Sandesh Ghimire <sandesh@soccentric.com> |
| **Date** | 2026-11-03 |
| **Board** | NUCLEO-F767ZI |

The image `stm32-dev-rust:latest` extends the shared `stm32-dev:latest` image
of the C projects, so OpenOCD, GDB, picocom, `arm-none-eabi-objcopy` and the
`scripts/docker-dev.sh` commands (flash, debug, serial, probe) work exactly as
there. It adds Rust 1.99.1 with the `thumbv7em-none-eabihf` target, `clippy` and `rustfmt`, in `/opt/rustup`.

| File | Role |
|---|---|
| [Dockerfile](Dockerfile) | The rust layer on top of `stm32-dev` (build args `RUST_VERSION`) |
| [Dockerfile.base](Dockerfile.base) | Identical copy of the validation firmware's Dockerfile, used to build `stm32-dev` when it is not on the machine yet |

`scripts/docker-dev.sh build` builds the base image first if it is missing,
then this one. The full description of the base image is in the validation
firmware's
[docker/README.md](https://github.com/soccentric-stm32/validation-firmware/blob/HEAD/docker/README.md).

| Variable | Default |
|---|---|
| `STM32_IMAGE` | `stm32-dev-rust:latest` |
| `BASE_IMAGE` | `stm32-dev:latest` |
