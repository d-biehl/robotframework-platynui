//! Input backends for Wayland.
//!
//! Provides keyboard and pointer input via compositor-specific backends:
//!
//! - **EIS** (Emulated Input Server): Direct EI protocol connection for
//!   `PlatynUI` compositor and compositors that expose an EIS socket.
//! - **Portal**: XDG Desktop Portal `RemoteDesktop` → `ConnectToEIS`.
//!   Primary path for Mutter and `KWin` (handles consent & token persistence).
//! - **Virtual Input**: `zwlr-virtual-pointer-v1` + `zwlr-virtual-keyboard-v1`
//!   for wlroots compositors without EIS.
//!
//! Backend selection is based on `CompositorType` detected at init time,
//! with runtime fallbacks if the preferred backend is unavailable.

pub mod control_socket;
pub mod eis;
pub mod portal;
pub mod virtual_input;

use std::sync::Mutex;

use platynui_core::platform::{
    KeyCode, KeyboardDevice, KeyboardError, KeyboardEvent, PlatformError, PointerButton, PointerDevice, ScrollDelta,
};
use platynui_core::types::Point;
use tracing::{debug, info, warn};

use crate::capabilities::CompositorType;

/// Internal trait for input backend implementations.
///
/// Each backend (EIS, Portal, virtual-input) implements this trait.
/// The selected backend is stored in `BACKEND` and accessed by
/// `WaylandKeyboardDevice` and `WaylandPointerDevice`. Its name for logging
/// is [`Candidate::name`], which also names a backend that failed to connect.
pub(crate) trait InputBackend: Send + Sync {
    // -- Keyboard --

    /// Convert a key name to a backend-specific key code.
    fn key_to_code(&self, name: &str) -> Result<KeyCode, KeyboardError>;

    /// Signal the start of an input sequence (e.g. `start_emulating`).
    fn start_input(&self) -> Result<(), KeyboardError> {
        Ok(())
    }

    /// Send a key press or release event.
    fn send_key_event(&self, event: KeyboardEvent) -> Result<(), KeyboardError>;

    /// Signal the end of an input sequence (e.g. `stop_emulating`).
    fn end_input(&self) -> Result<(), KeyboardError> {
        Ok(())
    }

    /// List of known key names for this backend.
    fn known_key_names(&self) -> Vec<String> {
        Vec::new()
    }

    // -- Pointer --

    /// Get current pointer position (not available on all backends).
    fn pointer_position(&self) -> Result<Point, PlatformError>;

    /// Move pointer to absolute position.
    fn pointer_move_to(&self, point: Point) -> Result<(), PlatformError>;

    /// Press a pointer button.
    fn pointer_press(&self, button: PointerButton) -> Result<(), PlatformError>;

    /// Release a pointer button.
    fn pointer_release(&self, button: PointerButton) -> Result<(), PlatformError>;

    /// Scroll by the given delta.
    fn pointer_scroll(&self, delta: ScrollDelta) -> Result<(), PlatformError>;
}

// ---------------------------------------------------------------------------
//  Global backend storage
// ---------------------------------------------------------------------------

static BACKEND: Mutex<Option<Box<dyn InputBackend>>> = Mutex::new(None);

/// Select and initialize the best available input backend based on the
/// detected compositor type.
///
/// Called during platform initialization. Tries the backends of
/// [`candidates`] in order, keeps the first that connects and logs the
/// decision (see [`select`]).
pub(crate) fn initialize(compositor: CompositorType) {
    let backend = select(compositor, candidates(compositor), |candidate| connect(candidate, compositor));

    let mut guard = BACKEND.lock().expect("input backend mutex poisoned");
    *guard = backend;
}

/// Shut down the input backend and release resources.
///
/// Kept `pub` alongside the sibling teardown functions
/// (`connection::clear_global`, `desktop::clear_outputs`) for the deferred
/// per-instance teardown; `create_wayland_bundle` does not call it yet.
///
/// # Panics
///
/// Panics if the internal mutex is poisoned.
pub fn shutdown() {
    let mut guard = BACKEND.lock().expect("input backend mutex poisoned");
    *guard = None;
}

