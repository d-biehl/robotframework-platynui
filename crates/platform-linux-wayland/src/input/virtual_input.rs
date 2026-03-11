//! Virtual-input backend using `zwlr-virtual-pointer-v1` and
//! `zwlr-virtual-keyboard-v1` protocols.
//!
//! These are wlroots-specific unstable protocols that allow clients to
//! inject pointer and keyboard events directly via the Wayland protocol.
//! This is the fallback for wlroots-based compositors (Sway, Hyprland)
//! that do not expose an EIS socket.
//!
//! **Limitation:** These protocols require binding globals on the Wayland
//! display connection. The current implementation establishes a separate
//! connection specifically for virtual input, since the main connection's
//! event queue is managed by the background dispatch thread.

use std::io::Write;
use std::os::fd::AsFd;
use std::sync::Mutex;
use std::time::Instant;

use platynui_core::platform::{
    KeyCode, KeyState, KeyboardError, KeyboardEvent, PlatformError, PlatformErrorKind, PointerButton, ScrollDelta,
};
use platynui_core::types::Point;
use platynui_xkb_util::xkb;
use platynui_xkb_util::{KeyAction, KeyCombination, KeymapLookup};
use tracing::{debug, info, warn};
use wayland_client::protocol::wl_keyboard::{self, WlKeyboard};
use wayland_client::protocol::wl_output::{self, WlOutput};
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, globals::GlobalListContents};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;
use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1;
use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_v1::{self, ZwlrVirtualPointerV1};

use super::InputBackend;

/// Virtual-input backend using wlr protocols.
pub(crate) struct VirtualInputBackend {
    inner: Mutex<VirtualInputState>,
}

struct VirtualInputState {
    conn: Connection,
    /// Held for ownership — dropping the queue invalidates Wayland objects.
    _event_queue: EventQueue<VirtualInputDispatch>,
    /// Held for ownership — dispatch state must outlive the queue.
    _dispatch: VirtualInputDispatch,
    virtual_pointer: Option<ZwlrVirtualPointerV1>,
    virtual_keyboard: Option<ZwpVirtualKeyboardV1>,
    keymap_lookup: Option<KeymapLookup>,
    /// Local XKB state used to track modifier state.  The
    /// `zwp_virtual_keyboard_v1` protocol requires the client to send
    /// explicit `modifiers` events — the compositor does NOT derive
    /// modifier state from key events (wlroots sets `update_state = false`).
    xkb_state: Option<SendState>,
    epoch: Instant,
    /// Shadow position — updated on every `pointer_move_to`.
    last_position: Option<Point>,
    /// Output dimensions used as extent for `motion_absolute`.
    output_width: u32,
    output_height: u32,
}

/// Wrapper to make `xkb::State` `Send`.
///
/// `xkb::State` wraps a `*mut xkb_state` and is `!Send` by default.
/// xkbcommon's state objects are safe to move between threads as long as
/// only one thread accesses them at a time, which is guaranteed by the
/// enclosing `Mutex<VirtualInputState>`.
struct SendState(xkb::State);

// SAFETY: xkb_state is safe to send between threads; exclusive access
// is guaranteed by the Mutex in VirtualInputBackend.
#[allow(unsafe_code)]
unsafe impl Send for SendState {}

/// Dispatch state for the virtual-input connection.
struct VirtualInputDispatch {
    pointer_manager: Option<ZwlrVirtualPointerManagerV1>,
    keyboard_manager: Option<ZwpVirtualKeyboardManagerV1>,
    seat: Option<WlSeat>,
    /// Held alive so the compositor delivers its `wl_keyboard::keymap` event
    /// during the roundtrip.  Dropped after we've read `seat_keymap`.
    seat_keyboard: Option<WlKeyboard>,
    /// Output dimensions collected from `wl_output.mode` events.
    output_width: u32,
    output_height: u32,
    /// XKB keymap string received from the compositor's seat keyboard.
    seat_keymap: Option<String>,
    /// Active layout group received from the compositor's seat keyboard.
    seat_group: u32,
}

