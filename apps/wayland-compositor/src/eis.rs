//! EIS (Emulated Input Server) — accepts libei clients for input injection.
//!
//! Listens on a Unix socket (`$XDG_RUNTIME_DIR/eis-platynui`) and uses the
//! `reis` crate's calloop integration to handle connections + input events.
//! Single-client for now; a second connection replaces the first.

use std::io;
use std::os::unix::io::AsFd;
use std::path::PathBuf;

use calloop::LoopHandle;
use reis::calloop::{EisListenerSource, EisRequestSource, EisRequestSourceEvent};
use reis::eis;
use reis::enumflags2::BitFlags;
use reis::request::{Connection, Device, EisRequest};
use smithay::backend::input::{ButtonState, KeyState};
use smithay::input::keyboard::{FilterResult, xkb};
use smithay::input::pointer::{AxisFrame, MotionEvent};
use smithay::utils::{Point, SERIAL_COUNTER};

use crate::input;
use crate::state::State;

/// Bind the EIS listener and register it with the calloop event loop.
///
/// Returns the socket path on success (for setting `LIBEI_SOCKET`).
pub fn setup_eis_server(handle: &LoopHandle<'static, State>) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let socket_name = "eis-platynui";
    let path = eis_socket_path(socket_name);

    // Remove stale socket if it exists.
    if path.exists() {
        std::fs::remove_file(&path)?;
    }

    let listener = eis::Listener::bind(&path)?;
    tracing::info!(path = %path.display(), "EIS server listening");

    let listener_source = EisListenerSource::new(listener);
    handle.insert_source(listener_source, |context, (), state| handle_new_eis_connection(context, state))?;

    Ok(path)
}

/// Compute the EIS socket path in `$XDG_RUNTIME_DIR`.
fn eis_socket_path(name: &str) -> PathBuf {
    let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR").map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
    runtime_dir.join(name)
}

// ---------------------------------------------------------------------------
// Connection lifecycle
// ---------------------------------------------------------------------------

/// Accept a new EIS client connection.
fn handle_new_eis_connection(context: eis::Context, state: &mut State) -> io::Result<calloop::PostAction> {
    // Disconnect previous client if any.
    if let Some(token) = state.eis_client_token.take() {
        tracing::info!("replacing previous EIS client");
        if let Some(device) = state.eis_client_device.take() {
            device.remove();
        }
        state.loop_handle.remove(token);
    }

    let request_source = EisRequestSource::new(context, 1);
    let token = state.loop_handle.insert_source(request_source, handle_eis_event).map_err(io::Error::other)?;

    state.eis_client_token = Some(token);
    tracing::info!("new EIS client connected (handshake in progress)");
    Ok(calloop::PostAction::Continue)
}

/// Handle events from an EIS client's `EisRequestSource`.
#[allow(clippy::unnecessary_wraps)] // calloop EventSource requires io::Result<PostAction>
fn handle_eis_event(
    event: Result<EisRequestSourceEvent, reis::Error>,
    connection: &mut Connection,
    state: &mut State,
) -> io::Result<calloop::PostAction> {
    match event {
        Ok(EisRequestSourceEvent::Connected) => {
            handle_eis_connected(connection, state);
            Ok(calloop::PostAction::Continue)
        }
        Ok(EisRequestSourceEvent::Request(request)) => {
            handle_eis_request(&request, connection, state);
            Ok(calloop::PostAction::Continue)
        }
        Err(err) => {
            tracing::warn!(%err, "EIS client error, disconnecting");
            cleanup_eis_client(state);
            Ok(calloop::PostAction::Remove)
        }
    }
}

/// Handshake completed — advertise a seat with all capabilities.
fn handle_eis_connected(connection: &Connection, state: &mut State) {
    let client_name = connection.name().unwrap_or("<unknown>");
    let context_type = connection.context_type();
    tracing::info!(name = client_name, ?context_type, "EIS client handshake complete");

    // Only sender context is valid — clients that send emulated input.
    if context_type != eis::handshake::ContextType::Sender {
        tracing::warn!(?context_type, "EIS client has wrong context type, expected Sender");
        connection
            .disconnected(eis::connection::DisconnectReason::Protocol, Some("only sender context type is supported"));
        let _ = connection.flush();
        cleanup_eis_client(state);
        return;
    }

    // Advertise a seat with all capabilities. Device creation is deferred
    // to the Bind handler (the client picks which capabilities it wants).
    let all_caps = BitFlags::all();
    let _seat = connection.add_seat(Some("default"), all_caps);
    let _ = connection.flush();
    tracing::debug!("EIS seat advertised, waiting for client Bind");
}

// ---------------------------------------------------------------------------
// Request dispatch
// ---------------------------------------------------------------------------

