// led.rs - LED task and Appendix A patterns
//
// Author: Sandesh Ghimire <sandesh@soccentric.com>
// SRS:    RFN-001; LED-001..LED-007, LED-005 (LD3 reserved)

use embassy_stm32::gpio::Output;
use embassy_time::{Duration, Instant, Ticker};

use crate::hwlink::{led, pattern};
use crate::ipc::{self, ButtonEvent, LedCommand};

const DEFAULT_PERIOD_MS: u16 = 1000;
const TICK_MS: u64 = 1;

fn morse_sos_on(elapsed_ms: u64) -> bool {
    // ··· — — — ··· with a 100 ms unit, including the 3/7 unit gaps.
    matches!(
        elapsed_ms % 3400,
        0..100
            | 200..300
            | 400..500
            | 800..1100
            | 1200..1500
            | 1600..1900
            | 2200..2300
            | 2400..2500
            | 2600..2700
    )
}

fn pattern_on(pattern: u8, period_ms: u16, elapsed_ms: u64, led2: bool) -> bool {
    let period = period_ms.max(50) as u64;
    match pattern {
        pattern::OFF => false,
        pattern::ON => true,
        pattern::BLINK => elapsed_ms % period < period / 2,
        pattern::HEARTBEAT => {
            let cycle = period.max(250);
            let phase = elapsed_ms % cycle;
            phase < 50 || (200..250).contains(&phase)
        }
        pattern::FAST => {
            let fast = (period / 5).max(2);
            elapsed_ms % fast < fast / 2
        }
        pattern::CHASE => {
            let half = period / 2;
            if led2 {
                elapsed_ms % period >= half
            } else {
                elapsed_ms % period < half
            }
        }
        pattern::MORSE_SOS => morse_sos_on(elapsed_ms),
        _ => false,
    }
}

fn update_status(led1: u8, p1: u16, led2: u8, p2: u16, led3: u8, p3: u16, fault: bool) {
    ipc::STATUS.sender().send_modify(|v| {
        if let Some(v) = v {
            v.led1_pattern = led1;
            v.led1_period = p1;
            v.led2_pattern = led2;
            v.led2_period = p2;
            v.led3_pattern = led3;
            v.led3_period = p3;
            v.fault_active = fault;
        }
    });
}

#[embassy_executor::task]
pub async fn task(
    mut led1: Output<'static>,
    mut led2: Output<'static>,
    mut led3: Output<'static>,
    initial_fault: bool,
) {
    let mut pattern1 = pattern::HEARTBEAT;
    let mut pattern2 = pattern::OFF;
    let mut period1 = DEFAULT_PERIOD_MS;
    let mut period2 = DEFAULT_PERIOD_MS;
    let mut phase1 = Instant::now();
    let mut phase2 = Instant::now();
    let mut fault_active = initial_fault;
    let mut health_failed = false;
    let mut led3_forced: Option<(u8, u16)> = None;
    let mut led3_pattern = pattern::OFF;
    let mut led3_period = DEFAULT_PERIOD_MS;
    let mut led3_phase = Instant::now();

    led1.set_low();
    led2.set_low();
    led3.set_low();

    update_status(
        pattern1,
        period1,
        pattern2,
        period2,
        led3_pattern,
        led3_period,
        fault_active,
    );

    let mut ticker = Ticker::every(Duration::from_millis(TICK_MS));
    let mut report = crate::log::TaskReport::new(
        1,
        "LED",
        "drives LD1-LD3 patterns, LD3 shows health and fault state",
    );
    loop {
        report.tick();
        while let Ok(command) = ipc::LED_COMMANDS.try_receive() {
            match command {
                LedCommand::Set {
                    led,
                    pattern: p,
                    period_ms,
                    force,
                } => match led {
                    crate::hwlink::led::LD1 => {
                        let now = Instant::now();
                        if p == pattern::CHASE {
                            pattern1 = p;
                            pattern2 = p;
                            period1 = period_ms;
                            period2 = period_ms;
                            phase1 = now;
                            phase2 = now;
                        } else {
                            if pattern1 == pattern::CHASE || pattern2 == pattern::CHASE {
                                pattern2 = pattern::OFF;
                                period2 = period_ms;
                                phase2 = now;
                            }
                            pattern1 = p;
                            period1 = period_ms;
                            phase1 = now;
                        }
                    }
                    crate::hwlink::led::LD2 => {
                        let now = Instant::now();
                        if p == pattern::CHASE {
                            pattern1 = p;
                            pattern2 = p;
                            period1 = period_ms;
                            period2 = period_ms;
                            phase1 = now;
                            phase2 = now;
                        } else {
                            if pattern1 == pattern::CHASE || pattern2 == pattern::CHASE {
                                pattern1 = pattern::OFF;
                                period1 = period_ms;
                                phase1 = now;
                            }
                            pattern2 = p;
                            period2 = period_ms;
                            phase2 = now;
                        }
                    }
                    crate::hwlink::led::LD3 if force => {
                        led3_forced = Some((p, period_ms));
                        led3_pattern = p;
                        led3_period = period_ms;
                        led3_phase = Instant::now();
                    }
                    _ => {}
                },
                LedCommand::SetAll {
                    pattern: p,
                    period_ms,
                } => {
                    pattern1 = p;
                    pattern2 = p;
                    period1 = period_ms;
                    period2 = period_ms;
                    phase1 = Instant::now();
                    phase2 = Instant::now();
                }
                LedCommand::ButtonShort => {
                    // BTN-002: the next Appendix A pattern on LD1 and LD2.
                    let now = Instant::now();
                    let period = if (50..=5000).contains(&period1) && period1.is_multiple_of(10) {
                        period1
                    } else {
                        DEFAULT_PERIOD_MS
                    };
                    let next = next_pattern(pattern1);
                    pattern1 = next;
                    pattern2 = next;
                    period1 = period;
                    period2 = period;
                    phase1 = now;
                    phase2 = now;
                    let _ = ipc::BUTTON_EVENTS.try_send(ButtonEvent {
                        press: ipc::BUTTON_SHORT,
                        pattern: next,
                        period_ms: period,
                    });
                }
                LedCommand::ButtonLong => {
                    // BTN-003: restore the defaults and report the event.
                    let now = Instant::now();
                    pattern1 = pattern::HEARTBEAT;
                    pattern2 = pattern::OFF;
                    period1 = DEFAULT_PERIOD_MS;
                    period2 = DEFAULT_PERIOD_MS;
                    phase1 = now;
                    phase2 = now;
                    let _ = ipc::BUTTON_EVENTS.try_send(ButtonEvent {
                        press: ipc::BUTTON_LONG,
                        pattern: pattern::HEARTBEAT,
                        period_ms: DEFAULT_PERIOD_MS,
                    });
                }
                LedCommand::ClearFault => {
                    // LED-005: health_failed is separate; a pending slot
                    // that failed its health check keeps blinking LD3.
                    fault_active = false;
                }
                LedCommand::HealthFailed => {
                    health_failed = true;
                }
            }
            update_status(
                pattern1,
                period1,
                pattern2,
                period2,
                led3_pattern,
                led3_period,
                fault_active,
            );
        }

        let now = Instant::now();
        let elapsed1 = now.duration_since(phase1).as_millis();
        let elapsed2 = now.duration_since(phase2).as_millis();

        if pattern_on(pattern1, period1, elapsed1, false) {
            led1.set_high();
        } else {
            led1.set_low();
        }
        if pattern_on(pattern2, period2, elapsed2, true) {
            led2.set_high();
        } else {
            led2.set_low();
        }

        if health_failed {
            // LED-005: fast blink while the slot is pending and health failed.
            let phase = now.duration_since(led3_phase).as_millis() % 100;
            if phase < 50 {
                led3.set_high();
            } else {
                led3.set_low();
            }
        } else if fault_active {
            led3.set_high();
        } else if let Some((p, period)) = led3_forced {
            let elapsed = now.duration_since(led3_phase).as_millis();
            if pattern_on(p, period, elapsed, false) {
                led3.set_high();
            } else {
                led3.set_low();
            }
        } else {
            led3.set_low();
        }

        ipc::check_in(ipc::TASK_LED);
        ticker.next().await;
    }
}

