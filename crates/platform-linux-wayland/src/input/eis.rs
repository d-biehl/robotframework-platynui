//! EIS (Emulated Input Server) backend for direct EI protocol connections.
//!
//! Connects to an EIS socket (discovered via `$LIBEI_SOCKET` or well-known
//! paths in `$XDG_RUNTIME_DIR`) and uses the `reis` crate for the EI
//! protocol handshake and input event emission.
//!
//! Used as the primary input backend for third-party compositors
//! (Mutter/KWin via Portal, wlroots via direct socket) and as a fallback
//! for the `PlatynUI` compositor when the control socket is unavailable.

use std::io;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use enumflags2::BitFlags;
use platynui_core::platform::{
    KeyCode, KeyState, KeyboardError, KeyboardEvent, PlatformError, PlatformErrorKind, PointerButton, ScrollDelta,
};
use platynui_core::types::Point;
use platynui_xkb_util::{KeyAction, KeyCombination};
use reis::PendingRequestResult;
use reis::ei;
use reis::event::{DeviceCapability, EiEvent, EiEventConverter};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use tracing::{debug, info, warn};

use super::InputBackend;
use crate::capabilities::CompositorType;

/// Key code wrapper for the EIS backend.
///
/// Named keys and raw numeric codes use `Raw`, single-character lookups
/// use `Action` which preserves the full `KeyAction` (including compose
/// sequences and modifier requirements).
#[derive(Clone)]
enum EisKeyCode {
    /// A single evdev keycode (named keys like "enter", raw numeric codes).
    Raw(u32),
    /// A full key action from keymap lookup (character input with modifiers/compose).
    Action(KeyAction),
}

/// How long to wait for devices from the EIS server.
const DEVICE_TIMEOUT: Duration = Duration::from_secs(5);

/// EIS input backend — holds the connection and discovered devices.
pub(crate) struct EisBackend {
    inner: Mutex<EisState>,
}

struct EisState {
    connection: reis::event::Connection,
    context: ei::Context,
    converter: EiEventConverter,
    /// Keyboard device (first one with `Keyboard` capability).
    keyboard: Option<reis::event::Device>,
    /// Pointer-absolute device (first one with `PointerAbsolute` capability).
    pointer: Option<reis::event::Device>,
    /// Button device (first one with `Button` capability).
    button: Option<reis::event::Device>,
    /// Scroll device (first one with `Scroll` capability).
    scroll: Option<reis::event::Device>,
    /// Keymap lookup for text/named-key resolution.
    keymap_lookup: Option<platynui_xkb_util::KeymapLookup>,
    /// Raw keymap string received from the EIS server (needed to rebuild
    /// the lookup when the active layout group changes).
    keymap_string: Option<String>,
    /// Currently active layout group (from `KeyboardModifiers` events).
    active_group: u32,
    /// Monotonic timestamp epoch.
    epoch: Instant,
    /// Shadow position — updated on every `pointer_move_to`.
    last_position: Option<Point>,
    /// Which compositor we're connected to.
    compositor_type: CompositorType,
}

// SAFETY: `EisState` is only accessed behind a `Mutex`, guaranteeing exclusive
// single-threaded access. The non-`Send` `EiEventConverter` callbacks are never
// leaked outside the lock.
#[allow(unsafe_code)]
unsafe impl Send for EisState {}

impl EisBackend {
    /// Try to connect to an available EIS socket.
    ///
    /// Discovery order:
    /// 1. `$LIBEI_SOCKET` environment variable
    /// 2. Well-known paths in `$XDG_RUNTIME_DIR`
    pub(crate) fn connect(compositor_type: CompositorType) -> Result<Self, PlatformError> {
        let stream = connect_to_eis_socket()?;
        Self::from_stream(stream, compositor_type)
    }

