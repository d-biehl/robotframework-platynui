//! `PlatynUI` Wayland Compositor binary entry point.
//!
//! This is a thin wrapper that delegates to [`platynui_wayland_compositor::run`].
//!
//! This binary only builds on Linux.

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("platynui-wayland-compositor is only supported on Linux");
    std::process::exit(1);
}

#[cfg(target_os = "linux")]
// Dependencies used by the library crate, acknowledged here for the binary crate.
use {
    calloop as _, clap as _, egui as _, egui_glow as _, png as _, reis as _, serde as _, serde_json as _,
    signal_hook as _, smithay as _, tempfile as _, toml as _, tracing as _, tracing_subscriber as _, winit as _,
    xcursor as _, zbus as _,
};

#[cfg(target_os = "linux")]
fn main() {
    if let Err(error) = platynui_wayland_compositor::run() {
        eprintln!("Compositor exited with error: {error}");
        std::process::exit(1);
    }
}
