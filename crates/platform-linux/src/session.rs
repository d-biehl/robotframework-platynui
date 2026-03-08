//! Linux session type detection.
//!
//! Determines at runtime whether we are on an X11 or Wayland session so that
//! the mediator can delegate to the correct sub-platform crate.

use platynui_core::platform::{PlatformError, PlatformErrorKind};
use std::sync::Mutex;

/// The type of display session detected at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionType {
    X11,
    Wayland,
}

/// Detect the current session type using environment variables.
///
/// Priority chain:
/// 1. `$XDG_SESSION_TYPE` → `"wayland"` | `"x11"` (most authoritative)
/// 2. `$WAYLAND_DISPLAY` set → Wayland
/// 3. `$DISPLAY` set → X11
/// 4. None → error
///
/// `XWayland` sets **both** `$DISPLAY` and `$WAYLAND_DISPLAY`, but
/// `$XDG_SESSION_TYPE=wayland` — hence step 1 has priority.
fn detect_session_type() -> Result<SessionType, PlatformError> {
    // 1. Authoritative: $XDG_SESSION_TYPE
    if let Ok(session_type) = std::env::var("XDG_SESSION_TYPE") {
        match session_type.to_lowercase().as_str() {
            "wayland" => return Ok(SessionType::Wayland),
            "x11" => return Ok(SessionType::X11),
            // Other values (e.g. "tty", "mir") fall through.
            _ => {}
        }
    }

    // 2. $WAYLAND_DISPLAY present → Wayland
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return Ok(SessionType::Wayland);
    }

    // 3. $DISPLAY present → X11
    if std::env::var_os("DISPLAY").is_some() {
        return Ok(SessionType::X11);
    }

    // 4. Cannot determine
    Err(PlatformError::new(
        PlatformErrorKind::UnsupportedPlatform,
        "cannot detect Linux session type: neither $XDG_SESSION_TYPE, $WAYLAND_DISPLAY, nor $DISPLAY is set",
    ))
}

static SESSION: Mutex<Option<SessionType>> = Mutex::new(None);

/// Return the detected session type, caching the result for the process lifetime.
///
/// # Errors
///
/// Returns `PlatformError` if the session type cannot be determined from
/// environment variables.
pub fn session_type() -> Result<SessionType, PlatformError> {
    // Lock cannot be poisoned by our code (no panics inside the critical
    // section), so an expect is safe here.
    #[allow(clippy::missing_panics_doc)]
    let mut guard = SESSION.lock().expect("session type lock poisoned");
    if let Some(st) = *guard {
        return Ok(st);
    }
    let st = detect_session_type()?;
    tracing::info!(session = ?st, "Linux session type detected");
    *guard = Some(st);
    Ok(st)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::sync::Mutex;

    // Environment variables are process-global — serialise all tests that
    // manipulate them so they don't interfere with each other.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Helper: clear all session-relevant env vars, run `f`, then restore.
    #[allow(unsafe_code)]
    fn with_env<F: FnOnce()>(xdg: Option<&str>, wayland_display: Option<&str>, display: Option<&str>, f: F) {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");

        // SAFETY: All tests that touch env vars are serialised behind
        // `ENV_LOCK`, so no concurrent mutation can occur.
        unsafe {
            // Save originals
            let orig_xdg = std::env::var_os("XDG_SESSION_TYPE");
            let orig_wayland = std::env::var_os("WAYLAND_DISPLAY");
            let orig_display = std::env::var_os("DISPLAY");

            // Set / remove
            match xdg {
                Some(v) => std::env::set_var("XDG_SESSION_TYPE", v),
                None => std::env::remove_var("XDG_SESSION_TYPE"),
            }
            match wayland_display {
                Some(v) => std::env::set_var("WAYLAND_DISPLAY", v),
                None => std::env::remove_var("WAYLAND_DISPLAY"),
            }
            match display {
                Some(v) => std::env::set_var("DISPLAY", v),
                None => std::env::remove_var("DISPLAY"),
            }

            f();

            // Restore originals
            match orig_xdg {
                Some(v) => std::env::set_var("XDG_SESSION_TYPE", v),
                None => std::env::remove_var("XDG_SESSION_TYPE"),
            }
            match orig_wayland {
                Some(v) => std::env::set_var("WAYLAND_DISPLAY", v),
                None => std::env::remove_var("WAYLAND_DISPLAY"),
            }
            match orig_display {
                Some(v) => std::env::set_var("DISPLAY", v),
                None => std::env::remove_var("DISPLAY"),
            }
        }
    }

    // -- Priority 1: $XDG_SESSION_TYPE --

    #[rstest]
    #[case("x11", SessionType::X11)]
    #[case("X11", SessionType::X11)]
    #[case("wayland", SessionType::Wayland)]
    #[case("Wayland", SessionType::Wayland)]
    #[case("WAYLAND", SessionType::Wayland)]
    fn xdg_session_type_is_authoritative(#[case] value: &str, #[case] expected: SessionType) {
        with_env(Some(value), None, None, || {
            assert_eq!(detect_session_type().unwrap(), expected);
        });
    }

    #[test]
    fn xdg_session_type_overrides_wayland_display() {
        // Even when $WAYLAND_DISPLAY is set, $XDG_SESSION_TYPE=x11 wins.
        with_env(Some("x11"), Some("wayland-0"), None, || {
            assert_eq!(detect_session_type().unwrap(), SessionType::X11);
        });
    }

    #[test]
    fn xdg_session_type_overrides_display() {
        // Even when $DISPLAY is set, $XDG_SESSION_TYPE=wayland wins.
        with_env(Some("wayland"), None, Some(":0"), || {
            assert_eq!(detect_session_type().unwrap(), SessionType::Wayland);
        });
    }

    #[rstest]
    #[case("tty")]
    #[case("mir")]
    #[case("unrecognised")]
    fn xdg_unknown_falls_through_to_wayland_display(#[case] value: &str) {
        with_env(Some(value), Some("wayland-0"), None, || {
            assert_eq!(detect_session_type().unwrap(), SessionType::Wayland);
        });
    }

    #[rstest]
    #[case("tty")]
    #[case("mir")]
    fn xdg_unknown_falls_through_to_display(#[case] value: &str) {
        with_env(Some(value), None, Some(":0"), || {
            assert_eq!(detect_session_type().unwrap(), SessionType::X11);
        });
    }

    // -- Priority 2: $WAYLAND_DISPLAY --

    #[test]
    fn wayland_display_without_xdg() {
        with_env(None, Some("wayland-0"), None, || {
            assert_eq!(detect_session_type().unwrap(), SessionType::Wayland);
        });
    }

    #[test]
    fn wayland_display_beats_display() {
        // Both set, no XDG — $WAYLAND_DISPLAY has higher priority.
        with_env(None, Some("wayland-0"), Some(":0"), || {
            assert_eq!(detect_session_type().unwrap(), SessionType::Wayland);
        });
    }

    // -- Priority 3: $DISPLAY --

    #[test]
    fn display_without_xdg_or_wayland() {
        with_env(None, None, Some(":0"), || {
            assert_eq!(detect_session_type().unwrap(), SessionType::X11);
        });
    }

    // -- Priority 4: nothing set → error --

    #[test]
    fn no_env_vars_returns_error() {
        with_env(None, None, None, || {
            let err = detect_session_type().unwrap_err();
            assert!(matches!(err, PlatformError::UnsupportedPlatform { .. }));
        });
    }

    // -- Caching via session_type() --

    #[test]
    fn detect_returns_result() {
        // Smoke test — the actual result depends on the test environment.
        let _ = detect_session_type();
    }
}
