#![allow(dead_code)]
// hwlink/mod.rs - HWLINK demo message set
//
// Author: Sandesh Ghimire <sandesh@soccentric.com>
// SRS:    RFN-003, RFN-005, RLNK-001..004; LINK A.3
//
// The schema is the shared protocol/hwlink.fbs.  See fb.rs for why the
// FlatBuffers encoder is local and fixed-buffer (OPEN-R01).

pub mod fb;
pub mod frame;

use fb::{Builder, Table, Value};

pub const PROTOCOL_MINOR: u16 = 1;
pub const MAX_FRAME: usize = frame::MAX_DECODED;

// Union tags: the order is the wire format (LINK-013).
pub mod body {
    pub const NONE: u8 = 0;
    pub const HELLO: u8 = 1;
    pub const HELLO_REPLY: u8 = 2;
    pub const PING: u8 = 3;
    pub const PONG: u8 = 4;
    pub const SET_BAUD: u8 = 5;
    pub const ACK: u8 = 6;
    pub const NAK: u8 = 7;
    pub const LOG: u8 = 8;
    pub const PROGRESS: u8 = 9;
    pub const RESET: u8 = 10;
    pub const GET_BOOT_INFO: u8 = 11;
    pub const BOOT_INFO: u8 = 12;
    pub const UPDATE_BEGIN: u8 = 13;
    pub const UPDATE_DATA: u8 = 14;
    pub const UPDATE_END: u8 = 15;
    pub const UPDATE_RESULT: u8 = 16;
    pub const ACTIVATE: u8 = 17;
    pub const UPDATE_ABORT: u8 = 18;
    pub const SET_LED: u8 = 19;
    pub const SET_LED_ALL: u8 = 20;
    pub const CLEAR_FAULT: u8 = 21;
    pub const ENTER_BOOTLOADER: u8 = 22;
    pub const GET_STATUS: u8 = 23;
    pub const STATUS: u8 = 24;
    pub const BUTTON_EVENT: u8 = 25;
}

pub mod role {
    pub const HWVAL: u8 = 1;
    pub const BOOTLOADER: u8 = 2;
    pub const DEMO: u8 = 3;
}

pub mod flash_mode {
    pub const UNKNOWN: u8 = 0;
    pub const SINGLE_BANK: u8 = 1;
    pub const DUAL_BANK: u8 = 2;
}

pub mod slot {
    pub const UNKNOWN: u8 = 0;
    pub const A: u8 = 1;
    pub const B: u8 = 2;
}

pub mod trial {
    pub const UNKNOWN: u8 = 0;
    pub const CONFIRMED: u8 = 1;
    pub const PENDING: u8 = 2;
}

pub mod health {
    pub const NOT_REQUIRED: u8 = 0;
    pub const RUNNING: u8 = 1;
    pub const PASSED: u8 = 2;
    pub const FAILED: u8 = 3;
}

pub mod button {
    pub const SHORT: u8 = 1;
    pub const LONG: u8 = 2;
}

pub mod pattern {
    pub const OFF: u8 = 0;
    pub const ON: u8 = 1;
    pub const BLINK: u8 = 2;
    pub const HEARTBEAT: u8 = 3;
    pub const FAST: u8 = 4;
    pub const CHASE: u8 = 5;
    pub const MORSE_SOS: u8 = 6;

    pub const COUNT: u8 = 7;

    pub fn valid(v: u8) -> bool {
        v < COUNT
    }
}

pub mod led {
    pub const LD1: u8 = 1;
    pub const LD2: u8 = 2;
    pub const LD3: u8 = 3;
}

pub mod error {
    pub const NONE: u16 = 0;
    pub const MALFORMED: u16 = 1;
    pub const UNSUPPORTED: u16 = 2;
    pub const BAD_ARGUMENT: u16 = 3;
    pub const BAD_STATE: u16 = 4;
    pub const BUSY: u16 = 5;
    pub const NOT_FOUND: u16 = 6;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    Malformed,
    Unsupported,
}

pub enum Request<'a> {
    Hello {
        host_name: Option<&'a str>,
        host_version: Option<&'a str>,
        protocol_minor: u16,
    },
    Ping,
    GetStatus,
    SetLed {
        led: u8,
        pattern: u8,
        period_ms: u16,
        force: bool,
    },
    SetLedAll {
        pattern: u8,
        period_ms: u16,
    },
    ClearFault,
    EnterBootloader,
    Reset,
    SetBaud {
        rate: u32,
    },
    Unknown,
}

fn required_u8(t: Table<'_>, id: u8) -> Result<u8, ParseError> {
    t.u8(id).ok_or(ParseError::Malformed)
}