/// Handle a high-level EIS request.
fn handle_eis_request(request: &EisRequest, connection: &Connection, state: &mut State) {
    match request {
        EisRequest::Disconnect => {
            tracing::info!("EIS client disconnected");
            cleanup_eis_client(state);
        }
        EisRequest::Bind(bind) => {
            handle_eis_bind(bind, connection, state);
        }
        EisRequest::PointerMotion(m) => handle_eis_pointer_motion(m, state),
        EisRequest::PointerMotionAbsolute(m) => handle_eis_pointer_motion_absolute(m, state),
        EisRequest::Button(b) => handle_eis_button(b, state),
        EisRequest::ScrollDelta(s) => handle_eis_scroll_delta(s, state),
        EisRequest::ScrollDiscrete(s) => handle_eis_scroll_discrete(s, state),
        EisRequest::ScrollStop(s) => handle_eis_scroll_stop(s, state),
        EisRequest::KeyboardKey(k) => handle_eis_keyboard_key(k, state),
        EisRequest::TouchDown(t) => handle_eis_touch_down(t),
        EisRequest::TouchMotion(t) => handle_eis_touch_motion(t),
        EisRequest::TouchUp(t) => handle_eis_touch_up(t),
        EisRequest::DeviceStartEmulating(_)
        | EisRequest::DeviceStopEmulating(_)
        | EisRequest::Frame(_)
        | EisRequest::ScrollCancel(_)
        | EisRequest::TouchCancel(_) => {}
    }
    let _ = connection.flush();
}

// ---------------------------------------------------------------------------
// Bind — client wants capabilities, create/update device
// ---------------------------------------------------------------------------

fn handle_eis_bind(bind: &reis::request::Bind, connection: &Connection, state: &mut State) {
    let caps = bind.capabilities;
    tracing::info!(?caps, "EIS client binding capabilities");

    // Remove previous device if any (client re-binding).
    if let Some(old_device) = state.eis_client_device.take() {
        old_device.remove();
    }

    // Capture shared borrows for the `before_done` closure (runs synchronously).
    let outputs = &state.outputs;
    let space = &state.space;

    let device = bind.seat.add_device(Some("virtual-input"), eis::device::DeviceType::Virtual, caps, |device| {
        // Before done: send regions for absolute pointer.
        if device.has_capability(reis::request::DeviceCapability::PointerAbsolute) {
            send_regions(device, outputs, space);
        }
        send_keymap_if_available(device);
    });

    device.resumed();
    let _ = connection.flush();
    tracing::info!("EIS device created and resumed");

    state.eis_client_device = Some(device);
}

/// Send output regions so that absolute pointer coordinates are meaningful.
fn send_regions(
    device: &Device,
    outputs: &[smithay::output::Output],
    space: &smithay::desktop::Space<smithay::desktop::Window>,
) {
    let eis_device = device.device();
    for output in outputs {
        if let Some(geo) = space.output_geometry(output) {
            let scale = output.current_scale().fractional_scale();
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            eis_device.region(geo.loc.x as u32, geo.loc.y as u32, geo.size.w as u32, geo.size.h as u32, scale as f32);
            tracing::debug!(x = geo.loc.x, y = geo.loc.y, w = geo.size.w, h = geo.size.h, scale, "sent EIS region");
        }
    }
}

/// Try to send the XKB keymap via a tempfile fd.
///
/// Creates a default keymap matching the compositor's key handling. If the
/// keymap can't be created, the device proceeds without one.
fn send_keymap_if_available(device: &Device) {
    let Some(eis_keyboard) = device.interface::<eis::Keyboard>() else {
        return;
    };

    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    let Some(keymap) = xkb::Keymap::new_from_names(&context, "", "", "", "", None, xkb::KEYMAP_COMPILE_NO_FLAGS) else {
        tracing::warn!("failed to create XKB keymap for EIS");
        return;
    };

    let keymap_string = keymap.get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1);

    match tempfile::tempfile() {
        Ok(mut file) => {
            use std::io::Write;
            let keymap_bytes = keymap_string.as_bytes();
            if let Err(err) = file.write_all(keymap_bytes) {
                tracing::warn!(%err, "failed to write keymap to tempfile");
                return;
            }

            #[allow(clippy::cast_possible_truncation)]
            eis_keyboard.keymap(eis::keyboard::KeymapType::Xkb, keymap_bytes.len() as u32, file.as_fd());
            tracing::debug!(size = keymap_bytes.len(), "sent EIS keymap");
        }
        Err(err) => {
            tracing::warn!(%err, "failed to create tempfile for keymap");
        }
    }
}

// ---------------------------------------------------------------------------
// Input event handlers
// ---------------------------------------------------------------------------

fn handle_eis_pointer_motion(motion: &reis::request::PointerMotion, state: &mut State) {
    let serial = SERIAL_COUNTER.next_serial();
    state.pointer_location += (f64::from(motion.dx), f64::from(motion.dy)).into();
    input::clamp_pointer_location(state);
    input::update_cursor_shape(state);

    let under = input::surface_under(state);
    let pointer = state.pointer();
    pointer.motion(
        state,
        under,
        &MotionEvent { location: state.pointer_location, serial, time: eis_time_to_msec(motion.time) },
    );
    pointer.frame(state);
}

