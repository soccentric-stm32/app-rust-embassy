# Software Requirements Specification — NUCLEO-F767ZI Rust (Embassy) Demo Application

| | |
|---|---|
| **Document** | SRS-RUST-F767 |
| **Version** | 0.1 (draft) |
| **Author** | Sandesh Ghimire <sandesh@soccentric.com> |
| **Date** | 2026-11-03 |
| **Status** | Draft for review |
| **Hardware** | NUCLEO-F767ZI, STM32F767ZIT6 rev Z (see [Hardware.md](https://github.com/soccentric-stm32/soccentric-stm32-validation-firmware/blob/HEAD/Hardware.md)) |
| **Software base** | Rust 1.99.1 (`thumbv7em-none-eabihf`), Embassy (`embassy-stm32` 0.6, `embassy-executor` 0.10, `embassy-time` 0.5), `cortex-m-rt` 0.7 |

---

## Contents

1. [Introduction](#1-introduction)
2. [Overall description](#2-overall-description)
3. [General requirements](#3-general-requirements)
4. [Bootloader contract](#4-bootloader-contract)
5. [Concurrency (Embassy) requirements](#5-concurrency-embassy-requirements)
6. [Functional requirements](#6-functional-requirements)
7. [HWLINK in Rust](#7-hwlink-in-rust)
8. [Safety and robustness](#8-safety-and-robustness)
9. [Non-functional requirements](#9-non-functional-requirements)
10. [Verification matrix](#10-verification-matrix)
- [Appendix A — Mapping to the C demo](#appendix-a--mapping-to-the-c-demo)
- [Appendix B — Open items](#appendix-b--open-items)

---

## 1. Introduction

### 1.1 Purpose

This document specifies **RDEMO**, the demo application of SRS-DEMO-F767
written in Rust on the Embassy async framework. It does the same job as the C
demo: it blinks the LEDs, reads the button, speaks HWLINK to the GUI, runs
under the A/B bootloader and confirms itself after a health check. Because the
behaviour is the same, the C, Rust and Zephyr demos can be compared like for
like: code size, RAM, start-up time, latency and effort.

### 1.2 Scope

RDEMO shall implement the **behaviour** of SRS-DEMO-F767 §6–§9 (LEDs,
button, communication, health and confirmation) and its bootloader contract
(§4), with the implementation constraints of this document. Where this SRS
says "as [DEMO] X", requirement X of SRS-DEMO-F767 applies unchanged; only
its implementation differs.

RDEMO is a demo and template. It has no networking, storage or other
peripherals.

### 1.3 Definitions

| Term | Meaning |
|---|---|
| **RDEMO** | The Rust application specified here |
| **[DEMO]** | The C FreeRTOS demo application, SRS-DEMO-F767 (functional baseline) |
| **Embassy** | Async executor, HAL and time driver for embedded Rust |
| **Task** | An `#[embassy_executor::task]` async function |
| **PAC** | Peripheral access crate (register-level), used through `embassy-stm32` |
| **HWLINK** | UART framing and FlatBuffers protocol ([LINK]) |
| **BOOT** | The A/B bootloader, SRS-BOOT-F767 |

### 1.4 Requirement conventions

**shall / should / may**, IDs `<AREA>-<NNN>`, methods **T** (test),
**D** (demonstration), **I** (inspection), **A** (analysis), as in [BOOT] §1.4.

### 1.5 References

| ID | Document |
|---|---|
| [DEMO] | [soccentric-stm32-application/SRS.md](https://github.com/soccentric-stm32/soccentric-stm32-application/blob/HEAD/SRS.md) — SRS-DEMO-F767 |
| [BOOT] | [soccentric-stm32-bootloader/SRS.md](https://github.com/soccentric-stm32/soccentric-stm32-bootloader/blob/HEAD/SRS.md) — SRS-BOOT-F767 |
| [GUI] | [soccentric-stm32-validation-gui/SRS.md](https://github.com/soccentric-stm32/soccentric-stm32-validation-gui/blob/HEAD/SRS.md) — SRS-GUI-F767 |
| [LINK] | [GUI] Appendix A — HWLINK protocol (normative) |
| [ZDEMO] | [soccentric-stm32-zephyr-application/SRS.md](https://github.com/soccentric-stm32/soccentric-stm32-zephyr-application/blob/HEAD/SRS.md) — SRS-ZEPHYR-F767 |
| [EMB] | Embassy documentation and `embassy-stm32` STM32F7 support |
| [RM0410] | ST, STM32F76xxx/F77xxx reference manual |

---

## 2. Overall description

### 2.1 Product perspective

RDEMO replaces the C demo in a bootloader slot. The GUI cannot tell them apart
except by the `HelloReply` name and version: both report role `DEMO`.

```
 BOOT (C, bank 1 sectors 0-1) ── hand-over at 0x0800C000 ──> RDEMO (Rust)
                                                              ├─ led task
                                                              ├─ button task
                                                              ├─ comms task  <── HWLINK ──> GUI
                                                              └─ health task ──> IWDG, CONFIRMED
```

### 2.2 Constraints

| ID | Constraint |
|---|---|
| RCON-001 | RDEMO shall be built with the pinned Rust toolchain in the project Docker image (`stm32-dev-rust`, which extends the shared `stm32-dev` image) with one command (`scripts/build.sh`). |
| RCON-002 | The toolchain version shall be pinned in `rust-toolchain.toml` and in the image; crate versions shall be pinned by a committed `Cargo.lock`. `embassy-stm32` shall be built with the `dual-bank` feature (product configuration, [BOOT] BASM-001). |
| RCON-003 | RDEMO shall be `#![no_std]` and `#![no_main]`; no heap allocator shall be linked unless an open item decides otherwise (Appendix B). |
| RCON-004 | `unsafe` code shall be limited to the boot hand-over items (VTOR, metadata write, backup registers) and shall carry a `// SAFETY:` comment each. |
| RCON-005 | RDEMO shall run only under BOOT, linked at `0x0800 C000` ([BOOT] LAY-002). |
| RCON-006 | RDEMO shall use USART3 (PD8/PD9) for HWLINK and never reconfigure PA13/PA14 (SWD) or PH0/PH1 (HSE). |

### 2.3 Assumptions

As [DEMO] §2.4 (board provisioned for BOOT, LED and button pins, working VCP).

---

## 3. General requirements

| ID | Requirement | Method |
|---|---|---|
| RGEN-001 | The build shall produce `build/rust-app.elf`, `.bin`, `.hex` and the bootloader image file `rust-app.img` (via [BOOT] BHOST-001). | I |
| RGEN-002 | RDEMO shall embed its crate version, git commit and build date (build script), and the same values shall go into the image header. | I, T |
| RGEN-003 | RDEMO shall run the core at 216 MHz from the 8 MHz HSE bypass clock through the PLL (M = 8, N = 432, P = 2, Q = 9), APB1 /4, APB2 /2, with the flash latency, over-drive and caches as [DEMO] DGEN-003. | T |
| RGEN-004 | The panic handler shall record the panic location in a no-init RAM area, show the fault pattern on LD3 and reset (as [DEMO] DGEN-004). Release builds shall not include panic message strings unless a build feature enables them. | T |
| RGEN-005 | HardFault shall be handled as [DEMO] DGEN-004 (`cortex-m-rt` `HardFault` exception). | T |
| RGEN-006 | The reset cause shall be read from `RCC_CSR`, kept for `Status`, then cleared (as [DEMO] DGEN-005). | T |
| RGEN-007 | The code shall build with `#![deny(warnings)]` and pass `cargo clippy -- -D warnings`. | I |
| RGEN-008 | All source files shall carry the author header of the project. | I |

---

## 4. Bootloader contract

RDEMO shall meet [BOOT] §11 APP-001…APP-007, as [DEMO] DBT-001…DBT-006:

| ID | Requirement | Method |
|---|---|---|
| RBT-001 | `memory.x` shall place `FLASH` at `0x0800 C000` with length 976 KB and `RAM` at `0x2000 0000`; the `embassy-stm32` `memory-x` feature shall not be used. | I |
| RBT-002 | The first statement of `main` shall set `SCB.VTOR` to `0x0800 C000` before the HAL is initialised. | I, T |
| RBT-003 | RDEMO shall configure clocks from the reset state BOOT leaves ([BOOT] HND-001). | T |
| RBT-004 | RDEMO shall read the hand-over record (`RTC_BKP2R`) and report slot and trial state, as [DEMO] DBT-004. | T |
| RBT-005 | `EnterBootloader` shall behave as [DEMO] DBT-005. | T |
| RBT-006 | The only flash write shall be the `CONFIRMED` record ([BOOT] Appendix B) in the running slot's metadata sector, through one audited function. | I |

---

## 5. Concurrency (Embassy) requirements

| ID | Requirement | Method |
|---|---|---|
| RRT-001 | RDEMO shall use the Embassy thread-mode executor (`platform-cortex-m`, `executor-thread`) and the `embassy-time` driver on a general-purpose timer (not SysTick-dependent code in tasks). | I |
| RRT-002 | RDEMO shall have four tasks with the jobs of [DEMO] §5.2: `health`, `comms`, `button`, `led`. Task priority differences of the C demo shall be met by an interrupt-mode executor for `health` and `comms` if measurements show the thread executor misses DRT-012 timing (Appendix B, OPEN-R03). | I, T |
| RRT-003 | Tasks shall communicate through `embassy-sync` channels or signals only; no `static mut` shared state. | I |
| RRT-004 | Every task shall check in with `health` at least once per second; `health` shall stop refreshing the IWDG when a task misses its check-in for 2 s (as [DEMO] DRT-012). | T |
| RRT-005 | Stack usage shall be measured (stack painting at start-up) and reported in `Status` as the high-water mark of the main stack (as [DEMO] DRT-013, adapted: Embassy tasks share one stack). | T |
| RRT-006 | Task storage shall be static (one pool slot per task, `pool_size` only where a task runs more than once); the build shall report RAM use. | I |

---

## 6. Functional requirements

| ID | Requirement | Method |
|---|---|---|
| RFN-001 | **LEDs:** as [DEMO] LED-001…LED-007, including the patterns of [DEMO] Appendix A and the reserved use of LD3. | T, D |
| RFN-002 | **Button:** as [DEMO] BTN-001…BTN-004, using `embassy-stm32` `ExtiInput` on PC13 with 30 ms debounce in the `button` task. | T, D |
| RFN-003 | **Communication:** as [DEMO] COM-001…COM-007 (messages of [DEMO] §8.1), using the `embassy-stm32` buffered or DMA UART on USART3. | T |
| RFN-004 | **Health and confirmation:** as [DEMO] HLT-001…HLT-005, including the `fail-health` cargo feature (equivalent of `DEMO_FAIL_HEALTH`). | T |
| RFN-005 | `HelloReply` shall report role `DEMO`, firmware name `rust-app` and the Rust and Embassy versions in its version text. | T |

---

## 7. HWLINK in Rust

| ID | Requirement | Method |
|---|---|---|
| RLNK-001 | The FlatBuffers code shall be generated from the shared schema `protocol/hwlink.fbs` ([LINK] A.5) with `flatc --rust` at build time (build script), not hand-written. | I |
| RLNK-002 | The FlatBuffers runtime shall be used in `no_std` mode; message building shall use a fixed-size buffer (no heap) — see OPEN-R01. | I, T |
| RLNK-003 | COBS framing and CRC32 shall follow [LINK] A.2; CRC32 should use the hardware CRC unit through `embassy-stm32`, or a table-driven software CRC if the HAL does not support the required configuration. | T |
| RLNK-004 | Framing, CRC and message handling shall be in a separate `no_std` library crate in the same workspace, with host-side unit tests (`cargo test` on the build machine) using the shared test vectors of [LINK] A.7. | T |

---

## 8. Safety and robustness

| ID | Requirement | Method |
|---|---|---|
| RSAFE-001 | RDEMO shall never unlock or write option bytes. | I |
| RSAFE-002 | The flash write function shall accept only the `CONFIRMED` record address of the running slot (as [DEMO] DSAFE-002). | I, T |
| RSAFE-003 | RDEMO shall drive only PB0, PB7, PB14, read PC13 and use PD8/PD9 (as [DEMO] DSAFE-003). | I |
| RSAFE-004 | Integer overflow checks shall stay enabled in debug builds; release builds shall use `panic = "abort"`, `lto = true`, `codegen-units = 1`, `opt-level = "s"` unless measurement shows `3` is needed. | I |

---

## 9. Non-functional requirements

| ID | Requirement | Method |
|---|---|---|
| RNFR-001 | **Footprint:** RDEMO shall be under 128 KB of flash and 64 KB of RAM; the build shall print both (`arm-none-eabi-size`). | I |
| RNFR-002 | **Start-up:** the LED pattern shall be visible within 300 ms of hand-over. | T |
| RNFR-003 | **Reliability:** as [DEMO] DNFR-003 (24 h with the GUI polling every 100 ms). | T |
| RNFR-004 | **Comparison:** a short report shall compare RDEMO with the C demo and the Zephyr demo: flash, RAM, start-up time, button-to-LED latency, HWLINK round-trip time and lines of code. | A |
| RNFR-005 | **Template:** a README shall explain how to start a new Rust application from RDEMO. | I |

---

## 10. Verification matrix

| Area | Requirements | Main method |
|---|---|---|
| General | RGEN-001…008 | T, I |
| Bootloader contract | RBT-001…006 | T, I |
| Concurrency | RRT-001…006 | T, I |
| Functional | RFN-001…005 | T, D |
| HWLINK | RLNK-001…004 | T, I |
| Safety | RSAFE-001…004 | I, T |
| Non-functional | RNFR-001…005 | T, I, A |

---

## Appendix A — Mapping to the C demo

| C demo (FreeRTOS) | RDEMO (Embassy) |
|---|---|
| `xTaskCreate` tasks with priorities | `#[embassy_executor::task]` async tasks; interrupt executor if priority is needed |
| Queues, task notifications | `embassy_sync::channel::Channel`, `Signal` |
| `vTaskDelayUntil` | `embassy_time::Ticker` |
| HAL tick on TIM6, SysTick for the kernel | `embassy-time` driver on a TIM (feature `time-driver-tim2` or `-any`) |
| `configASSERT`, hooks | `panic` handler, `HardFault` exception |
| HAL GPIO / EXTI / UART | `embassy_stm32::gpio::Output`, `exti::ExtiInput`, `usart::BufferedUart` |
| `heap_4` | none (static allocation only) |
| `linker script` at `0x0800C000` | `memory.x` at `0x0800C000` |

## Appendix B — Open items

| ID | Item | Status |
|---|---|---|
| OPEN-R01 | Confirm the `flatbuffers` crate builds `no_std` without an allocator for building messages, or choose a fixed-buffer builder / alternative. | Open |
| OPEN-R02 | Pin the Embassy crate versions that support `stm32f767zi` with the pinned Rust version. | **Closed**: Rust 1.99.1, `embassy-stm32` 0.6.0, `embassy-executor` 0.10.0, `embassy-time` 0.5.1, locked in `Cargo.lock`. |
| OPEN-R03 | Measure whether a single thread-mode executor meets the health and comms timing, or whether an interrupt executor is needed (RRT-002). | Open |
| OPEN-R04 | Decide whether to also support `probe-rs` for flashing and RTT logging, besides OpenOCD. | Open |