    /// Create an EIS backend from an already-connected `UnixStream`.
    ///
    /// Used by the Portal backend which obtains the stream via
    /// `ConnectToEIS`.
    pub(crate) fn from_stream(stream: UnixStream, compositor_type: CompositorType) -> Result<Self, PlatformError> {
        let context = ei::Context::new(stream).map_err(|e| {
            PlatformError::new(PlatformErrorKind::InitializationFailed, format!("EIS context creation failed: {e}"))
        })?;

        let resp = reis::handshake::ei_handshake_blocking(&context, "platynui", ei::handshake::ContextType::Sender)
            .map_err(|e| {
                PlatformError::new(PlatformErrorKind::InitializationFailed, format!("EIS handshake failed: {e}"))
            })?;

        let mut converter = EiEventConverter::new(&context, resp);
        let connection = converter.connection().clone();

        dispatch_buffered(&context, &mut converter)?;

        info!("EIS handshake completed");

        let mut state = EisState {
            connection,
            context,
            converter,
            keyboard: None,
            pointer: None,
            button: None,
            scroll: None,
            keymap_lookup: None,
            keymap_string: None,
            active_group: 0,
            epoch: Instant::now(),
            last_position: None,
            compositor_type,
        };

        // Wait for devices and populate state.
        wait_for_devices(&mut state)?;

        Ok(Self { inner: Mutex::new(state) })
    }
}