pub fn parse_request(payload: &[u8]) -> Result<Request<'_>, ParseError> {
    let msg = Table::root(payload, fb::FILE_ID).ok_or(ParseError::Malformed)?;
    let body_type = required_u8(msg, 0)?;
    let body = if body_type == body::NONE {
        None
    } else {
        Some(msg.table(1).ok_or(ParseError::Malformed)?)
    };

    Ok(match body_type {
        body::HELLO => {
            let t = body.ok_or(ParseError::Malformed)?;
            Request::Hello {
                host_name: t.str(0),
                host_version: t.str(1),
                protocol_minor: t.u16(2).unwrap_or(0),
            }
        }
        body::PING => Request::Ping,
        body::GET_STATUS => Request::GetStatus,
        body::SET_LED => {
            let t = body.ok_or(ParseError::Malformed)?;
            // FlatBuffers omits scalar fields equal to their default.  The
            // generated C reader returns LED1/OFF/0 in that case, so do the
            // same and let the argument checks answer BAD_ARGUMENT.
            Request::SetLed {
                led: t.u8(0).unwrap_or(led::LD1),
                pattern: t.u8(1).unwrap_or(pattern::OFF),
                period_ms: t.u16(2).unwrap_or(0),
                force: t.u8(3).unwrap_or(0) != 0,
            }
        }
        body::SET_LED_ALL => {
            let t = body.ok_or(ParseError::Malformed)?;
            Request::SetLedAll {
                pattern: t.u8(0).unwrap_or(pattern::OFF),
                period_ms: t.u16(1).unwrap_or(0),
            }
        }
        body::CLEAR_FAULT => Request::ClearFault,
        body::ENTER_BOOTLOADER => Request::EnterBootloader,
        body::RESET => Request::Reset,
        body::SET_BAUD => Request::SetBaud {
            rate: body.ok_or(ParseError::Malformed)?.u32(0).unwrap_or(0),
        },
        _ => Request::Unknown,
    })
}

pub struct HelloReply<'a> {
    pub firmware_name: &'a str,
    pub version: &'a str,
    pub commit: &'a str,
    pub build_time: &'a str,
    pub uid: u64,
    pub dev_id: u32,
    pub rev_id: u32,
    pub flash_mode: u8,
    pub detail: &'a str,
}

pub fn hello_reply<'a>(buf: &'a mut [u8], info: &HelloReply<'_>) -> Option<&'a [u8]> {
    let mut b = Builder::new(buf);
    let name = b.string(info.firmware_name);
    let version = b.string(info.version);
    let commit = b.string(info.commit);
    let build_time = b.string(info.build_time);
    let detail = b.string(info.detail);
    let t = b.table(&[
        (0, Value::U8(role::DEMO)),
        (1, Value::Offset(name)),
        (2, Value::Offset(version)),
        (3, Value::Offset(commit)),
        (4, Value::Offset(build_time)),
        (5, Value::U64(info.uid)),
        (6, Value::U32(info.dev_id)),
        (7, Value::U32(info.rev_id)),
        (8, Value::U8(info.flash_mode)),
        (9, Value::U16(frame::PROTO_MAJOR as u16)),
        (10, Value::U16(PROTOCOL_MINOR)),
        (11, Value::U16(MAX_FRAME as u16)),
        (12, Value::Offset(detail)),
    ]);
    let m = b.message(body::HELLO_REPLY, t);
    Some(b.finish(m))
}

pub fn pong(buf: &mut [u8], uptime_ms: u64) -> Option<&[u8]> {
    let mut b = Builder::new(buf);
    let t = b.table(&[(0, Value::U64(uptime_ms))]);
    let m = b.message(body::PONG, t);
    Some(b.finish(m))
}

pub struct FaultInfoData<'a> {
    pub present: bool,
    pub reason: &'a str,
    pub cfsr: u32,
    pub hfsr: u32,
    pub mmfar: u32,
    pub bfar: u32,
    pub pc: u32,
    pub lr: u32,
    pub task: &'a str,
    pub file: &'a str,
    pub line: u32,
}

pub struct LedStateData {
    pub pattern: u8,
    pub period_ms: u16,
}

pub struct TaskStatsData<'a> {
    pub name: &'a str,
    pub priority: u8,
    pub state: u8,
    pub stack_high_water: u16,
}

pub struct Status<'a> {
    pub firmware_name: &'a str,
    pub version: &'a str,
    pub commit: &'a str,
    pub build_time: &'a str,
    pub uid: u64,
    pub uptime_ms: u64,
    pub slot: u8,
    pub trial: u8,
    pub trial_count: u8,
    pub health: u8,
    pub reset_cause: u32,
    pub reset_cause_text: &'a str,
    pub fault: Option<FaultInfoData<'a>>,
    pub leds: [LedStateData; 3],
    pub tasks: &'a [TaskStatsData<'a>],
    pub heap_free: u32,
    pub heap_min_free: u32,
}

