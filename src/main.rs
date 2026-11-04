// rust-app - Embassy demo application entry point
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

mod board;
mod boot;
mod build_info;
mod button;
mod comms;
mod fault;
mod health;
mod hwlink;
mod identity;
mod ipc;
mod led;

use embassy_executor::Spawner;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::flash::{Blocking, Flash};
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::time::Hertz;
use embassy_stm32::usart::{BufferedUart, Config as UartConfig};
use embassy_stm32::wdg::IndependentWatchdog;
use static_cell::StaticCell;

embassy_stm32::bind_interrupts!(struct Irqs {
    USART3 => embassy_stm32::usart::BufferedInterruptHandler<embassy_stm32::peripherals::USART3>;
    EXTI15_10 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI15_10>;
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

    let fault_at_start = fault::last();
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

/// 216 MHz from the 8 MHz ST-LINK MCO in HSE bypass mode (RGEN-003):
/// PLL M = 8, N = 432, P = 2, Q = 9; AHB /1, APB1 /4 (54 MHz), APB2 /2 (108 MHz).
fn clock_config() -> embassy_stm32::Config {
    use embassy_stm32::rcc::*;

    let mut config = embassy_stm32::Config::default();
    // APP-004: BOOT leaves BDCR at its reset value (RTCSEL=0, RTCEN=0).
    // The embassy default (LSI + RTC enabled) does not match, so embassy
    // would run a backup-domain reset and clear the BKP0R..BKP3R hand-over
    // on the first boot after power-on. Keep the domain as BOOT left it.
    config.rcc.ls = LsConfig::off();
    config.rcc.hse = Some(Hse {
        freq: Hertz(8_000_000),
        mode: HseMode::Bypass,
    });
    config.rcc.pll_src = PllSource::HSE;
    config.rcc.pll = Some(Pll {
        prediv: PllPreDiv::DIV8,
        mul: PllMul::MUL432,
        divp: Some(PllPDiv::DIV2),
        divq: Some(PllQDiv::DIV9),
        divr: None,
    });
    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV4;
    config.rcc.apb2_pre = APBPrescaler::DIV2;
    config.rcc.sys = Sysclk::PLL1_P;
    config
}
