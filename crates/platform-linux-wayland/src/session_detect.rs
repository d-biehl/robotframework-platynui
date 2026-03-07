#![allow(dead_code)]

use std::fs;
use std::path::Path;

/// Session display technology for a running application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppDisplayBackend {
    /// Application is running as a native Wayland client.
    WaylandNative,
    /// Application is running under `XWayland` compatibility.
    XWayland,
    /// Could not determine the display backend.
    Unknown,
}

/// Detect whether an application (identified by PID) is running as a Wayland-native
/// client or under `XWayland`.
///
/// This reads `/proc/{pid}/environ` to check for environment variables that
/// indicate the display backend:
/// - `GDK_BACKEND=wayland` -> GTK Wayland-native
/// - `QT_QPA_PLATFORM=wayland` -> Qt Wayland-native
/// - `MOZ_ENABLE_WAYLAND=1` -> Firefox/Thunderbird Wayland-native
/// - `DISPLAY` without Wayland markers -> likely `XWayland`
///
/// **Coordinate implications:**
/// - Wayland-native: AT-SPI `GetExtents(SCREEN)` returns `(0, 0)` for window origin.
///   Must use `GetExtents(WINDOW)` + window offset from compositor.
/// - `XWayland`: AT-SPI `GetExtents(SCREEN)` returns `XWayland`-virtual coordinates
///   (correct within the `XWayland` coordinate space).
pub fn detect_app_backend(pid: u32) -> AppDisplayBackend {
    let environ_path = format!("/proc/{pid}/environ");
    let path = Path::new(&environ_path);

    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            tracing::debug!(pid, error = %e, "cannot read process environment");
            return AppDisplayBackend::Unknown;
        }
    };

    // /proc/PID/environ uses null bytes as separators.
    let vars: Vec<&[u8]> = data.split(|&b| b == 0).collect();

    let mut has_wayland_display = false;
    let mut has_x_display = false;
    let mut gtk_wayland = false;
    let mut qt_wayland = false;
    let mut moz_wayland = false;

    for var in &vars {
        if var.starts_with(b"WAYLAND_DISPLAY=") {
            has_wayland_display = true;
        } else if var.starts_with(b"DISPLAY=") {
            has_x_display = true;
        } else if var == b"GDK_BACKEND=wayland" {
            gtk_wayland = true;
        } else if var == b"QT_QPA_PLATFORM=wayland" {
            qt_wayland = true;
        } else if var == b"MOZ_ENABLE_WAYLAND=1" {
            moz_wayland = true;
        }
    }

    // Explicit Wayland backend markers take priority.
    if gtk_wayland || qt_wayland || moz_wayland {
        return AppDisplayBackend::WaylandNative;
    }

    // If WAYLAND_DISPLAY is set but no DISPLAY, likely Wayland-native.
    if has_wayland_display && !has_x_display {
        return AppDisplayBackend::WaylandNative;
    }

    // If only DISPLAY is set (no WAYLAND_DISPLAY), likely XWayland.
    if has_x_display && !has_wayland_display {
        return AppDisplayBackend::XWayland;
    }

    // Both set — ambiguous. Most toolkits default to Wayland when both are present,
    // but we can't be certain without toolkit-specific markers.
    if has_wayland_display && has_x_display {
        return AppDisplayBackend::WaylandNative;
    }

    AppDisplayBackend::Unknown
}

/// Detect the current session type from `XDG_SESSION_TYPE`.
pub fn session_type() -> SessionType {
    match std::env::var("XDG_SESSION_TYPE").as_deref() {
        Ok("wayland") => SessionType::Wayland,
        Ok("x11") => SessionType::X11,
        Ok("tty") => SessionType::Tty,
        _ => SessionType::Unknown,
    }
}

/// The type of the current login session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionType {
    Wayland,
    X11,
    Tty,
    Unknown,
}