pub fn status<'a>(buf: &'a mut [u8], s: &Status<'_>) -> Option<&'a [u8]> {
    let mut b = Builder::new(buf);

    let firmware_name = b.string(s.firmware_name);
    let version = b.string(s.version);
    let commit = b.string(s.commit);
    let build_time = b.string(s.build_time);
    let reset_cause_text = b.string(s.reset_cause_text);

    let fault = s.fault.as_ref().map(|f| {
        let reason = b.string(f.reason);
        let task = b.string(f.task);
        let file = b.string(f.file);
        b.table(&[
            (0, Value::U8(f.present as u8)),
            (1, Value::Offset(reason)),
            (2, Value::U32(f.cfsr)),
            (3, Value::U32(f.hfsr)),
            (4, Value::U32(f.mmfar)),
            (5, Value::U32(f.bfar)),
            (6, Value::U32(f.pc)),
            (7, Value::U32(f.lr)),
            (8, Value::Offset(task)),
            (9, Value::Offset(file)),
            (10, Value::U32(f.line)),
        ])
    });

    let mut led_refs = [0usize; 3];
    for (i, led) in s.leds.iter().enumerate() {
        led_refs[i] = b.table(&[
            (0, Value::U8(led::LD1 + i as u8)),
            (1, Value::U8(led.pattern)),
            (2, Value::U16(led.period_ms)),
        ]);
    }
    let leds = b.offset_vector(&led_refs);

    let mut task_refs = [0usize; 8];
    for (i, task) in s.tasks.iter().enumerate().take(task_refs.len()) {
        let name = b.string(task.name);
        task_refs[i] = b.table(&[
            (0, Value::Offset(name)),
            (1, Value::U8(task.priority)),
            (2, Value::U8(task.state)),
            (3, Value::U16(task.stack_high_water)),
        ]);
    }
    let tasks = b.offset_vector(&task_refs[..s.tasks.len().min(task_refs.len())]);

    let mut fields = [(0u8, Value::U8(0)); 19];
    let mut n = 0usize;
    fields[n] = (0, Value::Offset(firmware_name));
    n += 1;
    fields[n] = (1, Value::Offset(version));
    n += 1;
    fields[n] = (2, Value::Offset(commit));
    n += 1;
    fields[n] = (3, Value::Offset(build_time));
    n += 1;
    fields[n] = (4, Value::U64(s.uid));
    n += 1;
    fields[n] = (5, Value::U64(s.uptime_ms));
    n += 1;
    fields[n] = (6, Value::U8(s.slot));
    n += 1;
    fields[n] = (7, Value::U8(s.trial));
    n += 1;
    fields[n] = (8, Value::U8(s.trial_count));
    n += 1;
    fields[n] = (9, Value::U8(s.health));
    n += 1;
    fields[n] = (10, Value::U32(s.reset_cause));
    n += 1;
    fields[n] = (11, Value::Offset(reset_cause_text));
    n += 1;
    if let Some(fault) = fault {
        fields[n] = (12, Value::Offset(fault));
        n += 1;
    }
    fields[n] = (13, Value::Offset(leds));
    n += 1;
    fields[n] = (14, Value::Offset(tasks));
    n += 1;
    fields[n] = (15, Value::U32(s.heap_free));
    n += 1;
    fields[n] = (16, Value::U32(s.heap_min_free));
    n += 1;
    fields[n] = (17, Value::U16(frame::PROTO_MAJOR as u16));
    n += 1;
    fields[n] = (18, Value::U16(PROTOCOL_MINOR));
    n += 1;

    let t = b.table(&fields[..n]);
    let m = b.message(body::STATUS, t);
    Some(b.finish(m))
}

pub fn ack(buf: &mut [u8], seq: u16) -> Option<&[u8]> {
    let mut b = Builder::new(buf);
    let t = b.table(&[(0, Value::U16(seq)), (1, Value::U32(0))]);
    let m = b.message(body::ACK, t);
    Some(b.finish(m))
}

pub fn nak<'a>(buf: &'a mut [u8], seq: u16, code: u16, detail: &str) -> Option<&'a [u8]> {
    let mut b = Builder::new(buf);
    let detail = b.string(detail);
    let t = b.table(&[
        (0, Value::U16(seq)),
        (1, Value::U16(code)),
        (2, Value::Offset(detail)),
        (3, Value::U32(0)),
    ]);
    let m = b.message(body::NAK, t);
    Some(b.finish(m))
}

pub fn button_event(buf: &mut [u8], press: u8, pattern: u8, period_ms: u16) -> Option<&[u8]> {
    let mut b = Builder::new(buf);
    let t = b.table(&[
        (0, Value::U8(press)),
        (1, Value::U8(pattern)),
        (2, Value::U16(period_ms)),
    ]);
    let m = b.message(body::BUTTON_EVENT, t);
    Some(b.finish(m))
}
