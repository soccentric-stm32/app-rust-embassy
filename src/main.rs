// rust-app - Embassy demo application entry point
//
// Author:  Sandesh Ghimire <sandesh@soccentric.com>
// Date:    2026-11-03
// Target:  NUCLEO-F767ZI (STM32F767ZIT6), started by BOOT from slot A or B
// SRS:     RBT-001, RBT-002, RGEN-003, RGEN-004, RGEN-005, RFN-001 (LED-002)
//
// Implemented so far: VTOR, 216 MHz clock tree, caches, the heartbeat task
// on LD1, and the panic / HardFault paths. Still to come, section by
// section: health (IWDG + CONFIRMED), comms (USART3 + HWLINK), button,
// the hand-over record and the image file.

#![no_std]
#![no_main]
#![deny(warnings)]

use core::panic::PanicInfo;

use cortex_m_rt::{exception, ExceptionFrame};
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::time::Hertz;
use embassy_time::{Duration, Ticker, Timer};

/// Bootloader slot start, where this image's vector table is linked (RBT-001).
const SLOT_BASE: u32 = 0x0800_C000;

// HEARTBEAT ([DEMO] Appendix A): two 50 ms flashes 200 ms apart in a 1 s period.
const HEARTBEAT_PERIOD: Duration = Duration::from_millis(1000);
const HEARTBEAT_FLASH: Duration = Duration::from_millis(50);
const HEARTBEAT_GAP: Duration = Duration::from_millis(150);

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // RBT-002: vector table first, before the HAL enables any interrupt.
    // SAFETY: single write to VTOR during start-up; the table at SLOT_BASE is
    // this image's own, placed there by memory.x.
    unsafe { (*cortex_m::peripheral::SCB::PTR).vtor.write(SLOT_BASE) };

    let p = embassy_stm32::init(clock_config());

    // RGEN-003: I-cache and D-cache on, as in the C demo. No DMA buffers yet;
    // the comms task must place them in a non-cacheable region or clean the cache.
    // SAFETY: nothing else owns the core peripherals in this application.
    let mut core = unsafe { cortex_m::Peripherals::steal() };
    core.SCB.enable_icache();
    core.SCB.enable_dcache(&mut core.CPUID);

    let led1 = Output::new(p.PB0, Level::Low, Speed::Low);
    match heartbeat(led1) {
        Ok(task) => spawner.spawn(task),
        Err(_) => fatal(), // task pool exhausted: a programming error
    }
}

/// 216 MHz from the 8 MHz ST-LINK MCO in HSE bypass mode (RGEN-003):
/// PLL M = 8, N = 432, P = 2, Q = 9; AHB /1, APB1 /4 (54 MHz), APB2 /2 (108 MHz).
fn clock_config() -> embassy_stm32::Config {
    use embassy_stm32::rcc::*;

    let mut config = embassy_stm32::Config::default();
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

/// LED-002: default HEARTBEAT pattern on LD1, anchored to a 1 s ticker (LED-006).
#[embassy_executor::task]
async fn heartbeat(mut led: Output<'static>) {
    let mut ticker = Ticker::every(HEARTBEAT_PERIOD);
    loop {
        led.set_high();
        Timer::after(HEARTBEAT_FLASH).await;
        led.set_low();
        Timer::after(HEARTBEAT_GAP).await;
        led.set_high();
        Timer::after(HEARTBEAT_FLASH).await;
        led.set_low();
        ticker.next().await;
    }
}

// ---------------------------------------------------------------- fault paths

const RCC_AHB1ENR: *mut u32 = 0x4002_3830 as *mut u32;
const GPIOB_MODER: *mut u32 = 0x4002_0400 as *mut u32;
const GPIOB_BSRR: *mut u32 = 0x4002_0418 as *mut u32;
const LD3_PIN: u32 = 14;

/// LD3 on for about 2 s, then reset. Register-level so it works from any
/// context, including before the HAL is up (RGEN-004/005).
fn fatal() -> ! {
    cortex_m::interrupt::disable();
    // SAFETY: fixed RCC/GPIOB register addresses of the STM32F767 (RM0410);
    // interrupts are off, so nothing else touches them concurrently.
    unsafe {
        RCC_AHB1ENR.write_volatile(RCC_AHB1ENR.read_volatile() | (1 << 1)); // GPIOBEN
        let moder = GPIOB_MODER.read_volatile() & !(0b11 << (LD3_PIN * 2));
        GPIOB_MODER.write_volatile(moder | (0b01 << (LD3_PIN * 2)));
        GPIOB_BSRR.write_volatile(1 << LD3_PIN);
    }
    // Roughly 2 s at 216 MHz; exact timing does not matter here.
    cortex_m::asm::delay(432_000_000);
    cortex_m::peripheral::SCB::sys_reset();
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    // RGEN-004: saving the location to a no-init area comes with the comms task.
    fatal()
}

#[exception]
unsafe fn HardFault(_frame: &ExceptionFrame) -> ! {
    fatal()
}