/// Try to access the active input backend, returning an error if none is available.
fn try_with_backend<F, R, E>(f: F, make_err: impl FnOnce() -> E) -> Result<R, E>
where
    F: FnOnce(&dyn InputBackend) -> Result<R, E>,
{
    let guard = BACKEND.lock().expect("input backend mutex poisoned");
    match guard.as_deref() {
        Some(backend) => f(backend),
        None => Err(make_err()),
    }
}

// ---------------------------------------------------------------------------
//  Backend selection
// ---------------------------------------------------------------------------

/// An input backend that the selection can try.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Candidate {
    ControlSocket,
    Eis,
    Portal,
    VirtualInput,
}

impl Candidate {
    /// The backend's name in log records.
    const fn name(self) -> &'static str {
        match self {
            Self::ControlSocket => "ControlSocket",
            Self::Eis => "EIS",
            Self::Portal => "Portal",
            Self::VirtualInput => "virtual-input (wlr)",
        }
    }
}

/// The backends to try for `compositor`, in priority order:
/// - `PlatynUI` → control socket, then EIS, then Portal
/// - Mutter / `KWin` → Portal `RemoteDesktop` → EIS, then direct EIS
/// - Sway / Hyprland / Wlroots / Unknown → EIS, then Portal, then virtual-input
fn candidates(compositor: CompositorType) -> &'static [Candidate] {
    match compositor {
        CompositorType::PlatynUi => &[Candidate::ControlSocket, Candidate::Eis, Candidate::Portal],
        CompositorType::Mutter | CompositorType::KWin => &[Candidate::Portal, Candidate::Eis],
        CompositorType::Sway | CompositorType::Hyprland | CompositorType::Wlroots | CompositorType::Unknown => {
            &[Candidate::Eis, Candidate::Portal, Candidate::VirtualInput]
        }
    }
}

/// Connect one backend, returning its failure.
fn connect(candidate: Candidate, compositor: CompositorType) -> Result<Box<dyn InputBackend>, PlatformError> {
    match candidate {
        Candidate::ControlSocket => try_control_socket(),
        Candidate::Eis => try_eis(compositor),
        Candidate::Portal => try_portal(compositor),
        Candidate::VirtualInput => try_virtual_input(),
    }
}

/// Try `candidates` in order with `connect` and return the first backend that
/// connects.
///
/// Logs the decision once it is made: the chosen backend with the compositor
/// type that fixed the order of the attempts, and at debug each backend
/// rejected before it, with its reason. When none connects, one warning names
/// each backend with its reason.
fn select<B>(
    compositor: CompositorType,
    candidates: &[Candidate],
    mut connect: impl FnMut(Candidate) -> Result<B, PlatformError>,
) -> Option<B> {
    let mut rejected = Vec::new();
    for &candidate in candidates {
        match connect(candidate) {
            Ok(backend) => {
                for Rejected(rejected, error) in &rejected {
                    debug!(backend = rejected.name(), error = %error, "input backend unavailable");
                }
                info!(backend = candidate.name(), %compositor, "input backend initialized");
                return Some(backend);
            }
            Err(error) => rejected.push(Rejected(candidate, error)),
        }
    }
    warn!(
        %compositor,
        backends = %Rejections(&rejected),
        "no Wayland input backend could be initialized; keyboard and pointer input will fail"
    );
    None
}

/// A backend that failed to connect, with its failure.
struct Rejected(Candidate, PlatformError);

/// Each rejected backend with its reason, for the warning's `backends` field.
struct Rejections<'a>(&'a [Rejected]);

impl std::fmt::Display for Rejections<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (index, Rejected(backend, error)) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str("; ")?;
            }
            write!(f, "{}: {error}", backend.name())?;
        }
        Ok(())
    }
}

fn try_eis(compositor: CompositorType) -> Result<Box<dyn InputBackend>, PlatformError> {
    Ok(Box::new(eis::EisBackend::connect(compositor)?))
}

