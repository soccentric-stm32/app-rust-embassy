#![allow(dead_code)]
// board.rs - board, bootloader-contract and reset-cause constants
//
// Author: Sandesh Ghimire <sandesh@soccentric.com>
// SRS:    RBT-001..006, RFN-001..005, RSAFE-001..004

pub const SLOT_BASE: u32 = 0x0800_C000;

// A-9: reserved no-init window at the top of DTCM, shared with BOOT
// (boot_layout.h BOOT_NOINIT_*).  memory.x and noinit.x place .noinit
// here; the main stack starts below it.
pub const NOINIT_ADDR: u32 = 0x2001_FC00;
pub const NOINIT_SIZE: u32 = 0x0000_0400;
pub const NOINIT_END: u32 = 0x2002_0000;
pub const META_BASE: u32 = 0x0800_8000;
pub const CONFIRMED_ADDR: u32 = META_BASE + 0x140;

pub const FLASH_BASE: u32 = 0x0800_0000;
pub const CONFIRMED_OFFSET: u32 = CONFIRMED_ADDR - FLASH_BASE;
pub const CONFIRMED_MAGIC: u32 = 0x434F_4E46; // "CONF"

pub const STAY_IN_BOOT_MAGIC: u32 = 0x5941_5453; // "STAY" in RTC_BKP1R

// Hand-over record in RTC_BKP2R, shared with the C demo's boot_contract.h:
// bits 0..23 = magic "HND", bits 24..25 = slot, bit 26 = pending,
// bits 27..28 = trial count.
pub const HANDOVER_MAGIC: u32 = 0x0044_4E48; // bytes "HND", 0
pub const HANDOVER_MAGIC_MASK: u32 = 0x00FF_FFFF;
pub const HANDOVER_SLOT_SHIFT: u32 = 24;
pub const HANDOVER_SLOT_MASK: u32 = 0x3;
pub const HANDOVER_PENDING_BIT: u32 = 26;
pub const HANDOVER_TRIAL_SHIFT: u32 = 27;
pub const HANDOVER_TRIAL_MASK: u32 = 0x3;

pub const SLOT_UNKNOWN: u8 = 0;
pub const SLOT_A: u8 = 1;
pub const SLOT_B: u8 = 2;

pub const TRIAL_UNKNOWN: u8 = 0;
pub const TRIAL_CONFIRMED: u8 = 1;
pub const TRIAL_PENDING: u8 = 2;

pub const HEALTH_NOT_REQUIRED: u8 = 0;
pub const HEALTH_RUNNING: u8 = 1;
pub const HEALTH_PASSED: u8 = 2;
pub const HEALTH_FAILED: u8 = 3;

pub const RCC_CSR: *mut u32 = 0x4002_3874 as *mut u32;
pub const RCC_CSR_RMVF: u32 = 1 << 24;
pub const RCC_CSR_BORRSTF: u32 = 1 << 25;
pub const RCC_CSR_PINRSTF: u32 = 1 << 26;
pub const RCC_CSR_PORRSTF: u32 = 1 << 27;
pub const RCC_CSR_SFTRSTF: u32 = 1 << 28;
pub const RCC_CSR_WDGRSTF: u32 = 1 << 29;
pub const RCC_CSR_WWDGRSTF: u32 = 1 << 30;
pub const RCC_CSR_LPWRRSTF: u32 = 1 << 31;
pub const RCC_CSR_RESET_MASK: u32 = RCC_CSR_BORRSTF
    | RCC_CSR_PINRSTF
    | RCC_CSR_PORRSTF
    | RCC_CSR_SFTRSTF
    | RCC_CSR_WDGRSTF
    | RCC_CSR_WWDGRSTF
    | RCC_CSR_LPWRRSTF;

pub const IWDG_KR: *mut u32 = 0x4000_3000 as *mut u32;
pub const IWDG_KEY_RELOAD: u32 = 0xAAAA;

pub const RCC_APB1ENR: *mut u32 = 0x4002_3840 as *mut u32;
pub const RCC_APB1ENR_PWREN: u32 = 1 << 28;
pub const RCC_APB1ENR_RTCEN: u32 = 1 << 10;
pub const PWR_CR: *mut u32 = 0x4000_7000 as *mut u32;
pub const PWR_CR_DBP: u32 = 1 << 8;
pub const RTC_BKP0R: *mut u32 = 0x4000_2850 as *mut u32;
pub const RTC_BKP1R: *mut u32 = 0x4000_2854 as *mut u32;
pub const RTC_BKP2R: *mut u32 = 0x4000_2858 as *mut u32;
pub const RTC_WPR: *mut u32 = 0x4000_2824 as *mut u32;
pub const RTC_WPR_UNLOCK1: u32 = 0x00CA;
pub const RTC_WPR_UNLOCK2: u32 = 0x0053;
pub const RTC_WPR_LOCK: u32 = 0xFF;

pub const DBGMCU_IDCODE: *const u32 = 0xE004_2000 as *const u32;
pub const DBGMCU_DEV_ID: u32 = 0x451;
pub const UID_BASE: *const u32 = 0x1FF0_F420 as *const u32;

pub const RCC_AHB1ENR: *mut u32 = 0x4002_3830 as *mut u32;
pub const RCC_AHB1ENR_GPIOBEN: u32 = 1 << 1;
pub const GPIOB_MODER: *mut u32 = 0x4002_0400 as *mut u32;
pub const GPIOB_BSRR: *mut u32 = 0x4002_0418 as *mut u32;
pub const LD3_PIN: u32 = 14;