/// Update a `Status` mirror for a `SetLed` command.  The comms task uses this
/// so that a `GetStatus` immediately after the `Ack` already reports the new
/// configuration; the LED task applies the same rules to its own state.
pub fn mirror_set(snapshot: &mut ipc::StatusSnapshot, target: u8, p: u8, period_ms: u16) {
    match target {
        led::LD1 => {
            if p == pattern::CHASE {
                snapshot.led1_pattern = p;
                snapshot.led1_period = period_ms;
                snapshot.led2_pattern = p;
                snapshot.led2_period = period_ms;
            } else {
                if snapshot.led1_pattern == pattern::CHASE
                    || snapshot.led2_pattern == pattern::CHASE
                {
                    snapshot.led2_pattern = pattern::OFF;
                    snapshot.led2_period = period_ms;
                }
                snapshot.led1_pattern = p;
                snapshot.led1_period = period_ms;
            }
        }
        led::LD2 => {
            if p == pattern::CHASE {
                snapshot.led1_pattern = p;
                snapshot.led1_period = period_ms;
                snapshot.led2_pattern = p;
                snapshot.led2_period = period_ms;
            } else {
                if snapshot.led1_pattern == pattern::CHASE
                    || snapshot.led2_pattern == pattern::CHASE
                {
                    snapshot.led1_pattern = pattern::OFF;
                    snapshot.led1_period = period_ms;
                }
                snapshot.led2_pattern = p;
                snapshot.led2_period = period_ms;
            }
        }
        led::LD3 => {
            snapshot.led3_pattern = p;
            snapshot.led3_period = period_ms;
        }
        _ => {}
    }
}

pub fn mirror_set_all(snapshot: &mut ipc::StatusSnapshot, p: u8, period_ms: u16) {
    snapshot.led1_pattern = p;
    snapshot.led1_period = period_ms;
    snapshot.led2_pattern = p;
    snapshot.led2_period = period_ms;
}

pub fn next_pattern(current: u8) -> u8 {
    // Appendix A order, skipping the reserved error LED.
    match current {
        pattern::OFF => pattern::ON,
        pattern::ON => pattern::BLINK,
        pattern::BLINK => pattern::HEARTBEAT,
        pattern::HEARTBEAT => pattern::FAST,
        pattern::FAST => pattern::CHASE,
        pattern::CHASE => pattern::MORSE_SOS,
        _ => pattern::OFF,
    }
}