fn try_portal(compositor: CompositorType) -> Result<Box<dyn InputBackend>, PlatformError> {
    Ok(Box::new(portal::PortalBackend::connect(compositor)?))
}

fn try_virtual_input() -> Result<Box<dyn InputBackend>, PlatformError> {
    Ok(Box::new(virtual_input::VirtualInputBackend::connect()?))
}

fn try_control_socket() -> Result<Box<dyn InputBackend>, PlatformError> {
    Ok(Box::new(control_socket::ControlSocketBackend::connect()?))
}

// ---------------------------------------------------------------------------
//  Public device types — delegate to the active backend
// ---------------------------------------------------------------------------

/// Wayland keyboard device that delegates to the active input backend.
pub struct WaylandKeyboardDevice;

impl KeyboardDevice for WaylandKeyboardDevice {
    fn key_to_code(&self, name: &str) -> Result<KeyCode, KeyboardError> {
        try_with_backend(|b| b.key_to_code(name), || KeyboardError::NotReady)
    }

    fn start_input(&self) -> Result<(), KeyboardError> {
        try_with_backend(|b| b.start_input(), || KeyboardError::NotReady)
    }

    fn send_key_event(&self, event: KeyboardEvent) -> Result<(), KeyboardError> {
        try_with_backend(|b| b.send_key_event(event), || KeyboardError::NotReady)
    }

    fn end_input(&self) -> Result<(), KeyboardError> {
        try_with_backend(|b| b.end_input(), || KeyboardError::NotReady)
    }

    fn known_key_names(&self) -> Vec<String> {
        let guard = BACKEND.lock().expect("input backend mutex poisoned");
        match guard.as_deref() {
            Some(backend) => backend.known_key_names(),
            None => Vec::new(),
        }
    }
}

/// Wayland pointer device that delegates to the active input backend.
pub struct WaylandPointerDevice;

impl PointerDevice for WaylandPointerDevice {
    fn position(&self) -> Result<Point, PlatformError> {
        try_with_backend(
            |b| b.pointer_position(),
            || PlatformError::CapabilityUnavailable { capability: "input backend", details: None },
        )
    }

    fn move_to(&self, point: Point) -> Result<(), PlatformError> {
        try_with_backend(
            |b| b.pointer_move_to(point),
            || PlatformError::CapabilityUnavailable { capability: "input backend", details: None },
        )
    }

    fn press(&self, button: PointerButton) -> Result<(), PlatformError> {
        try_with_backend(
            |b| b.pointer_press(button),
            || PlatformError::CapabilityUnavailable { capability: "input backend", details: None },
        )
    }

    fn release(&self, button: PointerButton) -> Result<(), PlatformError> {
        try_with_backend(
            |b| b.pointer_release(button),
            || PlatformError::CapabilityUnavailable { capability: "input backend", details: None },
        )
    }

