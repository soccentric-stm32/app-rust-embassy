// button.rs - B1 (PC13) debounce, short/long press and LED pattern cycling
//
// Author: Sandesh Ghimire <sandesh@soccentric.com>
// SRS:    RFN-002; BTN-001..BTN-004
//
// The task only debounces the button and sends one command per press.  The
// LED task owns the pattern state (as in the C demo), so a second press that
// arrives before the first command has been handled can never lose a step.

use embassy_futures::select::select;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::mode::Async;
use embassy_time::{Duration, Instant, Timer};

use crate::ipc::{self, LedCommand};

const DEBOUNCE_MS: u64 = 30;
const SHORT_MAX_MS: u64 = 1000;
const LONG_MIN_MS: u64 = 2000;
const POLL: Duration = Duration::from_millis(10);
const CHECKIN_PERIOD_MS: u64 = 500;

#[embassy_executor::task]
pub async fn task(mut btn: ExtiInput<'static, Async>) {
    // B1 is high when pressed and has an external pull-down on the board.
    let mut pressed = btn.is_high();
    let mut candidate = pressed;
    let mut candidate_since = Instant::now();
    let mut press_start = Instant::now();
    let mut long_sent = false;
    let mut last_checkin = Instant::now();

    loop {
        // EXTI wakes us on an edge; the timeout keeps the debounce loop and
        // the health check-in running while the button is held.
        let _ = select(btn.wait_for_any_edge(), Timer::after(POLL)).await;

        let now = Instant::now();
        let raw = btn.is_high();
        if raw != candidate {
            candidate = raw;
            candidate_since = now;
        } else if candidate != pressed
            && now.duration_since(candidate_since).as_millis() >= DEBOUNCE_MS
        {
            pressed = candidate;
            if pressed {
                press_start = now;
                long_sent = false;
            } else {
                let held = now.duration_since(press_start).as_millis();
                if !long_sent && held < SHORT_MAX_MS {
                    log_info!("button: short press, next LED pattern");
                    let _ = ipc::LED_COMMANDS.try_send(LedCommand::ButtonShort);
                }
            }
        }

        // BTN-003: a long press acts at 2 s, not only when the button is
        // released, exactly as the C demo's button task does.
        if pressed && !long_sent && now.duration_since(press_start).as_millis() >= LONG_MIN_MS {
            long_sent = true;
            log_info!("button: long press, default LED patterns");
            let _ = ipc::LED_COMMANDS.try_send(LedCommand::ButtonLong);
        }

        if now.duration_since(last_checkin).as_millis() >= CHECKIN_PERIOD_MS {
            ipc::check_in(ipc::TASK_BUTTON);
            last_checkin = now;
        }
    }
}
