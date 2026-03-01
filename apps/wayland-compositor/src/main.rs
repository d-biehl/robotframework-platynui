//! `PlatynUI` Wayland Compositor binary entry point.
//!
//! This is a thin wrapper that delegates to [`platynui_wayland_compositor::run`].

// Dependencies used by the library crate, acknowledged here for the binary crate.
use calloop as _;
use clap as _;
use egui as _;
use egui_glow as _;
use png as _;
use serde as _;
use signal_hook as _;
use smithay as _;
use toml as _;
use tracing as _;
use tracing_subscriber as _;

fn main() {
    if let Err(error) = platynui_wayland_compositor::run() {
        eprintln!("Compositor exited with error: {error}");
        std::process::exit(1);
    }
}
