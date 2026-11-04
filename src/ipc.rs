// ipc.rs - channels and signals between the four Embassy tasks
//
// Author: Sandesh Ghimire <sandesh@soccentric.com>
// SRS:    RRT-003, RRT-004; DRT-011

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embassy_sync::watch::Watch;

pub const TASK_LED: usize = 0;
pub const TASK_BUTTON: usize = 1;
pub const TASK_COMMS: usize = 2;
pub const TASK_HEALTH: usize = 3;

#[derive(Clone, Copy)]
pub struct StatusSnapshot {
    pub led1_pattern: u8,
    pub led1_period: u16,
    pub led2_pattern: u8,
    pub led2_period: u16,
    pub health_state: u8,
    pub fault_active: bool,
    pub health_failed: bool,
    pub task_checks: u32,
    pub stack_high_water: u32,
    pub led3_pattern: u8,
    pub led3_period: u16,
}

impl StatusSnapshot {
    pub const fn initial() -> Self {
        Self {
            led1_pattern: crate::hwlink::pattern::HEARTBEAT,
            led1_period: 1000,
            led2_pattern: crate::hwlink::pattern::OFF,
            led2_period: 1000,
            health_state: crate::board::HEALTH_NOT_REQUIRED,
            fault_active: false,
            health_failed: false,
            task_checks: 0,
            stack_high_water: 0,
            led3_pattern: crate::hwlink::pattern::OFF,
            led3_period: 1000,
        }
    }
}

pub static STATUS: Watch<CriticalSectionRawMutex, StatusSnapshot, 2> =
    Watch::new_with(StatusSnapshot::initial());

#[derive(Clone, Copy)]
pub enum LedCommand {
    Set {
        led: u8,
        pattern: u8,
        period_ms: u16,
        force: bool,
    },
    SetAll {
        pattern: u8,
        period_ms: u16,
    },
    ButtonShort,
    ButtonLong,
    ClearFault,
    HealthFailed,
}

pub static LED_COMMANDS: Channel<CriticalSectionRawMutex, LedCommand, 8> = Channel::new();

pub const BUTTON_SHORT: u8 = 1;
pub const BUTTON_LONG: u8 = 2;

#[derive(Clone, Copy)]
pub struct ButtonEvent {
    pub press: u8,
    pub pattern: u8,
    pub period_ms: u16,
}

pub static BUTTON_EVENTS: Channel<CriticalSectionRawMutex, ButtonEvent, 4> = Channel::new();

static CHECKINS: [Signal<CriticalSectionRawMutex, ()>; 4] =
    [Signal::new(), Signal::new(), Signal::new(), Signal::new()];

/// Called at least once per loop by each task (RRT-004).
pub fn check_in(task: usize) {
    CHECKINS[task].signal(());
}

pub fn take_check_in(task: usize) -> bool {
    CHECKINS[task].try_take().is_some()
}
