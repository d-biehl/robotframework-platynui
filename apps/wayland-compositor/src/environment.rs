//! Environment setup — XDG runtime directory, socket management.

use std::path::PathBuf;

/// Ensure `$XDG_RUNTIME_DIR` exists and is usable.
///
/// # Errors
///
/// Returns an error if the variable is not set and cannot be inferred.
pub fn ensure_xdg_runtime_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = std::env::var("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .map_err(|_| "XDG_RUNTIME_DIR is not set; required for Wayland socket creation")?;

    if !dir.exists() {
        return Err(format!("XDG_RUNTIME_DIR ({}) does not exist", dir.display()).into());
    }

    Ok(dir)
}

/// Set `WAYLAND_DISPLAY` for child processes.
///
/// # Safety
///
/// `std::env::set_var` is unsafe in Edition 2024 because modifying the environment
/// is not thread-safe. We call this only during single-threaded startup.
#[allow(unsafe_code)]
pub fn set_wayland_display(socket_name: &str) {
    // SAFETY: Called during single-threaded startup before any client connections.
    unsafe {
        std::env::set_var("WAYLAND_DISPLAY", socket_name);
    }
    tracing::debug!(wayland_display = socket_name, "WAYLAND_DISPLAY set");
}

/// Set `PLATYNUI_CONTROL_SOCKET` for child processes and tools.
///
/// This allows `platynui-wayland-compositor-ctl` and other tools running inside
/// the compositor session to discover the control socket without deriving the
/// path manually.
///
/// # Safety
///
/// `std::env::set_var` is unsafe in Edition 2024 because modifying the environment
/// is not thread-safe. We call this only during single-threaded startup.
#[allow(unsafe_code)]
pub fn set_control_socket_env(path: &std::path::Path) {
    // SAFETY: Called during single-threaded startup before any client connections.
    unsafe {
        std::env::set_var("PLATYNUI_CONTROL_SOCKET", path);
    }
    tracing::debug!(path = %path.display(), "PLATYNUI_CONTROL_SOCKET set");
}

/// Export `LIBEI_SOCKET` so child processes (and test tools) can find the EIS endpoint.
#[allow(unsafe_code)]
pub fn set_eis_socket_env(path: &std::path::Path) {
    // SAFETY: Called during single-threaded startup before any client connections.
    unsafe {
        std::env::set_var("LIBEI_SOCKET", path);
    }
    tracing::debug!(path = %path.display(), "LIBEI_SOCKET set");
}
