# app-rust-embassy

The NUCLEO-F767ZI demo application written in Rust on the Embassy async framework. It runs under the A/B bootloader from either slot and does exactly the same job as the C FreeRTOS demo: LED patterns, the user button, HWLINK communication with the desktop GUI, watchdog supervision and self-confirmation after a health check. It shows how a safe, allocation-free Rust application can live inside an existing C bootloader and toolchain ecosystem, and lets you compare Rust and C side by side on the same hardware.

## Features

- Pure Rust (`no_std`, no allocator) on the Embassy async executor
- Runs from either bootloader slot, linked at `0x0800C000`
- Four async tasks: health, comms, button and LED
- Seven LED patterns on LD1/LD2 with LD3 reserved for faults
- B1 button with EXTI, 30 ms debounce and short/long press actions
- HWLINK over USART3 at 115200 or 921600 baud
- Fixed-buffer FlatBuffers codec, verified against the reference encoder
- COBS framing and CRC-32 shared with the C implementations
- Health task supervises check-ins and is the only watchdog feeder
- CONFIRMED record written after the trial health check
- Fault record in a reserved no-init RAM window that survives every reset
- Unsolicited ButtonEvent messages
- Crate versions locked in `Cargo.lock`
- Produces a ready-to-flash bootloader image (`embassy-app.img`)
- Reproducible Docker build environment with the pinned Rust toolchain

## Build and run

```bash
scripts/docker-dev.sh build   # build the stm32-dev-rust container image once
scripts/build.sh              # build inside the container
make flash                    # upload build/embassy-app.img through the bootloader
make help                     # list all targets
```

## Related projects

- [bootloader](https://github.com/soccentric-stm32/bootloader) — A/B bootloader with UART update, CRC32 checks, trial boot and rollback
- [app-c-freertos](https://github.com/soccentric-stm32/app-c-freertos) — Demo application in C on FreeRTOS
- [app-c-zephyr](https://github.com/soccentric-stm32/app-c-zephyr) — Demo application in C on Zephyr RTOS
- [app-rust-zephyr](https://github.com/soccentric-stm32/app-rust-zephyr) — Demo application in Rust on Zephyr RTOS
- [validation-firmware](https://github.com/soccentric-stm32/validation-firmware) — HWVAL hardware validation firmware
- [validation-gui](https://github.com/soccentric-stm32/validation-gui) — HWVAL Studio: desktop GUI for tests, updates and flashing
- [factory-station](https://github.com/soccentric-stm32/factory-station) — HWVAL Factory: end-of-line test and provisioning station
- [soccentric-stm32.github.io](https://github.com/soccentric-stm32/soccentric-stm32.github.io) — Project website

## License

Released under the [MIT License](LICENSE) © 2026 Sandesh Ghimire. You are free
to use, modify and ship it in your own products.

Contributions are welcome. Fork the repository, make your change on a branch and
open a pull request. Bug reports, ideas and improvements are all appreciated.
