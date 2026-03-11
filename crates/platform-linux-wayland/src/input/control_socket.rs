//! Control-Socket input backend for the `PlatynUI` compositor.
//!
//! Keeps a persistent Unix socket connection to the compositor's control
//! socket and sends JSON commands for keyboard, pointer, and scroll input.
//! This avoids the EIS protocol overhead when the compositor is our own.
//!
//! The control socket is discovered via `$PLATYNUI_CONTROL_SOCKET` or
//! the well-known path `$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY.control`.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use platynui_core::platform::{
    KeyCode, KeyState, KeyboardError, KeyboardEvent, PlatformError, PlatformErrorKind, PointerButton, ScrollDelta,
};
use platynui_core::types::Point;
use platynui_xkb_util::{KeyAction, KeyCombination};
use tracing::{info, warn};

use super::InputBackend;
use super::eis;

/// Key code wrapper for the control socket backend.
///
/// Named keys and raw numeric codes use `Raw`, single-character lookups
/// use `Action` which preserves the full `KeyAction` (including compose
/// sequences and modifier requirements).
#[derive(Clone)]
enum ControlKeyCode {
    /// A single evdev keycode (named keys like "enter", raw numeric codes).
    Raw(u32),
    /// A full key action from keymap lookup (character input with modifiers/compose).
    Action(KeyAction),
}

/// Control-socket input backend — persistent connection to `PlatynUI` compositor.
pub(crate) struct ControlSocketBackend {
    inner: Mutex<ControlSocketState>,
}

struct ControlSocketState {
    writer: UnixStream,
    reader: BufReader<UnixStream>,
    keymap_lookup: Option<platynui_xkb_util::KeymapLookup>,
    last_position: Option<Point>,
}

impl ControlSocketState {
    /// Send a fire-and-forget command (write + flush, no response read).
    ///
    /// Used for input injection commands where the compositor does not
    /// send a response, avoiding round-trip latency.
    fn send_event(&mut self, command: &str) -> Result<(), PlatformError> {
        writeln!(self.writer, "{command}").map_err(|e| {
            PlatformError::new(PlatformErrorKind::OperationFailed, format!("control socket write failed: {e}"))
        })?;
        self.writer.flush().map_err(|e| {
            PlatformError::new(PlatformErrorKind::OperationFailed, format!("control socket flush failed: {e}"))
        })?;
        Ok(())
    }

    /// Send a JSON command and read the JSON response.
    fn send_command(&mut self, command: &str) -> Result<serde_json::Value, PlatformError> {
        writeln!(self.writer, "{command}").map_err(|e| {
            PlatformError::new(PlatformErrorKind::OperationFailed, format!("control socket write failed: {e}"))
        })?;
        self.writer.flush().map_err(|e| {
            PlatformError::new(PlatformErrorKind::OperationFailed, format!("control socket flush failed: {e}"))
        })?;

        let mut line = String::new();
        self.reader.read_line(&mut line).map_err(|e| {
            PlatformError::new(PlatformErrorKind::OperationFailed, format!("control socket read failed: {e}"))
        })?;

        let value: serde_json::Value = serde_json::from_str(line.trim()).map_err(|e| {
            PlatformError::new(PlatformErrorKind::OperationFailed, format!("control socket invalid JSON: {e}"))
        })?;

        if value.get("status").and_then(serde_json::Value::as_str) != Some("ok") {
            let msg = value.get("message").and_then(serde_json::Value::as_str).unwrap_or("unknown error");
            return Err(PlatformError::new(PlatformErrorKind::OperationFailed, format!("control socket error: {msg}")));
        }

        Ok(value)
    }