impl InputBackend for EisBackend {
    fn name(&self) -> &'static str {
        "EIS"
    }

    fn key_to_code(&self, name: &str) -> Result<KeyCode, KeyboardError> {
        let lower = name.to_lowercase();
        if let Some(code) = named_key_code(&lower) {
            return Ok(KeyCode::new(EisKeyCode::Raw(code)));
        }

        if let Ok(code) = name.parse::<u32>() {
            return Ok(KeyCode::new(EisKeyCode::Raw(code)));
        }

        let mut guard = self.inner.lock().expect("EIS state mutex poisoned");
        // Check for layout changes from the compositor before resolving.
        poll_group_changes(&mut guard);
        if let Some(ref lookup) = guard.keymap_lookup {
            if let Some(ch) = name.chars().next()
                && name.chars().count() == 1
                && let Some(action) = lookup.lookup(ch)
            {
                return Ok(KeyCode::new(EisKeyCode::Action(*action)));
            }
            return Err(KeyboardError::UnsupportedKey(format!(
                "'{name}' is not available in the active keyboard layout '{}' (group {}, backend EIS)",
                lookup.layout_name(),
                guard.active_group,
            )));
        }

        Err(KeyboardError::UnsupportedKey(format!("'{name}' (no keymap available, backend EIS)")))
    }

    fn start_input(&self) -> Result<(), KeyboardError> {
        let guard = self.inner.lock().expect("EIS state mutex poisoned");
        if let Some(ref dev) = guard.keyboard {
            let serial = guard.connection.serial();
            dev.device().start_emulating(serial, 1);
            flush_blocking(&guard.connection, &guard.context).map_err(|e| KeyboardError::Platform(platform_err(&e)))?;
        }
        Ok(())
    }

    fn send_key_event(&self, event: KeyboardEvent) -> Result<(), KeyboardError> {
        let code = event.code.downcast_ref::<EisKeyCode>().ok_or(KeyboardError::NotReady)?;
        let guard = self.inner.lock().expect("EIS state mutex poisoned");
        let dev = guard.keyboard.as_ref().ok_or(KeyboardError::NotReady)?;
        let kbd = dev.interface::<ei::Keyboard>().ok_or(KeyboardError::NotReady)?;

        match code {
            EisKeyCode::Raw(evdev_code) => {
                let key_state = match event.state {
                    KeyState::Press => ei::keyboard::KeyState::Press,
                    KeyState::Release => ei::keyboard::KeyState::Released,
                };
                kbd.key(*evdev_code, key_state);
                let serial = guard.connection.serial();
                dev.device().frame(serial, timestamp_us(guard.epoch));
                flush_blocking(&guard.connection, &guard.context)
                    .map_err(|e| KeyboardError::Platform(platform_err(&e)))?;
            }
            EisKeyCode::Action(action) => {
                if event.state == KeyState::Release {
                    return Ok(());
                }
                match action {
                    KeyAction::Simple(combo) => {
                        send_eis_key_combo(&guard, dev, &kbd, combo)?;
                    }
                    KeyAction::Compose { dead_key, base_key } => {
                        send_eis_key_combo(&guard, dev, &kbd, dead_key)?;
                        send_eis_key_combo(&guard, dev, &kbd, base_key)?;
                    }
                }
            }
        }

        Ok(())
    }

    fn end_input(&self) -> Result<(), KeyboardError> {
        let guard = self.inner.lock().expect("EIS state mutex poisoned");
        if let Some(ref dev) = guard.keyboard {
            let serial = guard.connection.serial();
            dev.device().stop_emulating(serial);
            flush_blocking(&guard.connection, &guard.context).map_err(|e| KeyboardError::Platform(platform_err(&e)))?;
        }
        Ok(())
    }

    fn known_key_names(&self) -> Vec<String> {
        KEY_MAP.iter().map(|&(name, _)| name.to_string()).collect()
    }

    fn pointer_position(&self) -> Result<Point, PlatformError> {
        let mut guard = self.inner.lock().expect("EIS state mutex poisoned");
        if let Some(pos) = guard.last_position {
            return Ok(pos);
        }
        // No shadow position yet — try querying the compositor.
        let compositor_type = guard.compositor_type;
        if let Some(pos) = query_compositor_pointer_position(compositor_type) {
            guard.last_position = Some(pos);
            return Ok(pos);
        }
        Ok(Point::new(0.0, 0.0))
    }

    fn pointer_move_to(&self, point: Point) -> Result<(), PlatformError> {
        let mut guard = self.inner.lock().expect("EIS state mutex poisoned");
        let dev = guard.pointer.as_ref().ok_or_else(|| {
            PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "no EIS pointer-absolute device")
        })?;

        let abs = dev.interface::<ei::PointerAbsolute>().ok_or_else(|| {
            PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "device lacks PointerAbsolute interface")
        })?;

        let serial = guard.connection.serial();
        dev.device().start_emulating(serial, 1);
        #[expect(clippy::cast_possible_truncation, reason = "f64→f32 is intentional for EIS coords")]
        abs.motion_absolute(point.x() as f32, point.y() as f32);
        dev.device().frame(serial, timestamp_us(guard.epoch));
        dev.device().stop_emulating(serial);
        flush_blocking(&guard.connection, &guard.context).map_err(|e| platform_err(&e))?;
        guard.last_position = Some(point);

        Ok(())
    }

    fn pointer_press(&self, button: PointerButton) -> Result<(), PlatformError> {
        let guard = self.inner.lock().expect("EIS state mutex poisoned");
        let dev = guard
            .button
            .as_ref()
            .ok_or_else(|| PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "no EIS button device"))?;

        let btn = dev.interface::<ei::Button>().ok_or_else(|| {
            PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "device lacks Button interface")
        })?;

        let code = evdev_button_code(button);
        let serial = guard.connection.serial();
        dev.device().start_emulating(serial, 1);
        btn.button(code, ei::button::ButtonState::Press);
        dev.device().frame(serial, timestamp_us(guard.epoch));
        dev.device().stop_emulating(serial);
        flush_blocking(&guard.connection, &guard.context).map_err(|e| platform_err(&e))?;

        Ok(())
    }

    fn pointer_release(&self, button: PointerButton) -> Result<(), PlatformError> {
        let guard = self.inner.lock().expect("EIS state mutex poisoned");
        let dev = guard
            .button
            .as_ref()
            .ok_or_else(|| PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "no EIS button device"))?;

        let btn = dev.interface::<ei::Button>().ok_or_else(|| {
            PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "device lacks Button interface")
        })?;

        let code = evdev_button_code(button);
        let serial = guard.connection.serial();
        dev.device().start_emulating(serial, 1);
        btn.button(code, ei::button::ButtonState::Released);
        dev.device().frame(serial, timestamp_us(guard.epoch));
        dev.device().stop_emulating(serial);
        flush_blocking(&guard.connection, &guard.context).map_err(|e| platform_err(&e))?;

        Ok(())
    }

    fn pointer_scroll(&self, delta: ScrollDelta) -> Result<(), PlatformError> {
        let guard = self.inner.lock().expect("EIS state mutex poisoned");
        let dev = guard
            .scroll
            .as_ref()
            .ok_or_else(|| PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "no EIS scroll device"))?;

        let scroll = dev.interface::<ei::Scroll>().ok_or_else(|| {
            PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "device lacks Scroll interface")
        })?;

        let serial = guard.connection.serial();
        dev.device().start_emulating(serial, 1);
        #[expect(clippy::cast_possible_truncation, reason = "f64→f32 is intentional for EIS scroll")]
        scroll.scroll(delta.horizontal as f32, delta.vertical as f32);
        dev.device().frame(serial, timestamp_us(guard.epoch));
        dev.device().stop_emulating(serial);
        flush_blocking(&guard.connection, &guard.context).map_err(|e| platform_err(&e))?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
