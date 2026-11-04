// fault.rs - panic, HardFault and no-init fault record
//
// Author: Sandesh Ghimire <sandesh@soccentric.com>
// SRS:    RGEN-004, RGEN-005, RRT-005; DSAFE / LED-005 fault indication

use core::arch::naked_asm;
use core::panic::PanicInfo;
use core::ptr::{addr_of, addr_of_mut, read_volatile, write_volatile};

use cortex_m::asm;
use cortex_m_rt::{exception, ExceptionFrame};

use crate::board;

pub const MAGIC: u32 = 0x4641_554C; // "FAUL"

pub const PANIC: u32 = 1;
pub const HARD_FAULT: u32 = 2;
pub const MEM_MANAGE: u32 = 3;
pub const BUS_FAULT: u32 = 4;
pub const USAGE_FAULT: u32 = 5;

pub const FILE_NAME_MAX: usize = 40;
pub const MESSAGE_MAX: usize = 64;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct FaultRecord {
    pub magic: u32,
    pub kind: u32,
    pub pc: u32,
    pub lr: u32,
    pub cfsr: u32,
    pub hfsr: u32,
    pub mmfar: u32,
    pub bfar: u32,
    pub line: u32,
    pub file: [u8; FILE_NAME_MAX],
    pub file_len: u32,
    pub message: [u8; MESSAGE_MAX],
    pub message_len: u32,
}

const EMPTY: FaultRecord = FaultRecord {
    magic: 0,
    kind: 0,
    pc: 0,
    lr: 0,
    cfsr: 0,
    hfsr: 0,
    mmfar: 0,
    bfar: 0,
    line: 0,
    file: [0; FILE_NAME_MAX],
    file_len: 0,
    message: [0; MESSAGE_MAX],
    message_len: 0,
};

// No-init RAM: survives a system reset, is not cleared by cortex-m-rt.
#[link_section = ".noinit.fault"]
static mut RECORD: FaultRecord = EMPTY;

pub fn last() -> Option<FaultRecord> {
    // SAFETY: reads the no-init record by volatile copy; the linker places it
    // in the reserved no-init window and nothing else writes it except the
    // fault paths.
    let record = unsafe { read_volatile(addr_of!(RECORD)) };
    if record.magic == MAGIC {
        Some(record)
    } else {
        None
    }
}

pub fn clear() {
    // Invalidate the magic first, then clear the rest of the record.
    // SAFETY: sole write to the no-init fault record; interrupts are not
    // required because this is called from the comms task.
    unsafe { (addr_of_mut!(RECORD) as *mut u32).write_volatile(0) };
    asm::dmb();
    // SAFETY: same record; EMPTY has magic 0 as its first word.
    unsafe { write_volatile(addr_of_mut!(RECORD), EMPTY) };
    asm::dsb();
}

fn save(mut record: FaultRecord) {
    // Invalidate the magic first so a reset in the middle of the copy is not
    // mistaken for a complete record (the C demo does the same).
    record.magic = 0;
    // SAFETY: sole write to the no-init fault record.  The volatile write is
    // intentional: it must survive the reset that follows.
    unsafe { write_volatile(addr_of_mut!(RECORD), record) };
    asm::dsb();
    // SAFETY: magic is the first word of the repr(C) record; publish it last.
    unsafe { (addr_of_mut!(RECORD) as *mut u32).write_volatile(MAGIC) };
    asm::dsb();
}

fn scb_reg(address: usize) -> u32 {
    // SAFETY: fixed Cortex-M7 System Control Block registers.
    unsafe { (address as *const u32).read_volatile() }
}

#[cfg(feature = "panic-messages")]
struct MessageBuffer<'a> {
    bytes: &'a mut [u8],
    len: usize,
}

#[cfg(feature = "panic-messages")]
impl core::fmt::Write for MessageBuffer<'_> {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        let n = text.len().min(self.bytes.len().saturating_sub(self.len));
        self.bytes[self.len..self.len + n].copy_from_slice(&text.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}