    /// Send a complete key combination including modifier presses/releases.
    ///
    /// Presses modifiers → presses key → releases key → releases modifiers.
    fn send_key_combo(&mut self, combo: &KeyCombination) -> Result<(), PlatformError> {
        let evdev_code = combo.evdev_keycode();
        let mod_keys = combo.modifier_keycodes();

        for &m in &mod_keys {
            self.send_event(&format!(r#"{{"command":"key_event","key":{m},"state":"press"}}"#))?;
        }

        self.send_event(&format!(r#"{{"command":"key_event","key":{evdev_code},"state":"press"}}"#))?;
        self.send_event(&format!(r#"{{"command":"key_event","key":{evdev_code},"state":"release"}}"#))?;

        for &m in mod_keys.iter().rev() {
            self.send_event(&format!(r#"{{"command":"key_event","key":{m},"state":"release"}}"#))?;
        }

        Ok(())
    }

    /// Fetch the XKB keymap from the compositor and build a lookup table.
    fn fetch_keymap(&mut self) -> Option<platynui_xkb_util::KeymapLookup> {
        let response = self.send_command(r#"{"command":"get_keymap"}"#).ok()?;
        let keymap_str = response.get("keymap")?.as_str()?;
        match platynui_xkb_util::KeymapLookup::from_string(keymap_str) {
            Ok(lookup) => {
                info!(entries = lookup.len(), "using compositor keymap for key resolution");
                Some(lookup)
            }
            Err(err) => {
                warn!(%err, "failed to parse compositor keymap");
                None
            }
        }
    }
}

impl ControlSocketBackend {
    /// Connect to the `PlatynUI` compositor control socket.
    ///
    /// Discovers the socket path, establishes a persistent connection,
    /// and fetches the compositor's XKB keymap for key-name resolution.
    pub(crate) fn connect() -> Result<Self, PlatformError> {
        let socket_path = discover_control_socket_path().ok_or_else(|| {
            PlatformError::new(
                PlatformErrorKind::CapabilityUnavailable,
                "control socket path not found (set PLATYNUI_CONTROL_SOCKET or ensure WAYLAND_DISPLAY is set)",
            )
        })?;

        let stream = UnixStream::connect(&socket_path).map_err(|e| {
            PlatformError::new(
                PlatformErrorKind::InitializationFailed,
                format!("failed to connect to control socket {}: {e}", socket_path.display()),
            )
        })?;

        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
        stream.set_write_timeout(Some(Duration::from_secs(5))).ok();

        let reader_stream = stream.try_clone().map_err(|e| {
            PlatformError::new(PlatformErrorKind::InitializationFailed, format!("failed to clone stream: {e}"))
        })?;

        let mut state = ControlSocketState {
            writer: stream,
            reader: BufReader::new(reader_stream),
            keymap_lookup: None,
            last_position: None,
        };

        state.keymap_lookup = state.fetch_keymap();

        info!(
            path = %socket_path.display(),
            has_keymap = state.keymap_lookup.is_some(),
            "control socket backend connected",
        );

        Ok(Self { inner: Mutex::new(state) })
    }
}

impl InputBackend for ControlSocketBackend {
    fn name(&self) -> &'static str {
        "ControlSocket"
    }

    fn key_to_code(&self, name: &str) -> Result<KeyCode, KeyboardError> {
        let lower = name.to_lowercase();
        if let Some(code) = eis::named_key_code(&lower) {
            return Ok(KeyCode::new(ControlKeyCode::Raw(code)));
        }

        if let Ok(code) = name.parse::<u32>() {
            return Ok(KeyCode::new(ControlKeyCode::Raw(code)));
        }

        let guard = self.inner.lock().expect("control socket state mutex poisoned");
        if let Some(ref lookup) = guard.keymap_lookup {
            if let Some(ch) = name.chars().next()
                && name.chars().count() == 1
                && let Some(action) = lookup.lookup(ch)
            {
                return Ok(KeyCode::new(ControlKeyCode::Action(*action)));
            }
            return Err(KeyboardError::UnsupportedKey(format!(
                "'{name}' is not available in the active keyboard layout '{}' (backend control-socket)",
                lookup.layout_name(),
            )));
        }

        Err(KeyboardError::UnsupportedKey(format!("{name} (no keymap available, backend control-socket)")))
    }

    fn send_key_event(&self, event: KeyboardEvent) -> Result<(), KeyboardError> {
        let code = event.code.downcast_ref::<ControlKeyCode>().ok_or(KeyboardError::NotReady)?;
        let mut guard = self.inner.lock().expect("control socket state mutex poisoned");

        match code {
            ControlKeyCode::Raw(evdev_code) => {
                let state_str = match event.state {
                    KeyState::Press => "press",
                    KeyState::Release => "release",
                };
                guard
                    .send_event(&format!(r#"{{"command":"key_event","key":{evdev_code},"state":"{state_str}"}}"#))
                    .map_err(|e| KeyboardError::Platform(platform_err(&e)))?;
            }
            ControlKeyCode::Action(action) => {
                if event.state == KeyState::Release {
                    return Ok(());
                }
                match action {
                    KeyAction::Simple(combo) => {
                        guard.send_key_combo(combo).map_err(KeyboardError::Platform)?;
                    }
                    KeyAction::Compose { dead_key, base_key } => {
                        guard.send_key_combo(dead_key).map_err(KeyboardError::Platform)?;
                        guard.send_key_combo(base_key).map_err(KeyboardError::Platform)?;
                    }
                }
            }
        }

        Ok(())
    }

    fn known_key_names(&self) -> Vec<String> {
        eis::KEY_MAP.iter().map(|&(name, _)| name.to_string()).collect()
    }

    fn pointer_position(&self) -> Result<Point, PlatformError> {
        let mut guard = self.inner.lock().expect("control socket state mutex poisoned");
        let response = guard.send_command(r#"{"command":"get_pointer_position"}"#)?;
        if let Some(x) = response.get("x").and_then(serde_json::Value::as_f64)
            && let Some(y) = response.get("y").and_then(serde_json::Value::as_f64)
        {
            let pos = Point::new(x, y);
            guard.last_position = Some(pos);
            return Ok(pos);
        }
        Ok(guard.last_position.unwrap_or_else(|| Point::new(0.0, 0.0)))
    }

    fn pointer_move_to(&self, point: Point) -> Result<(), PlatformError> {
        let mut guard = self.inner.lock().expect("control socket state mutex poisoned");
        guard.send_event(&format!(r#"{{"command":"pointer_move_to","x":{},"y":{}}}"#, point.x(), point.y()))?;
        guard.last_position = Some(point);
        Ok(())
    }

    fn pointer_press(&self, button: PointerButton) -> Result<(), PlatformError> {
        let code = evdev_button_code(button);
        let mut guard = self.inner.lock().expect("control socket state mutex poisoned");
        guard.send_event(&format!(r#"{{"command":"pointer_button","button":{code},"state":"press"}}"#))?;
        Ok(())
    }

    fn pointer_release(&self, button: PointerButton) -> Result<(), PlatformError> {
        let code = evdev_button_code(button);
        let mut guard = self.inner.lock().expect("control socket state mutex poisoned");
        guard.send_event(&format!(r#"{{"command":"pointer_button","button":{code},"state":"release"}}"#))?;
        Ok(())
    }

    fn pointer_scroll(&self, delta: ScrollDelta) -> Result<(), PlatformError> {
        let mut guard = self.inner.lock().expect("control socket state mutex poisoned");
        guard.send_event(&format!(
            r#"{{"command":"pointer_scroll","dx":{},"dy":{}}}"#,
            delta.horizontal, delta.vertical
        ))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
//  Helpers
// ---------------------------------------------------------------------------

/// Discover the compositor control socket path.
fn discover_control_socket_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("PLATYNUI_CONTROL_SOCKET") {
        return Some(PathBuf::from(path));
    }
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").ok()?;
    let wayland_display = std::env::var("WAYLAND_DISPLAY").ok()?;
    Some(PathBuf::from(runtime_dir).join(format!("{wayland_display}.control")))
}

fn platform_err(e: &impl std::fmt::Display) -> PlatformError {
    PlatformError::new(PlatformErrorKind::OperationFailed, format!("control socket error: {e}"))
}

/// Map a `PointerButton` to an evdev button code.
fn evdev_button_code(button: PointerButton) -> u32 {
    match button {
        PointerButton::Left => 0x110,   // BTN_LEFT
        PointerButton::Right => 0x111,  // BTN_RIGHT
        PointerButton::Middle => 0x112, // BTN_MIDDLE
        PointerButton::Other(code) => u32::from(code),
    }
}
