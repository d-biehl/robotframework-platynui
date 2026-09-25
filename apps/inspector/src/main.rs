//! `PlatynUI` Inspector binary entry point.
//!
//! This is a thin wrapper that delegates to [`platynui_inspector::run`].
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// Bin target sharing the package's dependency list; everything but the entry point lives in the lib.
#![allow(unused_crate_dependencies)]

fn main() {
    if let Err(error) = platynui_inspector::run() {
        eprintln!("Inspector exited with error: {error}");
        std::process::exit(1);
    }
}
