// system/mod.rs - runtime services shared by the tasks
//
// Author: Sandesh Ghimire <sandesh@soccentric.com>
// SRS:    RGEN-002, RRT-003, RRT-004, RDBG-001

// The log macros are used across the whole crate, so export them first.
#[macro_use]
pub mod log;

pub mod build_info;
pub mod ipc;