//  Connection helpers
// ---------------------------------------------------------------------------

/// Flush the EIS write buffer, blocking until the socket is writable if needed.
///
/// The `reis` crate sets the socket to non-blocking, so a plain `flush()` may
/// return `EAGAIN`/`WouldBlock` when the kernel send buffer is full (e.g.
/// rapid pointer-move sequences). This wrapper retries with a short `poll`
/// wait before giving up after 1 second.
fn flush_blocking(connection: &reis::event::Connection, context: &ei::Context) -> io::Result<()> {
    const MAX_RETRIES: u32 = 100;
    const POLL_TIMEOUT: Timespec = Timespec { tv_sec: 0, tv_nsec: 10_000_000 }; // 10 ms

    for _ in 0..MAX_RETRIES {
        match connection.flush() {
            Ok(()) => return Ok(()),
            Err(rustix::io::Errno::AGAIN) => {
                // Socket send buffer full — wait until it becomes writable.
                let mut pfd = [PollFd::new(context, PollFlags::OUT)];
                let _ = poll(&mut pfd, Some(&POLL_TIMEOUT));
            }
            Err(e) => return Err(e.into()),
        }
    }
    Err(io::Error::new(io::ErrorKind::TimedOut, "EIS flush timed out after repeated EAGAIN"))
}

/// Query the compositor for the current pointer position.
///
/// Returns `None` for all compositor types — position tracking is handled
/// by compositor-specific backends (`ControlSocket` for `PlatynUI`, future
/// extensions for GNOME/KDE). When EIS is used as a fallback, shadow
/// position tracking provides the position after the first `pointer_move_to`.
fn query_compositor_pointer_position(_compositor_type: CompositorType) -> Option<Point> {
    None
}

/// Try to connect to an EIS socket.
fn connect_to_eis_socket() -> Result<UnixStream, PlatformError> {
    // 1. Check $LIBEI_SOCKET
    if let Ok(path) = std::env::var("LIBEI_SOCKET") {
        let full_path = if std::path::Path::new(&path).is_relative() {
            let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_default();
            PathBuf::from(runtime_dir).join(&path)
        } else {
            PathBuf::from(&path)
        };

        return UnixStream::connect(&full_path).map_err(|e| {
            PlatformError::new(
                PlatformErrorKind::InitializationFailed,
                format!("failed to connect to EIS socket {}: {e}", full_path.display()),
            )
        });
    }

    // 2. Try well-known paths in $XDG_RUNTIME_DIR
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").map_err(|_| {
        PlatformError::new(
            PlatformErrorKind::CapabilityUnavailable,
            "no LIBEI_SOCKET set and XDG_RUNTIME_DIR unavailable",
        )
    })?;

    let candidates = ["eis-0", "eis-1"];
    for name in &candidates {
        let path = PathBuf::from(&runtime_dir).join(name);
        if path.exists() {
            match UnixStream::connect(&path) {
                Ok(stream) => {
                    debug!(path = %path.display(), "connected to EIS socket");
                    return Ok(stream);
                }
                Err(e) => {
                    debug!(path = %path.display(), error = %e, "EIS socket exists but connection failed");
                }
            }
        }
    }

    Err(PlatformError::new(
        PlatformErrorKind::CapabilityUnavailable,
        "no EIS socket found (set LIBEI_SOCKET or ensure compositor provides eis-0)",
    ))
}

