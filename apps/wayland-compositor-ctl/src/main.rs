//! CLI tool for controlling a running `PlatynUI` Wayland compositor.
//!
//! Connects to the compositor's Unix control socket and sends JSON commands
//! to query status, manage windows, take screenshots, and shut down.
//!
//! This binary only builds on Linux.

#[cfg(target_os = "linux")]
mod app;

#[cfg(not(target_os = "linux"))]
fn main() -> std::process::ExitCode {
    eprintln!("platynui-wayland-compositor-ctl is only supported on Linux");
    std::process::ExitCode::from(1)
}

#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    app::run()
}