#[cfg(feature = "panic-messages")]
fn panic_message(info: &PanicInfo<'_>) -> ([u8; MESSAGE_MAX], u32) {
    let mut message = [0u8; MESSAGE_MAX];
    let len = {
        let mut buffer = MessageBuffer {
            bytes: &mut message,
            len: 0,
        };
        let _ = core::fmt::write(&mut buffer, format_args!("{}", info.message()));
        buffer.len
    };
    (message, len as u32)
}

pub fn record_panic(info: &PanicInfo<'_>) {
    let mut file = [0u8; FILE_NAME_MAX];
    let mut file_len = 0u32;
    let line = match info.location() {
        Some(l) => {
            let bytes = l.file().as_bytes();
            let n = bytes.len().min(FILE_NAME_MAX);
            file[..n].copy_from_slice(&bytes[..n]);
            file_len = n as u32;
            l.line()
        }
        None => 0,
    };
    #[cfg(feature = "panic-messages")]
    let (message, message_len) = panic_message(info);
    #[cfg(not(feature = "panic-messages"))]
    let (message, message_len) = ([0u8; MESSAGE_MAX], 0u32);
    save(FaultRecord {
        magic: MAGIC,
        kind: PANIC,
        pc: 0,
        lr: 0,
        cfsr: scb_reg(0xE000_ED28),
        hfsr: scb_reg(0xE000_ED2C),
        mmfar: scb_reg(0xE000_ED34),
        bfar: scb_reg(0xE000_ED38),
        line,
        file,
        file_len,
        message,
        message_len,
    });
}

pub fn record_exception(kind: u32, frame: &ExceptionFrame) {
    save(FaultRecord {
        magic: MAGIC,
        kind,
        pc: frame.pc(),
        lr: frame.lr(),
        cfsr: scb_reg(0xE000_ED28),
        hfsr: scb_reg(0xE000_ED2C),
        mmfar: scb_reg(0xE000_ED34),
        bfar: scb_reg(0xE000_ED38),
        line: 0,
        file: [0; FILE_NAME_MAX],
        file_len: 0,
        message: [0; MESSAGE_MAX],
        message_len: 0,
    });
}

fn record_exception_raw(kind: u32, pc: u32, lr: u32) {
    save(FaultRecord {
        magic: MAGIC,
        kind,
        pc,
        lr,
        cfsr: scb_reg(0xE000_ED28),
        hfsr: scb_reg(0xE000_ED2C),
        mmfar: scb_reg(0xE000_ED34),
        bfar: scb_reg(0xE000_ED38),
        line: 0,
        file: [0; FILE_NAME_MAX],
        file_len: 0,
        message: [0; MESSAGE_MAX],
        message_len: 0,
    });
}

#[allow(dead_code)]
pub fn describe(record: &FaultRecord) -> &'static str {
    match record.kind {
        PANIC => "panic",
        HARD_FAULT => "hardfault",
        MEM_MANAGE => "memmanage",
        BUS_FAULT => "busfault",
        USAGE_FAULT => "usagefault",
        _ => "fault",
    }
}

// ---------------------------------------------------------------- stack use

const STACK_PAINT: u32 = 0xC0DE_5A5A;
const STACK_GUARD: usize = 128;

extern "C" {
    static _stack_start: u32;
    static _stack_end: u32;
}

/// RRT-005: paint the unused part of the shared Embassy stack at start-up.
/// The health task later scans for the first untouched word to report the
/// high-water mark in `Status`.  The 128-byte guard below the current MSP is
/// left unpainted so the paint loop itself (and a late exception) is safe.
pub fn paint_stack() {
    let top = addr_of!(_stack_start) as usize;
    let bottom = addr_of!(_stack_end) as usize;
    if bottom >= top {
        return;
    }
    let sp = cortex_m::register::msp::read() as usize;
    let limit = sp.saturating_sub(STACK_GUARD).max(bottom);
    let mut address = (bottom + 3) & !3;
    while address + 4 <= limit {
        // SAFETY: the range is inside the linker-provided stack area and
        // below the current stack pointer, so it is unused at this point.
        unsafe { write_volatile(address as *mut u32, STACK_PAINT) };
        address += 4;
    }
}

