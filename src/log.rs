// log.rs - debug lines on USART6 (RDBG-001..RDBG-004)
//
// Author: Sandesh Ghimire <sandesh@soccentric.com>
// SRS:    RDBG-001..RDBG-004
//
// USART3 carries HWLINK only, so debug text goes to USART6 (PG14, Arduino
// D1, 115200 8N1, TX only) in the line format BOOT and the other firmwares
// use:
//
//     [   5012] R-EMB I health: passed, CONFIRMED written
//
// RDBG-002: the macros format into a stack buffer and push the line into a
// 1 KB pipe without waiting; a full pipe drops the line and counts it.  The
// log task drains the pipe with DMA, so no task waits on the UART.
// RDBG-004: flush_blocking() drains what is left by polling USART6 before a
// deliberate reset, when the executor no longer runs.

use core::fmt::{self, Write};
use core::sync::atomic::{AtomicU32, Ordering};

use embassy_stm32::mode::Async;
use embassy_stm32::usart::UartTx;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pipe::Pipe;
use embassy_time::Instant;

const TAG: &str = "R-EMB";
const LINE_MAX: usize = 128;

static PIPE: Pipe<CriticalSectionRawMutex, 1024> = Pipe::new();
static DROPPED: AtomicU32 = AtomicU32::new(0);

struct Line {
    buf: [u8; LINE_MAX],
    len: usize,
}

impl Write for Line {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        // Keep two bytes for CR LF.
        let take = s.len().min(LINE_MAX - 2 - self.len);
        self.buf[self.len..self.len + take].copy_from_slice(&s.as_bytes()[..take]);
        self.len += take;
        Ok(())
    }
}

pub fn write(level: char, args: fmt::Arguments) {
    let mut line = Line {
        buf: [0; LINE_MAX],
        len: 0,
    };
    let ms = Instant::now().as_millis() as u32;
    let _ = write!(line, "[{:7}] {} {} ", ms, TAG, level);
    let _ = line.write_fmt(args);
    line.buf[line.len] = b'\r';
    line.buf[line.len + 1] = b'\n';
    let bytes = &line.buf[..line.len + 2];
    // All writers run on the one executor thread, so nothing can fill the
    // pipe between the check and the write.
    if PIPE.free_capacity() < bytes.len() {
        DROPPED.fetch_add(1, Ordering::Relaxed);
        return;
    }
    // try_write() stops at the end of the ring, so a line that straddles the
    // wrap point needs a second call for its tail.  The capacity was checked
    // above, so this always finishes.
    let mut rest = bytes;
    while !rest.is_empty() {
        match PIPE.try_write(rest) {
            Ok(n) => rest = &rest[n..],
            Err(_) => {
                DROPPED.fetch_add(1, Ordering::Relaxed);
                break;
            }
        }
    }
}

/// Once-a-second "task <n> <name>: <purpose>" line for one task.  Each task
/// owns one and calls `tick()` once per loop iteration; it prints at most once
/// a second however often the loop wakes.
pub struct TaskReport {
    number: u8,
    name: &'static str,
    purpose: &'static str,
    last: Option<Instant>,
}

impl TaskReport {
    pub const fn new(number: u8, name: &'static str, purpose: &'static str) -> Self {
        Self {
            number,
            name,
            purpose,
            last: None,
        }
    }

    pub fn tick(&mut self) {
        let now = Instant::now();
        let due = match self.last {
            Some(last) => now.duration_since(last).as_millis() >= 1000,
            None => true,
        };
        if due {
            self.last = Some(now);
            write(
                'I',
                format_args!("task {} {}: {}", self.number, self.name, self.purpose),
            );
        }
    }
}

/// Lines dropped because the pipe was full (RDBG-003).
#[allow(dead_code)]
pub fn dropped() -> u32 {
    DROPPED.load(Ordering::Relaxed)
}

#[embassy_executor::task]
pub async fn task(mut tx: UartTx<'static, Async>) {
    let mut chunk = [0u8; 64];
    loop {
        let n = PIPE.read(&mut chunk).await;
        let _ = tx.write(&chunk[..n]).await;
    }
}

// USART6 registers for flush_blocking() (RM0410 section 34.8).
const USART6_ISR: *const u32 = 0x4001_141C as *const u32;
const USART6_TDR: *mut u32 = 0x4001_1428 as *mut u32;
const ISR_TC: u32 = 1 << 6;
const ISR_TXE: u32 = 1 << 7;
/// Upper bound for one register wait, about 1 ms at 216 MHz.
const WAIT_LOOPS: u32 = 200_000;

fn wait_flag(flag: u32) {
    for _ in 0..WAIT_LOOPS {
        // SAFETY: fixed, always-mapped USART6 status register; read only.
        if unsafe { USART6_ISR.read_volatile() } & flag != 0 {
            return;
        }
    }
}

/// RDBG-004: write out whatever is still queued, by polling.  Called just
/// before a reset; a DMA chunk in flight finishes first.
pub fn flush_blocking() {
    wait_flag(ISR_TC);
    let mut chunk = [0u8; 64];
    while let Ok(n) = PIPE.try_read(&mut chunk) {
        for &b in &chunk[..n] {
            wait_flag(ISR_TXE);
            // SAFETY: fixed USART6 data register; the log task is not
            // running (we are about to reset).
            unsafe { USART6_TDR.write_volatile(u32::from(b)) };
        }
    }
    wait_flag(ISR_TC);
}

macro_rules! log_error {
    ($($arg:tt)*) => { $crate::log::write('E', format_args!($($arg)*)) };
}

macro_rules! log_warn {
    ($($arg:tt)*) => { $crate::log::write('W', format_args!($($arg)*)) };
}

macro_rules! log_info {
    ($($arg:tt)*) => { $crate::log::write('I', format_args!($($arg)*)) };
}
