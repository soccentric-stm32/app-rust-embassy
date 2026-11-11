#![allow(dead_code)]
// build_info.rs - version, commit and build date embedded by build.rs
//
// Author: Sandesh Ghimire <sandesh@soccentric.com>
// SRS:    RGEN-002

include!(concat!(env!("OUT_DIR"), "/version.rs"));
