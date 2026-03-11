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
    KeyCode, KeyboardDevice, KeyboardError, KeyboardEvent, PlatformError, PlatformErrorKind, PointerButton,
    PointerDevice, ScrollDelta,
};
use platynui_core::types::Point;
use tracing::{debug, info, warn};

use crate::capabilities::CompositorType;

/// Internal trait for input backend implementations.
///
/// Each backend (EIS, Portal, virtual-input) implements this trait.
/// The selected backend is stored in `BACKEND` and accessed by
/// `WaylandKeyboardDevice` and `WaylandPointerDevice`.
pub(crate) trait InputBackend: Send + Sync {
    /// Human-readable name for logging.
    fn name(&self) -> &'static str;

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
/// Called during platform initialization. Tries backends in priority order:
/// - `PlatynUI` / wlroots with EIS socket → direct EIS
/// - Mutter / `KWin` → Portal `RemoteDesktop` → EIS
/// - Sway / Hyprland / Wlroots / Unknown → try EIS, then Portal, then virtual-input
pub(crate) fn initialize(compositor: CompositorType) {
    let backend: Option<Box<dyn InputBackend>> = match compositor {
        CompositorType::PlatynUi => try_control_socket_then_eis(compositor),
        CompositorType::Mutter | CompositorType::KWin => try_portal_then_eis(compositor),
        CompositorType::Sway | CompositorType::Hyprland | CompositorType::Wlroots | CompositorType::Unknown => {
            try_eis_then_portal_then_virtual(compositor)
        }
    };

    if let Some(b) = &backend {
        info!(backend = b.name(), "input backend initialized");
    } else {
        warn!("no input backend available — keyboard/pointer will not work");
    }

    let mut guard = BACKEND.lock().expect("input backend mutex poisoned");
    *guard = backend;
}

/// Shut down the input backend and release resources.
pub(crate) fn shutdown() {
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
//  Backend selection helpers
// ---------------------------------------------------------------------------

fn try_control_socket_then_eis(compositor: CompositorType) -> Option<Box<dyn InputBackend>> {
    if let Some(b) = try_control_socket() {
        return Some(b);
    }
    try_eis_then_portal(compositor)
}

fn try_eis_then_portal(compositor: CompositorType) -> Option<Box<dyn InputBackend>> {
    if let Some(b) = try_eis(compositor) {
        return Some(b);
    }
    try_portal(compositor)
}

fn try_portal_then_eis(compositor: CompositorType) -> Option<Box<dyn InputBackend>> {
    if let Some(b) = try_portal(compositor) {
        return Some(b);
    }
    try_eis(compositor)
}

fn try_eis_then_portal_then_virtual(compositor: CompositorType) -> Option<Box<dyn InputBackend>> {
    if let Some(b) = try_eis(compositor) {
        return Some(b);
    }
    if let Some(b) = try_portal(compositor) {
        return Some(b);
    }
    try_virtual_input()
}

fn try_eis(compositor: CompositorType) -> Option<Box<dyn InputBackend>> {
    match eis::EisBackend::connect(compositor) {
        Ok(b) => {
            debug!("EIS input backend available");
            Some(Box::new(b))
        }
        Err(e) => {
            debug!(error = %e, "EIS input backend unavailable");
            None
        }
    }
}

fn try_portal(compositor: CompositorType) -> Option<Box<dyn InputBackend>> {
    match portal::PortalBackend::connect(compositor) {
        Ok(b) => {
            debug!("Portal input backend available");
            Some(Box::new(b))
        }
        Err(e) => {
            debug!(error = %e, "Portal input backend unavailable");
            None
        }
    }
}

fn try_virtual_input() -> Option<Box<dyn InputBackend>> {
    match virtual_input::VirtualInputBackend::connect() {
        Ok(b) => {
            debug!("virtual-input backend available");
            Some(Box::new(b))
        }
        Err(e) => {
            debug!(error = %e, "virtual-input backend unavailable");
            None
        }
    }
}

fn try_control_socket() -> Option<Box<dyn InputBackend>> {
    match control_socket::ControlSocketBackend::connect() {
        Ok(b) => {
            debug!("control socket input backend available");
            Some(Box::new(b))
        }
        Err(e) => {
            debug!(error = %e, "control socket input backend unavailable");
            None
        }
    }
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
            || PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "no input backend available"),
        )
    }

    fn move_to(&self, point: Point) -> Result<(), PlatformError> {
        try_with_backend(
            |b| b.pointer_move_to(point),
            || PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "no input backend available"),
        )
    }

    fn press(&self, button: PointerButton) -> Result<(), PlatformError> {
        try_with_backend(
            |b| b.pointer_press(button),
            || PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "no input backend available"),
        )
    }

    fn release(&self, button: PointerButton) -> Result<(), PlatformError> {
        try_with_backend(
            |b| b.pointer_release(button),
            || PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "no input backend available"),
        )
    }

    fn scroll(&self, delta: ScrollDelta) -> Result<(), PlatformError> {
        try_with_backend(
            |b| b.pointer_scroll(delta),
            || PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "no input backend available"),
        )
    }
}