    fn scroll(&self, delta: ScrollDelta) -> Result<(), PlatformError> {
        try_with_backend(
            |b| b.pointer_scroll(delta),
            || PlatformError::CapabilityUnavailable { capability: "input backend", details: None },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Run `f` with its tracing output, down to debug, captured.
    fn logged<R>(f: impl FnOnce() -> R) -> (R, String) {
        #[derive(Clone)]
        struct Captured(Arc<Mutex<Vec<u8>>>);
        impl std::io::Write for Captured {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().expect("log buffer").extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let buffer = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = Captured(Arc::clone(&buffer));
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_ansi(false)
            .without_time()
            .with_writer(move || writer.clone())
            .finish();
        let result = tracing::subscriber::with_default(subscriber, f);
        let log = String::from_utf8(buffer.lock().expect("log buffer").clone()).expect("utf-8 log");
        (result, log)
    }

    fn lines_at<'a>(log: &'a str, level: &str) -> Vec<&'a str> {
        log.lines().filter(|line| line.trim_start().starts_with(level)).collect()
    }

    fn refused(candidate: Candidate) -> PlatformError {
        let details = match candidate {
            Candidate::ControlSocket => "/run/dead.control: Connection refused",
            Candidate::Eis => "set LIBEI_SOCKET or ensure the compositor provides eis-0",
            Candidate::Portal => "no session bus",
            Candidate::VirtualInput => "not available on this compositor",
        };
        PlatformError::InitializationFailed { component: "test backend", details: Some(details.into()) }
    }

    #[test]
    fn the_order_of_the_attempts_follows_the_compositor() {
        use Candidate::{ControlSocket, Eis, Portal, VirtualInput};
        assert_eq!(candidates(CompositorType::PlatynUi), [ControlSocket, Eis, Portal]);
        assert_eq!(candidates(CompositorType::Mutter), [Portal, Eis]);
        assert_eq!(candidates(CompositorType::KWin), [Portal, Eis]);
        for compositor in
            [CompositorType::Sway, CompositorType::Hyprland, CompositorType::Wlroots, CompositorType::Unknown]
        {
            assert_eq!(candidates(compositor), [Eis, Portal, VirtualInput], "{compositor}");
        }
    }

    /// A later backend is chosen: the record of the chosen one names the
    /// compositor, each one rejected before it is at debug with its reason,
    /// and nothing is warned.
    #[test]
    fn a_later_backend_names_the_compositor_and_records_the_rejected_ones_at_debug() {
        let mut tried = Vec::new();
        let (chosen, log) = logged(|| {
            select(CompositorType::PlatynUi, candidates(CompositorType::PlatynUi), |candidate| {
                tried.push(candidate);
                if candidate == Candidate::Eis { Ok(candidate) } else { Err(refused(candidate)) }
            })
        });

        assert_eq!(chosen, Some(Candidate::Eis));
        assert_eq!(tried, [Candidate::ControlSocket, Candidate::Eis], "the attempts stop at the first success");

        assert!(lines_at(&log, "WARN").is_empty(), "nothing is warned\n{log}");
        let infos = lines_at(&log, "INFO");
        assert_eq!(infos.len(), 1, "one record of the chosen backend\n{log}");
        for expected in ["input backend initialized", r#"backend="EIS""#, "compositor=PlatynUI"] {
            assert!(infos[0].contains(expected), "the record names `{expected}`: {}", infos[0]);
        }
        let debugs = lines_at(&log, "DEBUG");
        assert_eq!(debugs.len(), 1, "one debug record per rejected backend\n{log}");
        for expected in
            ["input backend unavailable", r#"backend="ControlSocket""#, "/run/dead.control: Connection refused"]
        {
            assert!(debugs[0].contains(expected), "the rejection names `{expected}`: {}", debugs[0]);
        }
    }

    /// No backend connects: one warning names the compositor and each backend
    /// with its reason, and says what stops working.
    #[test]
    fn no_backend_gives_one_warning_naming_each_backend_with_its_reason() {
        let (chosen, log) = logged(|| {
            select(CompositorType::PlatynUi, candidates(CompositorType::PlatynUi), |candidate| {
                Err::<Candidate, _>(refused(candidate))
            })
        });

        assert_eq!(chosen, None);
        assert!(lines_at(&log, "INFO").is_empty(), "no backend is reported as initialized\n{log}");
        assert!(lines_at(&log, "DEBUG").is_empty(), "the warning is the one record\n{log}");
        let warnings = lines_at(&log, "WARN");
        assert_eq!(warnings.len(), 1, "exactly one warning\n{log}");
        let warning = warnings[0];
        for expected in [
            "no Wayland input backend could be initialized; keyboard and pointer input will fail",
            "compositor=PlatynUI",
            "ControlSocket: platform initialization failed for test backend: /run/dead.control: Connection refused",
            "; EIS: platform initialization failed for test backend: set LIBEI_SOCKET",
            "; Portal: platform initialization failed for test backend: no session bus",
        ] {
            assert!(warning.contains(expected), "the warning names `{expected}`: {warning}");
        }
    }
}