fn handle_eis_pointer_motion_absolute(motion: &reis::request::PointerMotionAbsolute, state: &mut State) {
    let serial = SERIAL_COUNTER.next_serial();
    state.pointer_location = Point::from((f64::from(motion.dx_absolute), f64::from(motion.dy_absolute)));
    input::clamp_pointer_location(state);
    input::update_cursor_shape(state);

    let under = input::surface_under(state);
    let pointer = state.pointer();
    pointer.motion(
        state,
        under,
        &MotionEvent { location: state.pointer_location, serial, time: eis_time_to_msec(motion.time) },
    );
    pointer.frame(state);
}

fn handle_eis_button(button: &reis::request::Button, state: &mut State) {
    let button_state =
        if button.state == eis::button::ButtonState::Press { ButtonState::Pressed } else { ButtonState::Released };
    tracing::trace!(button = button.button, ?button_state, "EIS button");
    input::process_pointer_button(state, button.button, button_state, eis_time_to_msec(button.time));
}

fn handle_eis_scroll_delta(scroll: &reis::request::ScrollDelta, state: &mut State) {
    let time = eis_time_to_msec(scroll.time);
    let mut frame = AxisFrame::new(time).source(smithay::backend::input::AxisSource::Finger);
    if scroll.dx != 0.0 {
        frame = frame.value(smithay::backend::input::Axis::Horizontal, f64::from(scroll.dx));
    }
    if scroll.dy != 0.0 {
        frame = frame.value(smithay::backend::input::Axis::Vertical, f64::from(scroll.dy));
    }
    let pointer = state.pointer();
    pointer.axis(state, frame);
    pointer.frame(state);
}

fn handle_eis_scroll_discrete(scroll: &reis::request::ScrollDiscrete, state: &mut State) {
    let time = eis_time_to_msec(scroll.time);
    let mut frame = AxisFrame::new(time).source(smithay::backend::input::AxisSource::Wheel);
    if scroll.discrete_dx != 0 {
        frame = frame.v120(smithay::backend::input::Axis::Horizontal, scroll.discrete_dx);
    }
    if scroll.discrete_dy != 0 {
        frame = frame.v120(smithay::backend::input::Axis::Vertical, scroll.discrete_dy);
    }
    let pointer = state.pointer();
    pointer.axis(state, frame);
    pointer.frame(state);
}

fn handle_eis_scroll_stop(scroll: &reis::request::ScrollStop, state: &mut State) {
    let time = eis_time_to_msec(scroll.time);
    let mut frame = AxisFrame::new(time);
    if scroll.x {
        frame = frame.stop(smithay::backend::input::Axis::Horizontal);
    }
    if scroll.y {
        frame = frame.stop(smithay::backend::input::Axis::Vertical);
    }
    let pointer = state.pointer();
    pointer.axis(state, frame);
    pointer.frame(state);
}

fn handle_eis_keyboard_key(key: &reis::request::KeyboardKey, state: &mut State) {
    let key_state = if key.state == eis::keyboard::KeyState::Press { KeyState::Pressed } else { KeyState::Released };
    tracing::trace!(key = key.key, ?key_state, "EIS keyboard key");

    let serial = SERIAL_COUNTER.next_serial();
    let keyboard = state.keyboard();
    let time = eis_time_to_msec(key.time);
    // EIS sends Linux evdev scancodes; XKB keycodes are offset by +8.
    let keycode = smithay::input::keyboard::Keycode::new(key.key + 8);
    keyboard.input::<(), _>(state, keycode, key_state, serial, time, |_, _, _| FilterResult::Forward);
}

fn handle_eis_touch_down(touch: &reis::request::TouchDown) {
    tracing::debug!(touch_id = touch.touch_id, x = touch.x, y = touch.y, "EIS touch down (stub)");
}

fn handle_eis_touch_motion(touch: &reis::request::TouchMotion) {
    tracing::debug!(touch_id = touch.touch_id, x = touch.x, y = touch.y, "EIS touch motion (stub)");
}

fn handle_eis_touch_up(touch: &reis::request::TouchUp) {
    tracing::debug!(touch_id = touch.touch_id, "EIS touch up (stub)");
}

// ---------------------------------------------------------------------------
// Cleanup
// ---------------------------------------------------------------------------

fn cleanup_eis_client(state: &mut State) {
    if let Some(device) = state.eis_client_device.take() {
        device.remove();
    }
    if let Some(token) = state.eis_client_token.take() {
        state.loop_handle.remove(token);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert EIS time (microseconds) to Wayland time (milliseconds).
#[allow(clippy::cast_possible_truncation)]
fn eis_time_to_msec(us: u64) -> u32 {
    (us / 1000) as u32
}