impl VirtualInputBackend {
    /// Try to connect and bind the virtual-input protocols.
    pub(crate) fn connect() -> Result<Self, PlatformError> {
        let conn = Connection::connect_to_env().map_err(|e| {
            PlatformError::new(
                PlatformErrorKind::InitializationFailed,
                format!("Wayland connection for virtual-input failed: {e}"),
            )
        })?;

        let (globals, mut eq) =
            wayland_client::globals::registry_queue_init::<VirtualInputDispatch>(&conn).map_err(|e| {
                PlatformError::new(
                    PlatformErrorKind::InitializationFailed,
                    format!("Wayland registry init failed: {e}"),
                )
            })?;

        let qh = eq.handle();
        let mut dispatch = bind_globals(&globals, &qh);

        // Roundtrip to finish binding.
        eq.roundtrip(&mut dispatch).map_err(|e| {
            PlatformError::new(PlatformErrorKind::InitializationFailed, format!("Wayland roundtrip failed: {e}"))
        })?;

        // Create the virtual pointer if the manager is available.
        let virtual_pointer = if let Some(ref mgr) = dispatch.pointer_manager {
            let vp = if let Some(ref seat) = dispatch.seat {
                mgr.create_virtual_pointer(Some(seat), &qh, ())
            } else {
                mgr.create_virtual_pointer(None, &qh, ())
            };
            debug!("created virtual pointer");
            Some(vp)
        } else {
            warn!("zwlr_virtual_pointer_manager_v1 not available");
            None
        };

        // Create the virtual keyboard and upload the system XKB keymap.
        let (virtual_keyboard, keymap_lookup, xkb_state) =
            if let (Some(mgr), Some(seat)) = (&dispatch.keyboard_manager, &dispatch.seat) {
                let vk = mgr.create_virtual_keyboard(seat, &qh, ());
                debug!("created virtual keyboard");

                match load_and_upload_keymap(&vk, dispatch.seat_keymap.as_deref(), dispatch.seat_group) {
                    Ok((lookup, xkb_state)) => (Some(vk), Some(lookup), Some(SendState(xkb_state))),
                    Err(e) => {
                        warn!(%e, "failed to load/upload XKB keymap, keyboard will use raw codes only");
                        (Some(vk), None, None)
                    }
                }
            } else {
                if dispatch.keyboard_manager.is_none() {
                    warn!("zwp_virtual_keyboard_manager_v1 not available");
                }
                (None, None, None)
            };

        // Flush to ensure the keymap upload reaches the compositor.
        conn.flush().map_err(|e| {
            PlatformError::new(PlatformErrorKind::InitializationFailed, format!("Wayland flush failed: {e}"))
        })?;

        // Second roundtrip: ensure the compositor has processed the keymap
        // before any key events are sent.
        eq.roundtrip(&mut dispatch).map_err(|e| {
            PlatformError::new(PlatformErrorKind::InitializationFailed, format!("Wayland roundtrip failed: {e}"))
        })?;

        // The seat keyboard is no longer needed — drop it to stop receiving
        // keyboard events we won't process.
        dispatch.seat_keyboard = None;

        let has_pointer = virtual_pointer.is_some();
        let has_keyboard = virtual_keyboard.is_some();
        let (output_width, output_height) = if dispatch.output_width > 0 && dispatch.output_height > 0 {
            (dispatch.output_width, dispatch.output_height)
        } else {
            warn!("no output dimensions from wl_output, defaulting to 1920x1080");
            (1920, 1080)
        };
        info!(
            pointer = has_pointer,
            keyboard = has_keyboard,
            has_seat_keymap = dispatch.seat_keymap.is_some(),
            output_width,
            output_height,
            "virtual-input backend initialized"
        );

        if !has_pointer && !has_keyboard {
            return Err(PlatformError::new(
                PlatformErrorKind::CapabilityUnavailable,
                "no virtual-input protocols available on this compositor",
            ));
        }

        Ok(Self {
            inner: Mutex::new(VirtualInputState {
                conn,
                _event_queue: eq,
                _dispatch: dispatch,
                virtual_pointer,
                virtual_keyboard,
                keymap_lookup,
                xkb_state,
                epoch: Instant::now(),
                last_position: None,
                output_width,
                output_height,
            }),
        })
    }
}

