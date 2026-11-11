// comms.rs - USART3 HWLINK task and demo message handling
//
// Author: Sandesh Ghimire <sandesh@soccentric.com>
// SRS:    RFN-003, RFN-005; RLNK-001..004; COM-001..COM-007

use embassy_futures::select::{select, Either};
use embassy_stm32::usart::{BufferedUart, BufferedUartRx, BufferedUartTx};
use embassy_time::{Duration, Instant, Timer};
use embedded_io_async::{Read, Write};

use crate::boot::{self, Handover};
use crate::build_info;
use crate::fault::{self};
use crate::hwlink::frame::{self, FLAG_EVENT, FLAG_RESPONSE, MAX_DECODED, MAX_ENCODED};
use crate::hwlink::{self, error, led, pattern, ParseError, Request};
use crate::identity;
use crate::ipc::{self, ButtonEvent, LedCommand, StatusSnapshot};

const PAYLOAD_MAX: usize = 1024;
const BAUD_REVERT_MS: u64 = 5000;
const IDLE_CHECKIN_MS: u64 = 500;

#[derive(Clone, Copy)]
pub struct BootInfo {
    pub reset_cause: u32,
    pub handover: Handover,
}

type StatusReceiver<'a> = embassy_sync::watch::Receiver<
    'a,
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    StatusSnapshot,
    2,
>;

#[derive(Clone, Copy)]
enum Action {
    None,
    Hello,
    Reset,
    EnterBootloader,
    SetBaud(u32),
}

struct LastResponse {
    seq: u16,
    len: usize,
    buf: [u8; MAX_ENCODED],
}

impl LastResponse {
    fn new() -> Self {
        Self {
            seq: 0,
            len: 0,
            buf: [0; MAX_ENCODED],
        }
    }
}

fn valid_led_period(period_ms: u16) -> bool {
    (50..=5000).contains(&period_ms) && period_ms.is_multiple_of(10)
}

fn touch_baud(revert_at: &mut Option<Instant>) {
    if revert_at.is_some() {
        *revert_at = Some(Instant::now() + Duration::from_millis(BAUD_REVERT_MS));
    }
}

fn build_nak(out: &mut [u8], seq: u16, code: u16, detail: &str) -> usize {
    hwlink::nak(out, seq, code, detail)
        .map(|p| p.len())
        .unwrap_or(0)
}

