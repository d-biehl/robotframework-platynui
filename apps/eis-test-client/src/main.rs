//! Standalone EIS test client for validating the EI protocol against compositors.
//!
//! Supports three connection modes:
//! - **Portal**: XDG Desktop Portal `RemoteDesktop` (for GNOME/Mutter, KDE/KWin)
//! - **Socket**: Direct EIS socket path (for custom compositors)
//! - **Environment**: `LIBEI_SOCKET` environment variable
//!
//! This binary only builds on Linux.

#[cfg(target_os = "linux")]
mod app;
#[cfg(target_os = "linux")]
mod portal;

#[cfg(not(target_os = "linux"))]
fn main() -> std::process::ExitCode {
    eprintln!("platynui-eis-test-client is only supported on Linux");
    std::process::ExitCode::from(1)
}

#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    app::run()
}