impl InputBackend for VirtualInputBackend {
    fn name(&self) -> &'static str {
        "virtual-input (wlr)"
    }

    fn key_to_code(&self, name: &str) -> Result<KeyCode, KeyboardError> {
        let lower = name.to_lowercase();
        if let Some(code) = super::eis::named_key_code(&lower) {
            return Ok(KeyCode::new(VirtualKeyCode::Raw(code)));
        }

        if let Ok(code) = name.parse::<u32>() {
            return Ok(KeyCode::new(VirtualKeyCode::Raw(code)));
        }

        let guard = self.inner.lock().expect("virtual-input mutex poisoned");
        if let Some(ref lookup) = guard.keymap_lookup {
            if let Some(ch) = name.chars().next()
                && name.chars().count() == 1
                && let Some(action) = lookup.lookup(ch)
            {
                return Ok(KeyCode::new(VirtualKeyCode::Action(*action)));
            }
            return Err(KeyboardError::UnsupportedKey(format!("{name} (active layout: '{}')", lookup.layout_name(),)));
        }

        Err(KeyboardError::UnsupportedKey(format!("{name} (no keymap available, backend virtual-input)")))
    }

    fn send_key_event(&self, event: KeyboardEvent) -> Result<(), KeyboardError> {
        let code = event.code.downcast_ref::<VirtualKeyCode>().ok_or(KeyboardError::NotReady)?;
        let mut guard = self.inner.lock().expect("virtual-input mutex poisoned");
        let state = &mut *guard;

        if state.virtual_keyboard.is_none() {
            return Err(KeyboardError::NotReady);
        }

        match code {
            VirtualKeyCode::Raw(evdev_code) => {
                let wl_state = match event.state {
                    KeyState::Press => 1,   // WL_KEYBOARD_KEY_STATE_PRESSED
                    KeyState::Release => 0, // WL_KEYBOARD_KEY_STATE_RELEASED
                };
                let time = timestamp_ms(state.epoch);
                let vk = state.virtual_keyboard.as_ref().expect("checked above");
                vk.key(time, *evdev_code, wl_state);
                // Update local XKB state and send modifier state to compositor.
                send_modifier_state(state, *evdev_code, event.state)?;
            }
            VirtualKeyCode::Action(action) => {
                // Actions are full press+release sequences; only fire on Press.
                if event.state == KeyState::Release {
                    return Ok(());
                }
                match action {
                    KeyAction::Simple(combo) => {
                        send_virtual_key_combo(state, combo)?;
                    }
                    KeyAction::Compose { dead_key, base_key } => {
                        send_virtual_key_combo(state, dead_key)?;
                        send_virtual_key_combo(state, base_key)?;
                    }
                }
            }
        }

        Ok(())
    }

    fn known_key_names(&self) -> Vec<String> {
        super::eis::KEY_MAP.iter().map(|&(name, _)| name.to_string()).collect()
    }

    fn pointer_position(&self) -> Result<Point, PlatformError> {
        let guard = self.inner.lock().expect("virtual-input mutex poisoned");
        Ok(guard.last_position.unwrap_or(Point::new(0.0, 0.0)))
    }

    fn pointer_move_to(&self, point: Point) -> Result<(), PlatformError> {
        let mut guard = self.inner.lock().expect("virtual-input mutex poisoned");
        let state = &mut *guard;

        let vp = state.virtual_pointer.as_ref().ok_or_else(|| {
            PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "no virtual pointer available")
        })?;

        let time = timestamp_ms(state.epoch);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "display coordinates are non-negative, f64→u32 for wlr protocol"
        )]
        let (x, y) = (point.x().max(0.0) as u32, point.y().max(0.0) as u32);
        // The wlr protocol interprets (x, y) as fractions of (x_extent, y_extent).
        // Pass the output dimensions so pixel coordinates map 1:1.
        vp.motion_absolute(time, x, y, state.output_width, state.output_height);
        vp.frame();
        state.conn.flush().map_err(|e| {
            PlatformError::new(PlatformErrorKind::OperationFailed, format!("Wayland flush failed: {e}"))
        })?;
        state.last_position = Some(point);

        Ok(())
    }

    fn pointer_press(&self, button: PointerButton) -> Result<(), PlatformError> {
        let mut guard = self.inner.lock().expect("virtual-input mutex poisoned");
        let state = &mut *guard;

        let vp = state.virtual_pointer.as_ref().ok_or_else(|| {
            PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "no virtual pointer available")
        })?;

        let time = timestamp_ms(state.epoch);
        let code = evdev_button_code(button);
        vp.button(time, code, wayland_client::protocol::wl_pointer::ButtonState::Pressed);
        vp.frame();
        state.conn.flush().map_err(|e| {
            PlatformError::new(PlatformErrorKind::OperationFailed, format!("Wayland flush failed: {e}"))
        })?;

        Ok(())
    }

    fn pointer_release(&self, button: PointerButton) -> Result<(), PlatformError> {
        let mut guard = self.inner.lock().expect("virtual-input mutex poisoned");
        let state = &mut *guard;

        let vp = state.virtual_pointer.as_ref().ok_or_else(|| {
            PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "no virtual pointer available")
        })?;

        let time = timestamp_ms(state.epoch);
        let code = evdev_button_code(button);
        vp.button(time, code, wayland_client::protocol::wl_pointer::ButtonState::Released);
        vp.frame();
        state.conn.flush().map_err(|e| {
            PlatformError::new(PlatformErrorKind::OperationFailed, format!("Wayland flush failed: {e}"))
        })?;

        Ok(())
    }

    fn pointer_scroll(&self, delta: ScrollDelta) -> Result<(), PlatformError> {
        let mut guard = self.inner.lock().expect("virtual-input mutex poisoned");
        let state = &mut *guard;

        let vp = state.virtual_pointer.as_ref().ok_or_else(|| {
            PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "no virtual pointer available")
        })?;

        let time = timestamp_ms(state.epoch);
        // wl_pointer::Axis: 0 = vertical, 1 = horizontal
        if delta.vertical.abs() > f64::EPSILON {
            vp.axis(time, wayland_client::protocol::wl_pointer::Axis::VerticalScroll, delta.vertical);
        }
        if delta.horizontal.abs() > f64::EPSILON {
            vp.axis(time, wayland_client::protocol::wl_pointer::Axis::HorizontalScroll, delta.horizontal);
        }
        vp.frame();
        state.conn.flush().map_err(|e| {
            PlatformError::new(PlatformErrorKind::OperationFailed, format!("Wayland flush failed: {e}"))
        })?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