/// Drain events already sitting in the context's internal read buffer.
fn dispatch_buffered(context: &ei::Context, converter: &mut EiEventConverter) -> Result<(), PlatformError> {
    while let Some(result) = context.pending_event() {
        match result {
            PendingRequestResult::Request(event) => {
                converter.handle_event(event).map_err(|e| {
                    PlatformError::new(PlatformErrorKind::OperationFailed, format!("EIS protocol error: {e}"))
                })?;
            }
            PendingRequestResult::ParseError(e) => {
                return Err(PlatformError::new(PlatformErrorKind::OperationFailed, format!("EIS parse error: {e}")));
            }
            PendingRequestResult::InvalidObject(_) => {}
        }
    }
    Ok(())
}

/// Poll-read-dispatch with a deadline.
fn try_read_and_dispatch(
    context: &ei::Context,
    converter: &mut EiEventConverter,
    deadline: Instant,
) -> io::Result<bool> {
    let mut pfd = [PollFd::new(context, PollFlags::IN)];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(false);
        }
        let timeout =
            Timespec { tv_sec: remaining.as_secs().cast_signed(), tv_nsec: i64::from(remaining.subsec_nanos()) };

        match poll(&mut pfd, Some(&timeout)) {
            Ok(0) => return Ok(false),
            Ok(_) => break,
            Err(rustix::io::Errno::INTR) => {}
            Err(e) => return Err(e.into()),
        }
    }

    context.read()?;
    dispatch_buffered(context, converter).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(true)
}

/// Wait for EIS devices to appear and populate the state.
fn wait_for_devices(state: &mut EisState) -> Result<(), PlatformError> {
    let hard_deadline = Instant::now() + DEVICE_TIMEOUT;
    let grace = Duration::from_millis(500);
    let mut grace_deadline: Option<Instant> = None;

    loop {
        while let Some(event) = state.converter.next_event() {
            match event {
                EiEvent::SeatAdded(ref seat) => {
                    seat.seat.bind_capabilities(BitFlags::all());
                    flush_blocking(&state.connection, &state.context).map_err(|e| {
                        PlatformError::new(PlatformErrorKind::OperationFailed, format!("EIS flush failed: {e}"))
                    })?;
                }
                EiEvent::DeviceResumed(ref dev) => {
                    debug!(device = ?dev.device.name(), "EIS device resumed");
                    assign_device(state, &dev.device);
                    grace_deadline = Some(Instant::now() + grace);
                }
                EiEvent::KeyboardModifiers(ref mods) => {
                    if mods.group != state.active_group {
                        debug!(old_group = state.active_group, new_group = mods.group, "EIS layout group changed");
                        update_active_group(state, mods.group);
                    }
                }
                EiEvent::Disconnected(ref disc) => {
                    warn!(reason = ?disc.reason, "EIS disconnected during device discovery");
                    break;
                }
                _ => {}
            }
        }

        let effective_deadline = match grace_deadline {
            Some(gd) => gd.min(hard_deadline),
            None => hard_deadline,
        };

        if Instant::now() >= effective_deadline {
            break;
        }

        match try_read_and_dispatch(&state.context, &mut state.converter, effective_deadline) {
            Ok(true) => {}
            Ok(false) => break,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => {
                return Err(PlatformError::new(PlatformErrorKind::OperationFailed, format!("EIS read error: {e}")));
            }
        }
    }

    let has_kbd = state.keyboard.is_some();
    let has_ptr = state.pointer.is_some();
    let has_btn = state.button.is_some();
    info!(keyboard = has_kbd, pointer = has_ptr, button = has_btn, "EIS devices discovered");

    Ok(())
}

/// Assign a device to the appropriate slot(s) based on capabilities.
fn assign_device(state: &mut EisState, device: &reis::event::Device) {
    if device.has_capability(DeviceCapability::Keyboard) && state.keyboard.is_none() {
        if state.keymap_string.is_none() {
            state.keymap_string = read_device_keymap_string(device);
        }
        if state.keymap_lookup.is_none() {
            state.keymap_lookup =
                state.keymap_string.as_deref().and_then(|s| build_keymap_lookup(s, state.active_group));
        }
        state.keyboard = Some(device.clone());
    }
    if device.has_capability(DeviceCapability::PointerAbsolute) && state.pointer.is_none() {
        state.pointer = Some(device.clone());
    }
    if device.has_capability(DeviceCapability::Button) && state.button.is_none() {
        state.button = Some(device.clone());
    }
    if device.has_capability(DeviceCapability::Scroll) && state.scroll.is_none() {
        state.scroll = Some(device.clone());
    }
}

