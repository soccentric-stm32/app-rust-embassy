// clock.rs - system clock tree for the NUCLEO-F767ZI
//
// Author: Sandesh Ghimire <sandesh@soccentric.com>
// SRS:    RGEN-003

/// 216 MHz from the 8 MHz ST-LINK MCO in HSE bypass mode (RGEN-003):
/// PLL M = 8, N = 432, P = 2, Q = 9; AHB /1, APB1 /4 (54 MHz), APB2 /2 (108 MHz).
pub fn clock_config() -> embassy_stm32::Config {
    use embassy_stm32::rcc::*;
    use embassy_stm32::time::Hertz;

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