//  Dispatch implementations (minimal — we don't need to handle events)
// ---------------------------------------------------------------------------

impl Dispatch<WlRegistry, GlobalListContents> for VirtualInputDispatch {
    fn event(
        _state: &mut Self,
        _proxy: &WlRegistry,
        _event: wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrVirtualPointerManagerV1, ()> for VirtualInputDispatch {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrVirtualPointerManagerV1,
        _event: <ZwlrVirtualPointerManagerV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrVirtualPointerV1, ()> for VirtualInputDispatch {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrVirtualPointerV1,
        _event: zwlr_virtual_pointer_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for VirtualInputDispatch {
    fn event(
        _state: &mut Self,
        _proxy: &WlSeat,
        _event: wayland_client::protocol::wl_seat::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlKeyboard, ()> for VirtualInputDispatch {
    fn event(
        state: &mut Self,
        _proxy: &WlKeyboard,
        event: wl_keyboard::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // We care about keymap and modifiers events — the keymap tells us
        // the compositor's active XKB keymap, and the modifiers event tells us
        // the currently active layout group.
        if let wl_keyboard::Event::Keymap { format, fd, size } = event {
            if format != wayland_client::WEnum::Value(wl_keyboard::KeymapFormat::XkbV1) {
                return;
            }
            match read_keymap_from_fd(fd, size) {
                Some(km) => {
                    debug!(size, "received seat keymap from compositor");
                    state.seat_keymap = Some(km);
                }
                None => {
                    warn!("failed to read seat keymap from compositor FD");
                }
            }
        } else if let wl_keyboard::Event::Modifiers {
            mods_depressed: _, mods_latched: _, mods_locked: _, group, ..
        } = event
        {
            debug!(group, "received seat keyboard group from compositor");
            state.seat_group = group;
        }
    }
}

impl Dispatch<ZwpVirtualKeyboardManagerV1, ()> for VirtualInputDispatch {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpVirtualKeyboardManagerV1,
        _event: <ZwpVirtualKeyboardManagerV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpVirtualKeyboardV1, ()> for VirtualInputDispatch {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpVirtualKeyboardV1,
        _event: <ZwpVirtualKeyboardV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlOutput, ()> for VirtualInputDispatch {
    fn event(
        state: &mut Self,
        _proxy: &WlOutput,
        event: wl_output::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Collect the current mode's resolution (the one with `Current` flag).
        if let wl_output::Event::Mode { flags, width, height, .. } = event
            && let (Ok(w), Ok(h)) = (u32::try_from(width), u32::try_from(height))
        {
            let is_current = flags.into_result().is_ok_and(|f| f.contains(wl_output::Mode::Current));
            if is_current {
                debug!(width, height, "wl_output: current mode");
                state.output_width = w;
                state.output_height = h;
            }
        }
    }
}

// ---------------------------------------------------------------------------
//  Utilities
// ---------------------------------------------------------------------------

/// Key code representation for the virtual keyboard backend.
#[derive(Debug, Clone)]
enum VirtualKeyCode {
    /// Raw evdev keycode — sent directly via the protocol.
    Raw(u32),
    /// Resolved key action (may include modifier presses).
    Action(KeyAction),
}

/// Bind all required Wayland globals for virtual input.
fn bind_globals(
    globals: &wayland_client::globals::GlobalList,
    qh: &QueueHandle<VirtualInputDispatch>,
) -> VirtualInputDispatch {
    let mut dispatch = VirtualInputDispatch {
        pointer_manager: None,
        keyboard_manager: None,
        seat: None,
        seat_keyboard: None,
        output_width: 0,
        output_height: 0,
        seat_keymap: None,
        seat_group: 0,
    };

    if let Ok(mgr) = globals.bind::<ZwlrVirtualPointerManagerV1, _, _>(qh, 1..=2, ()) {
        debug!("bound zwlr_virtual_pointer_manager_v1");
        dispatch.pointer_manager = Some(mgr);
    }

    if let Ok(mgr) = globals.bind::<ZwpVirtualKeyboardManagerV1, _, _>(qh, 1..=1, ()) {
        debug!("bound zwp_virtual_keyboard_manager_v1");
        dispatch.keyboard_manager = Some(mgr);
    }

    // Bind wl_seat (needed to create virtual pointers/keyboards).
    for global in globals.contents().clone_list() {
        if global.interface == "wl_seat" {
            let seat = globals.registry().bind::<WlSeat, _, _>(global.name, global.version.min(8), qh, ());
            dispatch.seat = Some(seat);
            break;
        }
    }

    // Bind wl_output to discover the output resolution.
    for global in globals.contents().clone_list() {
        if global.interface == "wl_output" {
            let _output = globals.registry().bind::<WlOutput, _, _>(global.name, global.version.min(4), qh, ());
            break;
        }
    }

    // Get a keyboard from the seat so the compositor sends us its keymap.
    // The proxy MUST stay alive until after the roundtrip — if it is dropped,
    // wayland-client marks it "dead" and silently discards the keymap event.
    if let Some(ref seat) = dispatch.seat {
        dispatch.seat_keyboard = Some(seat.get_keyboard(qh, ()));
    }

    dispatch
}

/// Load or reuse a keymap, upload it to the virtual keyboard, and return
/// a `KeymapLookup` for character → keycode resolution.
///
/// If `seat_keymap` is provided (obtained from the compositor's `wl_keyboard`),
/// it is used directly.  Otherwise we fall back to the system default XKB
/// keymap via `new_from_names`.
fn load_and_upload_keymap(
    vk: &ZwpVirtualKeyboardV1,
    seat_keymap: Option<&str>,
    seat_group: u32,
) -> Result<(KeymapLookup, xkb::State), PlatformError> {
    let keymap_string = if let Some(km) = seat_keymap {
        debug!("using compositor seat keymap for virtual keyboard");
        km.to_string()
    } else {
        debug!("no seat keymap available, falling back to system default");
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_names(&context, "", "", "", "", None, xkb::KEYMAP_COMPILE_NO_FLAGS)
            .ok_or_else(|| {
                PlatformError::new(PlatformErrorKind::InitializationFailed, "failed to create default XKB keymap")
            })?;
        keymap.get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1)
    };

    // The Wayland protocol (wlroots) expects a null-terminated keymap string.
    // wlroots does `xkb_keymap_new_from_buffer(ctx, map, size - 1, ...)`,
    // so `size` must include the trailing null byte.
    let mut keymap_bytes = keymap_string.as_bytes().to_vec();
    keymap_bytes.push(0);

    let memfd = rustix::fs::memfd_create("xkb-keymap", rustix::fs::MemfdFlags::CLOEXEC).map_err(|e| {
        PlatformError::new(PlatformErrorKind::InitializationFailed, format!("memfd_create failed: {e}"))
    })?;
    let mut file = std::fs::File::from(memfd);
    file.write_all(&keymap_bytes).map_err(|e| {
        PlatformError::new(PlatformErrorKind::InitializationFailed, format!("writing keymap to memfd failed: {e}"))
    })?;

    #[expect(clippy::cast_possible_truncation, reason = "keymap size won't exceed u32")]
    let size = keymap_bytes.len() as u32;
    vk.keymap(wl_keyboard::KeymapFormat::XkbV1.into(), file.as_fd(), size);
    debug!(size, "uploaded XKB keymap to virtual keyboard");

    let lookup = KeymapLookup::from_string_for_layout(&keymap_string, seat_group).map_err(|e| {
        PlatformError::new(PlatformErrorKind::InitializationFailed, format!("failed to build keymap lookup: {e}"))
    })?;
    info!(entries = lookup.len(), "virtual keyboard keymap ready");

    // Build an xkb::State from the same keymap so we can track modifier
    // state locally and send `modifiers` events to the compositor.
    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    let keymap =
        xkb::Keymap::new_from_string(&context, keymap_string, xkb::KEYMAP_FORMAT_TEXT_V1, xkb::KEYMAP_COMPILE_NO_FLAGS)
            .ok_or_else(|| {
                PlatformError::new(PlatformErrorKind::InitializationFailed, "failed to parse keymap for xkb state")
            })?;
    let xkb_state = xkb::State::new(&keymap);

    Ok((lookup, xkb_state))
}

/// Read an XKB keymap string from a file descriptor.
fn read_keymap_from_fd(fd: std::os::fd::OwnedFd, size: u32) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};

    let size = size as usize;
    if size == 0 {
        return None;
    }

    let mut file = std::fs::File::from(fd);
    file.seek(SeekFrom::Start(0)).ok()?;
    let mut buf = vec![0u8; size];
    file.read_exact(&mut buf).ok()?;

    // Strip trailing null byte(s) if present.
    let text = if buf.last() == Some(&0) { &buf[..buf.len() - 1] } else { &buf[..] };
    let keymap_str = std::str::from_utf8(text).ok()?;
    Some(keymap_str.to_string())
}

/// Send a complete key combination via the virtual keyboard protocol.
///
/// Presses modifiers → presses key → releases key → releases modifiers.
/// Sends `modifiers` events after each state change so the compositor knows
/// which modifiers are active.
fn send_virtual_key_combo(state: &mut VirtualInputState, combo: &KeyCombination) -> Result<(), KeyboardError> {
    if state.virtual_keyboard.is_none() {
        return Err(KeyboardError::NotReady);
    }
    let evdev_code = combo.evdev_keycode();
    let mod_keys = combo.modifier_keycodes();
    let time = timestamp_ms(state.epoch);

    // Press modifiers.
    for &m in &mod_keys {
        state.virtual_keyboard.as_ref().unwrap().key(time, m, 1);
        update_xkb_and_send_mods(state, m, KeyState::Press);
    }

    // Press key.
    state.virtual_keyboard.as_ref().unwrap().key(time, evdev_code, 1);
    update_xkb_and_send_mods(state, evdev_code, KeyState::Press);

    // Release key.
    state.virtual_keyboard.as_ref().unwrap().key(time, evdev_code, 0);
    update_xkb_and_send_mods(state, evdev_code, KeyState::Release);

    // Release modifiers (reverse order).
    for &m in mod_keys.iter().rev() {
        state.virtual_keyboard.as_ref().unwrap().key(time, m, 0);
        update_xkb_and_send_mods(state, m, KeyState::Release);
    }

    state.conn.flush().map_err(|e| {
        KeyboardError::Platform(PlatformError::new(
            PlatformErrorKind::OperationFailed,
            format!("Wayland flush failed: {e}"),
        ))
    })?;

    Ok(())
}

/// XKB keycode offset: XKB keycodes = evdev keycode + 8.
const XKB_EVDEV_OFFSET: u32 = 8;

/// Update the local XKB state for a key event and send the resulting modifier
/// state to the compositor via `vk.modifiers()`.
///
/// After calling `vk.key()`, this MUST be called so the compositor learns
/// which modifiers are depressed/latched/locked.
fn send_modifier_state(
    state: &mut VirtualInputState,
    evdev_code: u32,
    key_state: KeyState,
) -> Result<(), KeyboardError> {
    update_xkb_and_send_mods(state, evdev_code, key_state);
    state.conn.flush().map_err(|e| {
        KeyboardError::Platform(PlatformError::new(
            PlatformErrorKind::OperationFailed,
            format!("Wayland flush failed: {e}"),
        ))
    })
}

/// Update the local XKB state for the given key and emit `vk.modifiers()`.
///
/// Does nothing if no XKB state or virtual keyboard is available.
fn update_xkb_and_send_mods(state: &mut VirtualInputState, evdev_code: u32, key_state: KeyState) {
    let Some(ref mut xkb_state) = state.xkb_state else {
        return;
    };
    let direction = match key_state {
        KeyState::Press => xkb::KeyDirection::Down,
        KeyState::Release => xkb::KeyDirection::Up,
    };
    xkb_state.0.update_key((evdev_code + XKB_EVDEV_OFFSET).into(), direction);

    let depressed = xkb_state.0.serialize_mods(xkb::STATE_MODS_DEPRESSED);
    let latched = xkb_state.0.serialize_mods(xkb::STATE_MODS_LATCHED);
    let locked = xkb_state.0.serialize_mods(xkb::STATE_MODS_LOCKED);
    let group = xkb_state.0.serialize_layout(xkb::STATE_LAYOUT_EFFECTIVE);

    if let Some(ref vk) = state.virtual_keyboard {
        vk.modifiers(depressed, latched, locked, group);
    }
}

/// Returns a monotonic timestamp in milliseconds.
#[expect(clippy::cast_possible_truncation, reason = "timestamp won't overflow u32 in practice")]
fn timestamp_ms(epoch: Instant) -> u32 {
    epoch.elapsed().as_millis() as u32
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