/// Try to build a keymap lookup from the device's keymap FD.
fn read_device_keymap_string(device: &reis::event::Device) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};

    let keymap = device.keymap()?;
    let size = keymap.size as usize;
    if size == 0 {
        return None;
    }

    let mut file = std::fs::File::from(keymap.fd.try_clone().ok()?);
    file.seek(SeekFrom::Start(0)).ok()?;
    let mut buf = vec![0u8; size];
    file.read_exact(&mut buf).ok()?;

    let text = if buf.last() == Some(&0) { &buf[..buf.len() - 1] } else { &buf[..] };
    std::str::from_utf8(text).ok().map(str::to_string)
}

/// Build a `KeymapLookup` for the given layout group from a keymap string.
fn build_keymap_lookup(keymap_str: &str, group: u32) -> Option<platynui_xkb_util::KeymapLookup> {
    match platynui_xkb_util::KeymapLookup::from_string_for_layout(keymap_str, group) {
        Ok(lookup) => {
            info!(entries = lookup.len(), group, "using EIS device keymap for key resolution");
            Some(lookup)
        }
        Err(err) => {
            warn!(%err, "failed to parse EIS device keymap");
            None
        }
    }
}

/// Update the active layout group and rebuild the keymap lookup.
fn update_active_group(state: &mut EisState, new_group: u32) {
    state.active_group = new_group;
    if let Some(ref keymap_str) = state.keymap_string {
        state.keymap_lookup = build_keymap_lookup(keymap_str, new_group);
    }
}

/// Non-blocking poll for `KeyboardModifiers` events from the compositor.
///
/// Drains any pending events and updates the active layout group if the
/// compositor signals a change (e.g. the user switched keyboard layout).
fn poll_group_changes(state: &mut EisState) {
    // Non-blocking read: check if data is available on the EIS socket.
    let mut pfd = [PollFd::new(&state.context, PollFlags::IN)];
    let poll_result = poll(&mut pfd, Some(&Timespec { tv_sec: 0, tv_nsec: 0 }));
    if poll_result.is_ok_and(|n| n > 0) {
        let _ = state.context.read();
        let _ = dispatch_buffered(&state.context, &mut state.converter);
    }

    while let Some(event) = state.converter.next_event() {
        if let EiEvent::KeyboardModifiers(ref mods) = event
            && mods.group != state.active_group
        {
            debug!(old_group = state.active_group, new_group = mods.group, "EIS layout group changed");
            update_active_group(state, mods.group);
        }
    }
}

// ---------------------------------------------------------------------------
//  Utilities
// ---------------------------------------------------------------------------

fn platform_err(e: &impl std::fmt::Display) -> PlatformError {
    PlatformError::new(PlatformErrorKind::OperationFailed, format!("EIS error: {e}"))
}

/// Send a complete key combination via EIS including modifier presses/releases.
///
/// Presses modifiers → presses key → releases key → releases modifiers.
/// Each step is a separate EIS frame with flush.
fn send_eis_key_combo(
    state: &EisState,
    dev: &reis::event::Device,
    kbd: &ei::Keyboard,
    combo: &KeyCombination,
) -> Result<(), KeyboardError> {
    let evdev_code = combo.evdev_keycode();
    let mod_keys = combo.modifier_keycodes();
    let serial = state.connection.serial();

    if !mod_keys.is_empty() {
        for &m in &mod_keys {
            kbd.key(m, ei::keyboard::KeyState::Press);
        }
        dev.device().frame(serial, timestamp_us(state.epoch));
        flush_blocking(&state.connection, &state.context).map_err(|e| KeyboardError::Platform(platform_err(&e)))?;
    }

    kbd.key(evdev_code, ei::keyboard::KeyState::Press);
    dev.device().frame(serial, timestamp_us(state.epoch));
    flush_blocking(&state.connection, &state.context).map_err(|e| KeyboardError::Platform(platform_err(&e)))?;

    kbd.key(evdev_code, ei::keyboard::KeyState::Released);
    dev.device().frame(serial, timestamp_us(state.epoch));
    flush_blocking(&state.connection, &state.context).map_err(|e| KeyboardError::Platform(platform_err(&e)))?;

    if !mod_keys.is_empty() {
        for &m in mod_keys.iter().rev() {
            kbd.key(m, ei::keyboard::KeyState::Released);
        }
        dev.device().frame(serial, timestamp_us(state.epoch));
        flush_blocking(&state.connection, &state.context).map_err(|e| KeyboardError::Platform(platform_err(&e)))?;
    }

    Ok(())
}

