// health.rs - health task, IWDG refresh and CONFIRMED record
//
// Author: Sandesh Ghimire <sandesh@soccentric.com>
// SRS:    RRT-004, RFN-004; HLT-001..HLT-005

use embassy_stm32::flash::{Blocking, Flash};
use embassy_stm32::peripherals::IWDG;
use embassy_stm32::wdg::IndependentWatchdog;
use embassy_time::{Duration, Instant, Ticker};

use crate::board;
use crate::boot;
use crate::fault::{self, FaultRecord};
use crate::ipc::{self, LedCommand};

const CHECKIN_TIMEOUT: u64 = 2000;
const HEALTH_PASS_MS: u64 = 5000;
const HEALTH_LIMIT_MS: u64 = 10000;

fn same_fault(a: Option<FaultRecord>, b: Option<FaultRecord>) -> bool {
    a == b
}

fn set_health(state: u8, failed: bool, checks: u32) {
    ipc::STATUS.sender().send_modify(|v| {
        if let Some(v) = v {
            v.health_state = state;
            v.health_failed = failed;
            v.task_checks = checks;
        }
    });
}

fn stack_usage() -> u32 {
    // RRT-005: high-water mark of the shared Embassy stack, painted at start
    // by fault::paint_stack().
    fault::stack_high_water()
}

#[embassy_executor::task]
pub async fn task(
    mut wdg: IndependentWatchdog<'static, IWDG>,
    mut flash: Flash<'static, Blocking>,
    handover: boot::Handover,
    fault_at_start: Option<FaultRecord>,
) {
    let start = Instant::now();
    let mut last_seen = [start; 4];
    let mut counts = [0u32; 4];
    let mut health_checked = false;
    let mut failed = false;
    let mut stopped = false;
    // A fault in a previous trial of this pending slot has already failed a
    // trial; require a clean run before confirming it again (HLT-001 and the
    // C demo's previous-boot check).
    let previous_trial_fault =
        handover.pending() && handover.trial_count > 1 && fault_at_start.is_some();
    let mut ticker = Ticker::every(Duration::from_millis(200));

    wdg.pet();
    set_health(
        if handover.pending() {
            board::HEALTH_RUNNING
        } else {
            board::HEALTH_NOT_REQUIRED
        },
        false,
        0,
    );

    loop {
        // Health is a task too: its own check-in is the loop itself.
        ipc::check_in(ipc::TASK_HEALTH);

        let now = Instant::now();
        for task in 0..4 {
            let new_checks = ipc::take_check_in(task);
            if new_checks > 0 {
                counts[task] = counts[task].saturating_add(new_checks);
                last_seen[task] = now;
            }
        }
        let mut missed = false;
        for seen in &last_seen[..3] {
            if now.duration_since(*seen).as_millis() > CHECKIN_TIMEOUT {
                missed = true;
            }
        }
        let checks = counts.iter().sum();
        let stack = stack_usage();
        ipc::STATUS.sender().send_modify(|v| {
            if let Some(v) = v {
                v.stack_high_water = v.stack_high_water.max(stack);
            }
        });

        if missed && !stopped {
            // RRT-004: stop petting when another task misses its check-in.
            failed = true;
            stopped = true;
            set_health(board::HEALTH_FAILED, true, checks);
            let _ = ipc::LED_COMMANDS.try_send(LedCommand::HealthFailed);
        }

        let elapsed = now.duration_since(start).as_millis();
        let current_fault = fault::last();
        let new_fault = current_fault.is_some() && !same_fault(fault_at_start, current_fault);

        if handover.pending() && !health_checked {
            if elapsed >= HEALTH_LIMIT_MS {
                // HLT-002: the check must have passed by 10 s.
                log_error!("health: no verdict by 10 s, trial fails");
                health_checked = true;
                failed = true;
                set_health(board::HEALTH_FAILED, true, checks);
                let _ = ipc::LED_COMMANDS.try_send(LedCommand::HealthFailed);
            } else if elapsed >= HEALTH_PASS_MS {
                health_checked = true;
                let tasks_ok = counts.iter().all(|&c| c >= 5);
                let passed = !failed
                    && !previous_trial_fault
                    && !cfg!(feature = "fail-health")
                    && tasks_ok
                    && !new_fault;

                if passed && boot::write_confirmed(&mut flash) {
                    boot::mark_confirmed();
                    set_health(board::HEALTH_PASSED, false, checks);
                    log_info!("health: passed, CONFIRMED written");
                } else {
                    log_error!("health: check failed, trial fails");
                    failed = true;
                    set_health(board::HEALTH_FAILED, true, checks);
                    let _ = ipc::LED_COMMANDS.try_send(LedCommand::HealthFailed);
                }
            }
        } else if !handover.pending() && !failed {
            set_health(board::HEALTH_NOT_REQUIRED, false, checks);
        }

        if failed && elapsed >= HEALTH_LIMIT_MS {
            // HLT-003: stop refreshing the IWDG after 10 s so BOOT counts a
            // failed trial.
            stopped = true;
        }

        if !stopped {
            wdg.pet();
        }

        ticker.next().await;
    }
}
