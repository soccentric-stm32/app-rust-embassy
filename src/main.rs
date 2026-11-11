// embassy-app - Embassy demo application entry point
//
// Author:  Sandesh Ghimire <sandesh@soccentric.com>
// Date:    2026-11-03
// Target:  NUCLEO-F767ZI (STM32F767ZIT6), started by BOOT from slot A or B
// SRS:     SRS-RUST-F767, SRS-DEMO-F767
//
// Four Embassy tasks: health, comms, button and led (RRT-002).  The bootloader
// contract is handled before the executor starts; the only flash write in the
// application is the CONFIRMED record in boot.rs.

#![no_std]
#![no_main]
#![deny(warnings)]

#[macro_use]
mod system;

mod app;
mod board;
mod protocol;

use embassy_executor::Spawner;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::flash::{Blocking, Flash};
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::usart::{BufferedUart, Config as UartConfig, UartTx};
use embassy_stm32::wdg::IndependentWatchdog;
use static_cell::StaticCell;

use crate::app::{button, comms, health, led};
use crate::board::{boot, clock::clock_config, fault};
use crate::system::{build_info, ipc, log};

embassy_stm32::bind_interrupts!(struct Irqs {
    USART3 => embassy_stm32::usart::BufferedInterruptHandler<embassy_stm32::peripherals::USART3>;
    EXTI15_10 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI15_10>;
    DMA2_STREAM6 => embassy_stm32::dma::InterruptHandler<embassy_stm32::peripherals::DMA2_CH6>;
});

static UART_TX: StaticCell<[u8; 4096]> = StaticCell::new();
static UART_RX: StaticCell<[u8; 4096]> = StaticCell::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // RBT-002 / APP-004: vector table and watchdog before the HAL.
    boot::set_vtor();
    boot::pet_watchdog_raw();
    let reset_cause = boot::take_reset_cause();
    // APP-004: BOOT hands over through RTC_BKP0R..BKP3R. Read the record
    // before the HAL, because embassy's RCC init may reset the backup domain.
    let handover = boot::read_handover();

    let p = embassy_stm32::init(clock_config());

    // RGEN-003: I-cache and D-cache on, as in the C demo.
    // SAFETY: nothing else owns the core peripherals in this application.
    let mut core = unsafe { cortex_m::Peripherals::steal() };
    core.SCB.enable_icache();
    core.SCB.enable_dcache(&mut core.CPUID);
    core.SCB
        .enable(cortex_m::peripheral::scb::Exception::MemoryManagement);
    core.SCB
        .enable(cortex_m::peripheral::scb::Exception::BusFault);
    core.SCB
        .enable(cortex_m::peripheral::scb::Exception::UsageFault);
    // RGEN-003: enable the flash prefetch and ART accelerator; embassy
    // leaves both off after the clock switch.
    const FLASH_ACR: *mut u32 = 0x4002_3C00 as *mut u32;
    // SAFETY: fixed F7 FLASH_ACR register, configured once at start-up.
    unsafe {
        FLASH_ACR.write_volatile(FLASH_ACR.read_volatile() | (1 << 8) | (1 << 9));
    }
    cortex_m::asm::dsb();
    cortex_m::asm::isb();

    // RRT-005: paint the unused stack before the executor starts so health
    // can report a real high-water mark in Status.
    fault::paint_stack();

    // RDBG-001: debug output on USART6 (PG14 = Arduino D1), TX only, DMA.
    let mut log_config = UartConfig::default();
    log_config.baudrate = 115_200;
    match UartTx::new(p.USART6, p.PG14, p.DMA2_CH6, Irqs, log_config) {
        Ok(tx) => match log::task(tx) {
            Ok(task) => spawner.spawn(task),
            Err(_) => fault::fatal(),
        },
        Err(_) => fault::fatal(),
    }
    log_info!(
        "start v{} {}, slot {} {}, reset {}",
        build_info::VERSION,
        build_info::GIT_COMMIT,
        match handover.slot {
            board::SLOT_A => 'A',
            board::SLOT_B => 'B',
            _ => '?',
        },
        match handover.trial_state {
            board::TRIAL_PENDING => "on trial",
            board::TRIAL_CONFIRMED => "confirmed",
            _ => "no hand-over",
        },
        boot::reset_cause_text(reset_cause)
    );

    let fault_at_start = fault::last();
    if fault_at_start.is_some() {
        log_warn!("last fault recorded before this reset");
    }
    let initial_fault = fault_at_start.is_some() || boot::reset_was_watchdog(reset_cause);
    ipc::STATUS.sender().send_modify(|s| {
        if let Some(s) = s {
            s.fault_active = initial_fault;
        }
    });

    let mut wdg = IndependentWatchdog::new(p.IWDG, 4_000_000);
    wdg.unleash();
    wdg.pet();

    let flash: Flash<'static, Blocking> = Flash::new_blocking(p.FLASH);

    let mut uart_config = UartConfig::default();
    uart_config.baudrate = 115_200;
    uart_config.eager_reads = Some(1);
    let uart = match BufferedUart::new(
        p.USART3,
        p.PD9,
        p.PD8,
        UART_TX.init([0u8; 4096]),
        UART_RX.init([0u8; 4096]),
        Irqs,
        uart_config,
    ) {
        Ok(uart) => uart,
        Err(_) => fault::fatal(),
    };

    let led1 = Output::new(p.PB0, Level::Low, Speed::Low);
    let led2 = Output::new(p.PB7, Level::Low, Speed::Low);
    let led3 = Output::new(p.PB14, Level::Low, Speed::Low);
    let btn = ExtiInput::new(p.PC13, p.EXTI13, embassy_stm32::gpio::Pull::Down, Irqs);

    let info = comms::BootInfo {
        reset_cause,
        handover,
    };

    match led::task(led1, led2, led3, initial_fault) {
        Ok(task) => spawner.spawn(task),
        Err(_) => fault::fatal(),
    }
    match button::task(btn) {
        Ok(task) => spawner.spawn(task),
        Err(_) => fault::fatal(),
    }
    match health::task(wdg, flash, handover, fault_at_start) {
        Ok(task) => spawner.spawn(task),
        Err(_) => fault::fatal(),
    }
    match comms::task(uart, info) {
        Ok(task) => spawner.spawn(task),
        Err(_) => fault::fatal(),
    }
}
