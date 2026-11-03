// build.rs - put memory.x on the linker search path
//
// Author: Sandesh Ghimire <sandesh@soccentric.com>
// SRS:    RBT-001

use std::{env, fs, path::PathBuf};

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by cargo"));
    fs::copy("memory.x", out.join("memory.x")).expect("memory.x is readable");
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory.x");
}