/// RRT-005: high-water mark of the main stack in bytes.
pub fn stack_high_water() -> u32 {
    let top = addr_of!(_stack_start) as usize;
    let bottom = addr_of!(_stack_end) as usize;
    if bottom >= top {
        return 0;
    }

    let mut address = (bottom + 3) & !3;
    while address + 4 <= top {
        // SAFETY: reads the linker-provided stack area; the paint pattern was
        // written by paint_stack and any non-painted word is live stack.
        let word = unsafe { read_volatile(address as *const u32) };
        if word != STACK_PAINT {
            return top.saturating_sub(address) as u32;
        }
        address += 4;
    }
    0
}

// ---------------------------------------------------------------- fault LEDs

fn led3_init() {
    // SAFETY: fixed RCC/GPIOB registers (RM0410); interrupts are disabled by
    // fatal() so nothing else touches them.
    unsafe {
        board::RCC_AHB1ENR
            .write_volatile(board::RCC_AHB1ENR.read_volatile() | board::RCC_AHB1ENR_GPIOBEN);
        let moder = board::GPIOB_MODER.read_volatile() & !(0b11 << (board::LD3_PIN * 2));
        board::GPIOB_MODER.write_volatile(moder | (0b01 << (board::LD3_PIN * 2)));
    }
}

fn led3(on: bool) {
    // SAFETY: GPIOB BSRR write; the pin was configured as an output above.
    unsafe {
        if on {
            board::GPIOB_BSRR.write_volatile(1 << board::LD3_PIN);
        } else {
            board::GPIOB_BSRR.write_volatile(1 << (board::LD3_PIN + 16));
        }
    }
}

/// Fault pattern of [DEMO] Appendix A: three short flashes, pause, repeat.
/// Reset after roughly two seconds (RGEN-004/005).
pub fn fatal() -> ! {
    cortex_m::interrupt::disable();
    led3_init();

    for _ in 0..2 {
        for _ in 0..3 {
            led3(true);
            asm::delay(22_000_000); // ~100 ms at 216 MHz
            led3(false);
            asm::delay(22_000_000);
        }
        asm::delay(108_000_000); // ~500 ms pause
    }

    asm::dsb();
    cortex_m::peripheral::SCB::sys_reset()
}

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    record_panic(info);
    fatal()
}

#[exception]
unsafe fn HardFault(frame: &ExceptionFrame) -> ! {
    record_exception(HARD_FAULT, frame);
    fatal()
}

// MemManage, BusFault and UsageFault are separate exceptions on the Cortex-M7
// (SHCSR enables them in main).  cortex-m-rt only passes the exception frame
// to HardFault, so the other three use the same naked trampoline as the C
// demo: LR bit 2 selects MSP or PSP, and the stacked PC/LR are r0[6]/r0[5].
#[unsafe(no_mangle)]
pub extern "C" fn fault_handler_c(frame: *const u32, kind: u32) -> ! {
    // SAFETY: the naked trampolines pass the hardware exception frame, which
    // contains at least the eight words through xPSR (r0..r3, r12, lr, pc).
    let pc = unsafe { read_volatile(frame.add(6)) };
    let lr = unsafe { read_volatile(frame.add(5)) };
    record_exception_raw(kind, pc, lr);
    fatal()
}

macro_rules! fault_trampoline {
    ($name:ident, $symbol:literal, $kind:expr) => {
        #[unsafe(naked)]
        #[unsafe(export_name = $symbol)]
        unsafe extern "C" fn $name() -> ! {
            naked_asm!(
                "tst lr, #4",
                "ite eq",
                "mrseq r0, msp",
                "mrsne r0, psp",
                "movs r1, {kind}",
                "b {handler}",
                kind = const $kind,
                handler = sym fault_handler_c,
            );
        }
    };
}

fault_trampoline!(memory_management, "MemoryManagement", MEM_MANAGE);
fault_trampoline!(bus_fault, "BusFault", BUS_FAULT);
fault_trampoline!(usage_fault, "UsageFault", USAGE_FAULT);