/// Returns a monotonic timestamp in microseconds.
#[expect(clippy::cast_possible_truncation, reason = "timestamp won't overflow u64 in practice")]
fn timestamp_us(epoch: Instant) -> u64 {
    epoch.elapsed().as_micros() as u64
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

// ---------------------------------------------------------------------------
//  Named key → evdev keycode table
// ---------------------------------------------------------------------------

/// All recognized key names and their evdev keycodes.
pub(crate) const KEY_MAP: &[(&str, u32)] = &[
    // Letters
    ("a", 30),
    ("b", 48),
    ("c", 46),
    ("d", 32),
    ("e", 18),
    ("f", 33),
    ("g", 34),
    ("h", 35),
    ("i", 23),
    ("j", 36),
    ("k", 37),
    ("l", 38),
    ("m", 50),
    ("n", 49),
    ("o", 24),
    ("p", 25),
    ("q", 16),
    ("r", 19),
    ("s", 31),
    ("t", 20),
    ("u", 22),
    ("v", 47),
    ("w", 17),
    ("x", 45),
    ("y", 21),
    ("z", 44),
    // Number row
    ("1", 2),
    ("2", 3),
    ("3", 4),
    ("4", 5),
    ("5", 6),
    ("6", 7),
    ("7", 8),
    ("8", 9),
    ("9", 10),
    ("0", 11),
    // F-keys
    ("f1", 59),
    ("f2", 60),
    ("f3", 61),
    ("f4", 62),
    ("f5", 63),
    ("f6", 64),
    ("f7", 65),
    ("f8", 66),
    ("f9", 67),
    ("f10", 68),
    ("f11", 87),
    ("f12", 88),
    // Special keys
    ("esc", 1),
    ("escape", 1),
    ("enter", 28),
    ("return", 28),
    ("tab", 15),
    ("space", 57),
    ("backspace", 14),
    ("delete", 111),
    ("insert", 110),
    ("home", 102),
    ("end", 107),
    ("pageup", 104),
    ("pagedown", 109),
    // Arrow keys
    ("up", 103),
    ("down", 108),
    ("left", 105),
    ("right", 106),
    // Modifiers
    ("shift", 42),
    ("leftshift", 42),
    ("lshift", 42),
    ("rightshift", 54),
    ("rshift", 54),
    ("ctrl", 29),
    ("control", 29),
    ("leftctrl", 29),
    ("lctrl", 29),
    ("rightctrl", 97),
    ("rctrl", 97),
    ("alt", 56),
    ("leftalt", 56),
    ("lalt", 56),
    ("rightalt", 100),
    ("ralt", 100),
    ("altgr", 100),
    ("super", 125),
    ("meta", 125),
    ("win", 125),
    ("leftmeta", 125),
    ("lmeta", 125),
    ("rightmeta", 126),
    ("rmeta", 126),
    // Punctuation & symbols
    ("minus", 12),
    ("equal", 13),
    ("leftbrace", 26),
    ("rightbrace", 27),
    ("semicolon", 39),
    ("apostrophe", 40),
    ("grave", 41),
    ("backslash", 43),
    ("comma", 51),
    ("dot", 52),
    ("period", 52),
    ("slash", 53),
    // Lock keys
    ("capslock", 58),
    ("numlock", 69),
    ("scrolllock", 70),
    // Misc
    ("print", 99),
    ("printscreen", 99),
    ("pause", 119),
    ("menu", 127),
    ("compose", 127),
];

/// Look up a key name → evdev keycode.
pub(crate) fn named_key_code(name: &str) -> Option<u32> {
    KEY_MAP.iter().find(|&&(alias, _)| alias == name).map(|&(_, code)| code)
}
