//! Readiness notification — signals to CI scripts that the compositor is ready.

use std::io::Write;

/// Notify that the compositor is ready to accept clients.
///
/// Supports three modes:
/// - `ready_fd`: Write `READY\n` to the specified file descriptor via `/dev/fd/<N>`.
/// - `print_env`: Print environment variables to stdout.
/// - Fallback: Write `READY\n` to stderr.
pub fn notify_ready(socket_name: &str, ready_fd: Option<i32>, print_env: bool) {
    if print_env {
        println!("WAYLAND_DISPLAY={socket_name}");
        if let Ok(control) = std::env::var("PLATYNUI_CONTROL_SOCKET") {
            println!("PLATYNUI_CONTROL_SOCKET={control}");
        }
        if let Ok(eis) = std::env::var("LIBEI_SOCKET") {
            println!("LIBEI_SOCKET={eis}");
        }
    }

    if let Some(fd) = ready_fd {
        // Use /dev/fd/<N> to open the file descriptor without unsafe code
        let path = format!("/dev/fd/{fd}");
        match std::fs::OpenOptions::new().write(true).open(&path) {
            Ok(mut file) => {
                if let Err(err) = writeln!(file, "READY") {
                    tracing::warn!(fd, %err, "failed to write READY to notification fd");
                }
            }
            Err(err) => {
                tracing::warn!(fd, %err, "failed to write readiness notification to fd");
            }
        }
    } else {
        eprintln!("READY");
    }

    tracing::info!(socket = socket_name, "compositor ready");
}
