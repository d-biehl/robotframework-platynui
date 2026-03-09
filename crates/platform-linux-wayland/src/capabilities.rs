//! Compositor detection via `SO_PEERCRED` on the Wayland socket.
//!
//! After connecting to the compositor, we can read the server PID from the
//! socket credentials and resolve `/proc/<pid>/exe` to identify which
//! compositor binary is running. This allows the crate to select the best
//! available backends (e.g. EIS for input, layer-shell for overlays).

use std::os::fd::AsFd as _;
use std::path::PathBuf;

use tracing::{debug, warn};
use wayland_client::Connection;

/// Known compositor types relevant for backend selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompositorType {
    /// Our own test compositor (platynui-wayland-compositor).
    PlatynUi,
    /// GNOME's Mutter.
    Mutter,
    /// KDE's `KWin`.
    KWin,
    /// Hyprland.
    Hyprland,
    /// Sway / wlroots-based.
    Sway,
    /// Another wlroots-based compositor.
    Wlroots,
    /// Compositor we don't specifically recognise.
    Unknown,
}

impl std::fmt::Display for CompositorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PlatynUi => f.write_str("PlatynUI"),
            Self::Mutter => f.write_str("Mutter"),
            Self::KWin => f.write_str("KWin"),
            Self::Hyprland => f.write_str("Hyprland"),
            Self::Sway => f.write_str("Sway"),
            Self::Wlroots => f.write_str("wlroots"),
            Self::Unknown => f.write_str("Unknown"),
        }
    }
}

/// Best-effort detection of the running Wayland compositor.
///
/// Strategy:
/// 1. Read `SO_PEERCRED` from the Wayland socket → server PID.
/// 2. `readlink /proc/<pid>/exe` → binary path → match known names.
/// 3. Fall back to `$XDG_CURRENT_DESKTOP` heuristic.
#[must_use]
pub fn detect_compositor(conn: &Connection) -> CompositorType {
    if let Some(ct) = detect_via_peercred(conn) {
        return ct;
    }
    detect_via_env()
}

/// Attempt detection via `SO_PEERCRED` on the underlying Unix socket.
fn detect_via_peercred(conn: &Connection) -> Option<CompositorType> {
    let backend = conn.backend();
    let guard = backend.poll_fd();
    let fd = guard.as_fd();

    match rustix::net::sockopt::socket_peercred(fd) {
        Ok(cred) => {
            let pid = cred.pid.as_raw_pid();
            let exe_link = PathBuf::from(format!("/proc/{pid}/exe"));
            match std::fs::read_link(&exe_link) {
                Ok(path) => {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    debug!(pid, exe = %path.display(), "compositor process identified");
                    Some(classify_binary_name(name))
                }
                Err(e) => {
                    debug!(pid, error = %e, "could not readlink /proc/<pid>/exe");
                    None
                }
            }
        }
        Err(e) => {
            debug!(error = %e, "SO_PEERCRED failed on Wayland socket");
            None
        }
    }
}

/// Classify a compositor by its binary name.
fn classify_binary_name(name: &str) -> CompositorType {
    // Normalize: strip path, lowercase for matching.
    let lower = name.to_ascii_lowercase();
    if lower.contains("platynui") {
        CompositorType::PlatynUi
    } else if lower.contains("mutter") || lower.contains("gnome-shell") {
        CompositorType::Mutter
    } else if lower.contains("kwin") {
        CompositorType::KWin
    } else if lower == "hyprland" || lower.starts_with("hyprland") {
        CompositorType::Hyprland
    } else if lower == "sway" {
        CompositorType::Sway
    } else if lower.contains("wlroots") {
        CompositorType::Wlroots
    } else {
        debug!(binary = name, "unrecognised compositor binary — using Unknown");
        CompositorType::Unknown
    }
}

/// Fall back to `$XDG_CURRENT_DESKTOP` when `SO_PEERCRED` is unavailable
/// (e.g. when running under a socket proxy or container).
fn detect_via_env() -> CompositorType {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let lower = desktop.to_ascii_lowercase();

    let ct = if lower.contains("gnome") {
        CompositorType::Mutter
    } else if lower.contains("kde") || lower.contains("plasma") {
        CompositorType::KWin
    } else if lower.contains("hyprland") {
        CompositorType::Hyprland
    } else if lower.contains("sway") {
        CompositorType::Sway
    } else {
        warn!(
            XDG_CURRENT_DESKTOP = %desktop,
            "could not identify compositor via SO_PEERCRED or XDG_CURRENT_DESKTOP"
        );
        CompositorType::Unknown
    };

    debug!(?ct, XDG_CURRENT_DESKTOP = %desktop, "compositor detected via environment");
    ct
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_known_binaries() {
        assert_eq!(classify_binary_name("mutter"), CompositorType::Mutter);
        assert_eq!(classify_binary_name("gnome-shell"), CompositorType::Mutter);
        assert_eq!(classify_binary_name("kwin_wayland"), CompositorType::KWin);
        assert_eq!(classify_binary_name("Hyprland"), CompositorType::Hyprland);
        assert_eq!(classify_binary_name("sway"), CompositorType::Sway);
        assert_eq!(classify_binary_name("platynui-wayland-compositor"), CompositorType::PlatynUi);
        assert_eq!(classify_binary_name("cage"), CompositorType::Unknown);
    }
}