fn reply(
    request: Request<'_>,
    seq: u16,
    out: &mut [u8],
    status_rx: &mut StatusReceiver<'_>,
    info: &BootInfo,
) -> (Action, u8, usize) {
    let mut action = Action::None;

    let len = match request {
        Request::Hello { .. } => {
            let (dev_id, rev_id) = boot::device_id();
            let mut identity_buf = [0u8; 192];
            let hello = hwlink::HelloReply {
                firmware_name: "embassy-app",
                version: build_info::VERSION_TEXT,
                commit: build_info::GIT_COMMIT,
                build_time: build_info::BUILD_TIME,
                uid: boot::device_uid(),
                dev_id,
                rev_id,
                flash_mode: hwlink::flash_mode::DUAL_BANK,
                detail: identity::detail(&mut identity_buf),
            };
            hwlink::hello_reply(out, &hello).map(|p| {
                action = Action::Hello;
                p.len()
            })
        }
        Request::Ping => hwlink::pong(out, Instant::now().as_millis()).map(|p| p.len()),
        Request::GetStatus => {
            let snap = status_rx.try_get().unwrap_or_else(StatusSnapshot::initial);
            let fault_record = fault::last();
            let mut fault_file = [0u8; fault::FILE_NAME_MAX];
            let fault_file_len = fault_record
                .as_ref()
                .map(|f| {
                    let n = (f.file_len as usize).min(f.file.len());
                    fault_file[..n].copy_from_slice(&f.file[..n]);
                    n
                })
                .unwrap_or(0);
            let active_fault = fault_record.as_ref().map(|f| hwlink::FaultInfoData {
                present: true,
                reason: fault::describe(f),
                cfsr: f.cfsr,
                hfsr: f.hfsr,
                mmfar: f.mmfar,
                bfar: f.bfar,
                pc: f.pc,
                lr: f.lr,
                task: "",
                file: core::str::from_utf8(&fault_file[..fault_file_len]).unwrap_or(""),
                line: f.line,
            });
            let leds = [
                hwlink::LedStateData {
                    pattern: snap.led1_pattern,
                    period_ms: snap.led1_period,
                },
                hwlink::LedStateData {
                    pattern: snap.led2_pattern,
                    period_ms: snap.led2_period,
                },
                hwlink::LedStateData {
                    pattern: snap.led3_pattern,
                    period_ms: snap.led3_period,
                },
            ];
            let stack = snap.stack_high_water.min(u16::MAX as u32) as u16;
            let tasks = [
                hwlink::TaskStatsData {
                    name: "health",
                    priority: 5,
                    state: 1,
                    stack_high_water: stack,
                },
                hwlink::TaskStatsData {
                    name: "comms",
                    priority: 4,
                    state: 1,
                    stack_high_water: stack,
                },
                hwlink::TaskStatsData {
                    name: "button",
                    priority: 3,
                    state: 1,
                    stack_high_water: stack,
                },
                hwlink::TaskStatsData {
                    name: "led",
                    priority: 2,
                    state: 1,
                    stack_high_water: stack,
                },
            ];
            let status = hwlink::Status {
                firmware_name: "embassy-app",
                version: build_info::VERSION,
                commit: build_info::GIT_COMMIT,
                build_time: build_info::BUILD_TIME,
                uid: boot::device_uid(),
                uptime_ms: Instant::now().as_millis(),
                slot: info.handover.slot,
                trial: info.handover.current_trial_state(),
                trial_count: info.handover.trial_count,
                health: snap.health_state,
                reset_cause: info.reset_cause,
                reset_cause_text: boot::reset_cause_text(info.reset_cause),
                fault: active_fault,
                leds,
                tasks: &tasks,
                heap_free: 0,
                heap_min_free: 0,
            };
            hwlink::status(out, &status).map(|p| p.len())
        }
        Request::SetLed {
            led: target,
            pattern: p,
            period_ms,
            force,
        } => {
            if !(led::LD1..=led::LD3).contains(&target)
                || !pattern::valid(p)
                || !valid_led_period(period_ms)
                || (target == led::LD3 && !force)
            {
                Some(build_nak(
                    out,
                    seq,
                    error::BAD_ARGUMENT,
                    "LED, pattern or period",
                ))
            } else {
                let command = LedCommand::Set {
                    led: target,
                    pattern: p,
                    period_ms,
                    force,
                };
                if ipc::LED_COMMANDS.try_send(command).is_err() {
                    Some(build_nak(out, seq, error::BUSY, "LED queue full"))
                } else {
                    ipc::STATUS.sender().send_modify(|v| {
                        if let Some(v) = v {
                            crate::led::mirror_set(v, target, p, period_ms);
                        }
                    });
                    hwlink::ack(out, seq).map(|r| r.len())
                }
            }
        }
        Request::SetLedAll {
            pattern: p,
            period_ms,
        } => {
            if !pattern::valid(p) || !valid_led_period(period_ms) {
                Some(build_nak(
                    out,
                    seq,
                    error::BAD_ARGUMENT,
                    "pattern or period",
                ))
            } else {
                let command = LedCommand::SetAll {
                    pattern: p,
                    period_ms,
                };
                if ipc::LED_COMMANDS.try_send(command).is_err() {
                    Some(build_nak(out, seq, error::BUSY, "LED queue full"))
                } else {
                    ipc::STATUS.sender().send_modify(|v| {
                        if let Some(v) = v {
                            crate::led::mirror_set_all(v, p, period_ms);
                        }
                    });
                    hwlink::ack(out, seq).map(|r| r.len())
                }
            }
        }
        Request::ClearFault => {
            fault::clear();
            let _ = ipc::LED_COMMANDS.try_send(LedCommand::ClearFault);
            hwlink::ack(out, seq).map(|r| r.len())
        }
        Request::EnterBootloader => hwlink::ack(out, seq).map(|r| {
            action = Action::EnterBootloader;
            r.len()
        }),
        Request::Reset => hwlink::ack(out, seq).map(|r| {
            action = Action::Reset;
            r.len()
        }),
        Request::SetBaud { rate } => {
            if rate == 115_200 || rate == 921_600 {
                hwlink::ack(out, seq).map(|r| {
                    action = Action::SetBaud(rate);
                    r.len()
                })
            } else {
                Some(build_nak(out, seq, error::BAD_ARGUMENT, "baud rate"))
            }
        }
        Request::Unknown => Some(build_nak(out, seq, error::UNSUPPORTED, "message")),
    };

    let len = len.unwrap_or_else(|| build_nak(out, seq, error::MALFORMED, "encode"));
    (action, FLAG_RESPONSE, len)
}

