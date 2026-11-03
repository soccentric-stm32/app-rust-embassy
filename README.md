# soccentric-stm32-rust-application

The demo application of the NUCLEO-F767ZI system written in Rust on the Embassy async framework. It runs under the A/B bootloader from either slot and does the same job as the C FreeRTOS demo, so the C, Rust and Zephyr versions can be compared like for like.

| | |
|---|---|
| Requirements | [SRS.md](SRS.md) — SRS-RUST-F767 (v0.1 draft) |
| Functional baseline | [SRS-DEMO-F767](https://github.com/soccentric-stm32/soccentric-stm32-application/blob/HEAD/SRS.md) — same behaviour as the C FreeRTOS demo |
| Board | NUCLEO-F767ZI — see [Hardware.md](https://github.com/soccentric-stm32/soccentric-stm32-validation-firmware/blob/HEAD/Hardware.md) |
| Status | Skeleton: VTOR, 216 MHz clock, caches, heartbeat task on LD1, panic/HardFault paths. Comms, button, health and the image file still to come. |
| Build environment | [docker/README.md](docker/README.md): `stm32-dev-rust` = the shared `stm32-dev` image + Rust 1.99.1 (`thumbv7em-none-eabihf`, clippy, rustfmt) |

## Build and flash

Everything runs in the container through `scripts/docker-dev.sh`:

```bash
scripts/docker-dev.sh build   # build the image once (and the stm32-dev base if missing)
scripts/build.sh              # build inside the container
make flash                    # flash build/rust-app.elf over SWD, verify, reset
make help                     # all targets
```

Outputs in `build/`: `rust-app.elf`, `rust-app.bin`, `rust-app.hex`. The image is linked at `0x0800C000` (bootloader slot) and starts only through the bootloader. Crate versions are locked in `Cargo.lock`; the crate cache is kept in `.cargo-home/`.

## Related projects

| Repository | Role |
|---|---|
| [soccentric-stm32-validation-firmware](https://github.com/soccentric-stm32/soccentric-stm32-validation-firmware) | HWVAL hardware validation firmware |
| [soccentric-stm32-bootloader](https://github.com/soccentric-stm32/soccentric-stm32-bootloader) | A/B bootloader (UART update, CRC32, trial boot and rollback) |
| [soccentric-stm32-application](https://github.com/soccentric-stm32/soccentric-stm32-application) | FreeRTOS demo application in C |
| [soccentric-stm32-rust-application](https://github.com/soccentric-stm32/soccentric-stm32-rust-application) | The same demo in Rust (Embassy) |
| [soccentric-stm32-zephyr-application](https://github.com/soccentric-stm32/soccentric-stm32-zephyr-application) | The same demo on Zephyr RTOS |
| [soccentric-stm32-validation-gui](https://github.com/soccentric-stm32/soccentric-stm32-validation-gui) | PySide6 GUI: runs HWVAL tests, updates and flashes firmware |
| [soccentric-stm32-factory-provisioning](https://github.com/soccentric-stm32/soccentric-stm32-factory-provisioning) | HWVAL Factory: end-of-line test and provisioning station |

## License

[MIT](LICENSE) © 2026 Sandesh Ghimire. Use it as a starting point for your own
products. Third-party code (STM32CubeF7, FreeRTOS, LwIP, Embassy, Zephyr and
others) is fetched at build time and keeps its own licence.

Author: Sandesh Ghimire <sandesh@soccentric.com>
