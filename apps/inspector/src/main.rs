//! PlatynUI Inspector binary entry point.
//!
//! This is a thin wrapper that delegates to [`platynui_inspector::run`].
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(error) = platynui_inspector::run() {
        eprintln!("Inspector exited with error: {error}");
        std::process::exit(1);
    }
}