fn handle_payload(
    payload: &[u8],
    seq: u16,
    out: &mut [u8],
    status_rx: &mut StatusReceiver<'_>,
    info: &BootInfo,
) -> (Action, u8, usize) {
    match hwlink::parse_request(payload) {
        Ok(request) => reply(request, seq, out, status_rx, info),
        Err(ParseError::Unsupported) => (
            Action::None,
            FLAG_RESPONSE,
            build_nak(out, seq, error::UNSUPPORTED, "message"),
        ),
        Err(ParseError::Malformed) => (
            Action::None,
            FLAG_RESPONSE,
            build_nak(out, seq, error::MALFORMED, "verifier"),
        ),
    }
}

async fn send_event(
    tx: &mut BufferedUartTx<'static>,
    raw: &mut [u8],
    tx_buf: &mut [u8],
    payload: &mut [u8],
    event: ButtonEvent,
) {
    let Some(p) = hwlink::button_event(payload, event.press, event.pattern, event.period_ms) else {
        return;
    };
    let len = p.len();
    if let Some(n) = frame::encode(raw, tx_buf, FLAG_EVENT, 0, &payload[..len]) {
        let _ = tx.write_all(&tx_buf[..n]).await;
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_frame(
    decoded: &[u8],
    raw: &mut [u8],
    tx_buf: &mut [u8],
    payload: &mut [u8],
    last: &mut LastResponse,
    status_rx: &mut StatusReceiver<'_>,
    info: &BootInfo,
    tx: &mut BufferedUartTx<'static>,
    rx: &mut BufferedUartRx<'static>,
    revert_at: &mut Option<Instant>,
    link_open: &mut bool,
    decoder: &mut frame::Decoder,
) {
    let Some(f) = frame::parse(decoded) else {
        return;
    };
    touch_baud(revert_at);
    if f.flags & (FLAG_RESPONSE | FLAG_EVENT) != 0 {
        return;
    }

    // A new host session starts at sequence 1; never answer its Hello from
    // the replay cache.
    if matches!(
        hwlink::parse_request(f.payload),
        Ok(hwlink::Request::Hello { .. })
    ) {
        last.len = 0;
    }

    // LNK-008: repeated seq resends the last response without re-executing.
    if f.seq != 0 && last.seq == f.seq && last.len != 0 {
        let _ = tx.write_all(&last.buf[..last.len]).await;
        touch_baud(revert_at);
        return;
    }

    let (action, flags, len) = handle_payload(f.payload, f.seq, payload, status_rx, info);
    if len == 0 {
        return;
    }
    let Some(n) = frame::encode(raw, tx_buf, flags, f.seq, &payload[..len]) else {
        return;
    };

    if tx.write_all(&tx_buf[..n]).await.is_err() {
        return;
    }

    // Cache only after the frame was written: a failed TX must not make the
    // host's retry replay a response for a request that never ran.
    last.seq = f.seq;
    last.len = n;
    last.buf[..n].copy_from_slice(&tx_buf[..n]);
    touch_baud(revert_at);

    match action {
        Action::None => {}
        Action::Hello => *link_open = true,
        Action::Reset => {
            let _ = tx.flush().await;
            boot::reset();
        }
        Action::EnterBootloader => {
            let _ = tx.flush().await;
            boot::enter_bootloader();
        }
        Action::SetBaud(rate) => {
            let _ = tx.flush().await;
            *decoder = frame::Decoder::new();
            if tx.set_baudrate(rate).is_ok() && rx.set_baudrate(rate).is_ok() {
                *revert_at = if rate == 115_200 {
                    None
                } else {
                    Some(Instant::now() + Duration::from_millis(BAUD_REVERT_MS))
                };
            } else {
                let _ = tx.set_baudrate(115_200);
                let _ = rx.set_baudrate(115_200);
                *revert_at = None;
            }
        }
    }
}

#[embassy_executor::task]
pub async fn task(uart: BufferedUart<'static>, info: BootInfo) {
    let (mut tx, mut rx) = uart.split();
    let mut decoder = frame::Decoder::new();
    let mut decoded = [0u8; MAX_DECODED];
    let mut raw = [0u8; MAX_DECODED];
    let mut tx_buf = [0u8; MAX_ENCODED];
    let mut payload = [0u8; PAYLOAD_MAX];
    let mut last = LastResponse::new();
    let mut rx_buf = [0u8; 64];
    let mut revert_at: Option<Instant> = None;
    let mut link_open = false;

    let mut status_rx = match ipc::STATUS.receiver() {
        Some(r) => r,
        None => fault::fatal(),
    };

    let mut report = crate::log::TaskReport::new(
        4,
        "Comms",
        "serves HWLINK on USART3: info, identity, button and LED commands",
    );
    loop {
        report.tick();
        ipc::check_in(ipc::TASK_COMMS);

        let wait_ms = match revert_at {
            Some(deadline) => deadline
                .saturating_duration_since(Instant::now())
                .as_millis()
                .clamp(1, IDLE_CHECKIN_MS),
            None => IDLE_CHECKIN_MS,
        };

        let event = select(
            select(rx.read(&mut rx_buf), ipc::BUTTON_EVENTS.receive()),
            Timer::after(Duration::from_millis(wait_ms)),
        )
        .await;

        match event {
            Either::First(Either::First(Ok(0))) => {
                // Do not spin if the driver reports an empty read.
                Timer::after(Duration::from_millis(1)).await;
            }
            Either::First(Either::First(Ok(n))) => {
                for &byte in &rx_buf[..n] {
                    if decoder.push(byte) {
                        if let Some(len) = decoder.take(&mut decoded) {
                            handle_frame(
                                &decoded[..len],
                                &mut raw,
                                &mut tx_buf,
                                &mut payload,
                                &mut last,
                                &mut status_rx,
                                &info,
                                &mut tx,
                                &mut rx,
                                &mut revert_at,
                                &mut link_open,
                                &mut decoder,
                            )
                            .await;
                        }
                    }
                }
            }
            Either::First(Either::First(Err(_))) => {
                Timer::after(Duration::from_millis(1)).await;
            }
            Either::First(Either::Second(event)) => {
                if link_open {
                    send_event(&mut tx, &mut raw, &mut tx_buf, &mut payload, event).await;
                }
            }
            Either::Second(()) => {
                if let Some(deadline) = revert_at {
                    if Instant::now() >= deadline {
                        decoder = frame::Decoder::new();
                        let _ = tx.set_baudrate(115_200);
                        let _ = rx.set_baudrate(115_200);
                        revert_at = None;
                    }
                }
            }
        }
    }
}
