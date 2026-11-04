# app-rust-freertos

The demo application of the NUCLEO-F767ZI system written in Rust on the Embassy async framework. It runs under the A/B bootloader from either slot and does the same job as the C FreeRTOS demo, so the C, Rust and Zephyr versions can be compared like for like.

| | |
|---|---|
| Requirements | [SRS.md](SRS.md) — SRS-RUST-F767 (v0.1 draft) |
| Functional baseline | [SRS-DEMO-F767](https://github.com/soccentric-stm32/app-c-freertos/blob/HEAD/SRS.md) — same behaviour as the C FreeRTOS demo |
| Board | NUCLEO-F767ZI — see [Hardware.md](https://github.com/soccentric-stm32/validation-firmware/blob/HEAD/Hardware.md) |
| Status | Feature complete: VTOR, 216 MHz clock, caches, LD1/LD2/LD3 patterns, B1 short/long press, USART3 HWLINK (COBS, CRC32, fixed-buffer FlatBuffers), health/IWDG/CONFIRMED, fault record and BOOT image file. |
| Build environment | [docker/README.md](docker/README.md): `stm32-dev-rust` = the shared `stm32-dev` image + Rust 1.99.1 (`thumbv7em-none-eabihf`, clippy, rustfmt) |

## Build and flash

Everything runs in the container through `scripts/docker-dev.sh`:

```bash
scripts/docker-dev.sh build   # build the image once (and the stm32-dev base if missing)
scripts/build.sh              # build inside the container
make flash                    # upload build/rust-app.img through BOOT (bootctl)
make erase-all                # erase BOOT + slots (asks for confirmation)
make help                     # all targets
```

Outputs in `build/`: `rust-app.elf`, `rust-app.bin`, `rust-app.hex` and `rust-app.img` (256-byte BOOT header + binary). The image is linked at `0x0800C000` (bootloader slot) and starts only through the bootloader. `make flash` uploads the `.img` with `bootctl.py update --activate`; never program the raw `.elf`/`.bin` into a slot, because BOOT requires the header and metadata records. Point `BOOTCTL` or `PORT` elsewhere if the defaults do not fit. Crate versions are locked in `Cargo.lock`; the crate cache is kept in `.cargo-home/`.

## What runs

| Task | Job |
|---|---|
| `health` | IWDG refresh, 2 s task check-in supervision, 5–10 s health check and the only flash write (`CONFIRMED` at `0x08008140`) |
| `comms` | USART3 115200/921600 HWLINK: `Hello`, `Ping`, `GetStatus`, `SetLed`, `SetLedAll`, `ClearFault`, `SetBaud`, `Reset`, `EnterBootloader`; unsolicited `ButtonEvent` |
| `button` | PC13 EXTI, 30 ms debounce, short press cycles patterns, long press restores `HEARTBEAT` |
| `led` | LD1/LD2 Appendix A patterns (`OFF`, `ON`, `BLINK`, `HEARTBEAT`, `FAST`, `CHASE`, `MORSE_SOS`), LD3 reserved for faults/health failure |

`protocol/hwlink.fbs` is the schema. `src/hwlink/fb.rs` is a fixed-buffer
FlatBuffers codec because the upstream Rust runtime needs an allocator
(OPEN-R01); the wire format is verified against `flatc --python`. `tools/mkimage.py`
wraps the binary in the BOOT image header used by `build/rust-app.img`.

## Starting a new application

1. Rename the package in `Cargo.toml` and the `name` field in `tools/mkimage.py`.
2. Keep `memory.x` at `0x0800C000` and `SCB.VTOR` in `boot::set_vtor()`.
   RAM is DTCM only: 127 KB for data/bss and the stack, plus the reserved
   1 KB no-init window at `0x2001FC00` (`noinit.x`) where the fault record
   lives. The window is above BOOT's 60 KB budget and above the stack, so
   the record survives every reset; keep `.noinit` out of `RAM`.
3. Keep the IWDG pet at the top of `main` and let `health` own the refresh.
4. Add tasks with `#[embassy_executor::task]`, give them static channels in
   `src/ipc.rs`, and call `ipc::check_in()` at least once per second.
5. Extend `protocol/hwlink.fbs` and `src/hwlink/mod.rs` together, adding fields
   only at the end (LINK-013).
6. Rebuild and flash; `scripts/build.sh` also refreshes `build/rust-app.img`.

## Related projects

| Repository | Role |
|---|---|
| [validation-firmware](https://github.com/soccentric-stm32/validation-firmware) | HWVAL hardware validation firmware |
| [bootloader](https://github.com/soccentric-stm32/bootloader) | A/B bootloader (UART update, CRC32, trial boot and rollback) |
| [app-c-freertos](https://github.com/soccentric-stm32/app-c-freertos) | FreeRTOS demo application in C |
| [app-rust-freertos](https://github.com/soccentric-stm32/app-rust-freertos) | The same demo in Rust (Embassy) |
| [app-c-zephyr](https://github.com/soccentric-stm32/app-c-zephyr) | The same demo on Zephyr RTOS |
| [app-rust-zephyr](https://github.com/soccentric-stm32/app-rust-zephyr) | The same demo in Rust on Zephyr RTOS |
| [validation-gui](https://github.com/soccentric-stm32/validation-gui) | PySide6 GUI: runs HWVAL tests, updates and flashes firmware |
| [factory-station](https://github.com/soccentric-stm32/factory-station) | HWVAL Factory: end-of-line test and provisioning station |

## CI and releases

The [Jenkinsfile](Jenkinsfile) runs the build and checks, then **Package**
builds `dist/app-rust-freertos-<version>.tar.gz` with the outputs, a `manifest.json`
(version, commit, build) and `SHA256SUMS`, and archives `dist/`. For a tag
`v<version>`, **Release** publishes `dist/` as a GitHub release
(`ci/github_release.py`, Jenkins credential `github-release-token`); the tag
must match the version the build reports (`ci/version.sh`), and 0.x versions
are marked as pre-releases.

To release, bump the version (`version` in `Cargo.toml`), push it, then push a matching tag:

```bash
git tag v0.2.0
git push origin v0.2.0
```

## License

[MIT](LICENSE) © 2026 Sandesh Ghimire. Use it as a starting point for your own
products. Third-party code (STM32CubeF7, FreeRTOS, LwIP, Embassy, Zephyr and
others) is fetched at build time and keeps its own licence.

Author: Sandesh Ghimire <sandesh@soccentric.com>
