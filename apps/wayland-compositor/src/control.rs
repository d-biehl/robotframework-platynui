//! Test-Control IPC — Unix socket with JSON protocol for CI integration.
//!
//! Provides a control socket that test harnesses can connect to in order to
//! query compositor state, inject input, take screenshots, and control timing.
//!
//! # Protocol
//!
//! Newline-delimited JSON over a Unix stream socket. Each message is a single
//! JSON object terminated by `\n`. The compositor responds with a JSON object
//! also terminated by `\n`.
//!
//! ## Commands
//!
//! - `{"command": "status"}` → compositor status (version, uptime, backend, windows, outputs)
//! - `{"command": "list_windows"}` → list all mapped and minimized windows with state info
//! - `{"command": "get_window", "id": <n>}` → get details of a specific window by index
//! - `{"command": "get_window", "app_id": "..."}` → get window by `app_id` (exact match)
//! - `{"command": "get_window", "title": "..."}` → get window by title (case-insensitive substring)
//! - `{"command": "close_window", "id"|"app_id"|"title": ...}` → send close to a window
//! - `{"command": "focus_window", "id"|"app_id"|"title": ...}` → focus a window
//! - `{"command": "screenshot"}` → capture the current frame (base64 PNG)
//! - `{"command": "get_pointer_position"}` → current pointer coordinates (`x`, `y`)
//! - `{"command": "key_event", "key": <evdev_code>, "state": "press"|"release"}` → inject a keyboard event
//! - `{"command": "pointer_move_to", "x": <f64>, "y": <f64>}` → move pointer to absolute position
//! - `{"command": "pointer_button", "button": <evdev_code>, "state": "press"|"release"}` → inject pointer button
//! - `{"command": "pointer_scroll", "dx": <f64>, "dy": <f64>}` → inject scroll event
//! - `{"command": "get_keymap"}` → current XKB keymap string
//! - `{"command": "ping"}` → alias for `status`
//! - `{"command": "shutdown"}` → request compositor shutdown

use std::io::{Read, Write};
use std::os::fd::AsFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use smithay::backend::input::{AxisSource, ButtonState, KeyState};
use smithay::desktop::Window;
use smithay::input::keyboard::{FilterResult, xkb};
use smithay::input::pointer::{AxisFrame, MotionEvent};
use smithay::utils::{Physical, Point, SERIAL_COUNTER, Size};

use crate::handlers::foreign_toplevel;
use crate::input;
use crate::state::State;

// ---------------------------------------------------------------------------
// Protocol types
// ---------------------------------------------------------------------------

/// Incoming IPC request (deserialized from JSON).
#[derive(Deserialize)]
struct Request {
    command: Option<String>,
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    app_id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    /// Evdev keycode for `key_event`.
    #[serde(default)]
    key: Option<u32>,
    /// `"press"` or `"release"` for `key_event` / `pointer_button`.
    #[serde(default)]
    state: Option<String>,
    /// X coordinate for `pointer_move_to`.
    #[serde(default)]
    x: Option<f64>,
    /// Y coordinate for `pointer_move_to`.
    #[serde(default)]
    y: Option<f64>,
    /// Evdev button code for `pointer_button`.
    #[serde(default)]
    button: Option<u32>,
    /// Horizontal scroll delta for `pointer_scroll`.
    #[serde(default)]
    dx: Option<f64>,
    /// Vertical scroll delta for `pointer_scroll`.
    #[serde(default)]
    dy: Option<f64>,
}

/// Window information returned in IPC responses.
#[derive(Serialize)]
struct WindowInfo {
    id: usize,
    title: String,
    app_id: String,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    focused: bool,
    maximized: bool,
    fullscreen: bool,
}

/// Minimized window information.
#[derive(Serialize)]
struct MinimizedWindowInfo {
    id: String,
    title: String,
    app_id: String,
    x: i32,
    y: i32,
}

/// Output information.
#[derive(Serialize)]
struct OutputInfo {
    index: usize,
    name: String,
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    scale: f64,
}

/// A connected control client with a per-connection read buffer.
///
/// Registered as a non-blocking calloop event source so the compositor
/// event loop is never blocked waiting for client data.
struct ControlClient {
    stream: UnixStream,
    buf: Vec<u8>,
}

impl AsFd for ControlClient {
    fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> {
        self.stream.as_fd()
    }
}

/// Path to the control socket, derived from `$XDG_RUNTIME_DIR` and socket name.
pub fn control_socket_path(socket_name: &str) -> PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(runtime_dir).join(format!("{socket_name}.control"))
}

/// Set up the control socket as a calloop event source.
///
/// Creates a Unix listener at the control socket path and registers it with
/// the event loop to accept connections and process commands.
pub fn setup_control_socket(
    loop_handle: &calloop::LoopHandle<'static, State>,
    socket_name: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = control_socket_path(socket_name);

    if path.exists() {
        std::fs::remove_file(&path)?;
    }

    let listener = UnixListener::bind(&path)?;
    listener.set_nonblocking(true)?;

    tracing::info!(path = %path.display(), "control socket listening");

    loop_handle.insert_source(
        calloop::generic::Generic::new(listener, calloop::Interest::READ, calloop::Mode::Level),
        |_, listener, state| {
            // Accept all pending connections.
            loop {
                match listener.accept() {
                    Ok((stream, _addr)) => {
                        if let Err(err) = register_control_client(stream, state) {
                            tracing::warn!(%err, "failed to register control client");
                        }
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(err) => {
                        tracing::warn!(%err, "error accepting control connection");
                        break;
                    }
                }
            }
            Ok(calloop::PostAction::Continue)
        },
    )?;

    Ok(path)
}

/// Register an accepted client stream as a non-blocking calloop event source.
///
/// Each client connection gets its own event source so the compositor event
/// loop is never blocked waiting for client data. The source is automatically
/// removed when the client disconnects or a fatal I/O error occurs.
fn register_control_client(stream: UnixStream, state: &mut State) -> std::io::Result<()> {
    stream.set_nonblocking(true)?;
    let client = ControlClient { stream, buf: Vec::with_capacity(1024) };
    let source = calloop::generic::Generic::new(client, calloop::Interest::READ, calloop::Mode::Level);
    state
        .loop_handle
        .insert_source(source, |_, client, state| {
            // SAFETY: We only read from the stream and modify the buffer;
            // the file descriptor (calloop event source) is never replaced.
            #[allow(unsafe_code)]
            let client = unsafe { client.get_mut() };
            Ok(handle_client_data(client, state))
        })
        .map_err(std::io::Error::other)?;
    Ok(())
}

/// Handle readable data from a control client.
///
/// Reads available bytes into the client's buffer, processes complete
/// newline-terminated JSON lines, and sends responses. Returns
/// [`calloop::PostAction::Remove`] on EOF or fatal I/O errors to deregister
/// the source.
fn handle_client_data(client: &mut ControlClient, state: &mut State) -> calloop::PostAction {
    // Read all available data.
    let mut tmp = [0u8; 4096];
    loop {
        match client.stream.read(&mut tmp) {
            Ok(0) => return calloop::PostAction::Remove, // EOF
            Ok(n) => client.buf.extend_from_slice(&tmp[..n]),
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(err) => {
                tracing::debug!(%err, "control client read error");
                return calloop::PostAction::Remove;
            }
        }
    }

    // Process complete newline-terminated lines.
    while let Some(pos) = client.buf.iter().position(|&b| b == b'\n') {
        let line_bytes: Vec<u8> = client.buf.drain(..=pos).collect();
        let line = String::from_utf8_lossy(&line_bytes);
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let response = process_command(line, state);
        if let Some(response) = response
            && let Err(err) = writeln!(client.stream, "{response}")
        {
            tracing::debug!(%err, "control client write error");
            return calloop::PostAction::Remove;
        }
    }

    calloop::PostAction::Continue
}

/// Process a single JSON command and return a JSON response string.
///
/// Returns `None` for fire-and-forget input injection commands that need
/// no client acknowledgement (key, pointer, scroll events).
fn process_command(input: &str, state: &mut State) -> Option<String> {
    let request: Request = match serde_json::from_str(input.trim()) {
        Ok(req) => req,
        Err(_) => {
            return Some(serde_json::json!({"status": "error", "message": "invalid JSON"}).to_string());
        }
    };

    let response = match request.command.as_deref() {
        Some("ping" | "status") => build_status_response(state),

        Some("shutdown") => {
            state.running = false;
            serde_json::json!({"status": "ok", "message": "shutting down"})
        }

        Some("list_windows") => {
            let windows = list_windows(state);
            let minimized = list_minimized_windows(state);
            serde_json::json!({"status": "ok", "windows": windows, "minimized": minimized})
        }

        Some("get_window") => {
            match resolve_window_selector(state, request.id, request.app_id.as_deref(), request.title.as_deref()) {
                Some(info) => serde_json::json!({"status": "ok", "window": info}),
                None => serde_json::json!({"status": "error", "message": "window not found"}),
            }
        }

        Some("close_window") => {
            match resolve_and_act_on_window(
                state,
                request.id,
                request.app_id.as_deref(),
                request.title.as_deref(),
                |state, idx| close_window(state, idx),
            ) {
                Some((t, a)) => {
                    serde_json::json!({"status": "ok", "message": "close sent", "title": t, "app_id": a})
                }
                None => serde_json::json!({"status": "error", "message": "window not found"}),
            }
        }

        Some("focus_window") => {
            match resolve_and_act_on_window(
                state,
                request.id,
                request.app_id.as_deref(),
                request.title.as_deref(),
                focus_window,
            ) {
                Some((t, a)) => {
                    serde_json::json!({"status": "ok", "message": "window focused", "title": t, "app_id": a})
                }
                None => serde_json::json!({"status": "error", "message": "window not found"}),
            }
        }

        Some("get_pointer_position") => {
            let loc = state.pointer_location;
            serde_json::json!({"status": "ok", "x": loc.x, "y": loc.y})
        }

        Some("key_event" | "pointer_move_to" | "pointer_button" | "pointer_scroll" | "get_keymap") => {
            return process_input_command(request.command.as_deref().unwrap_or_default(), &request, state);
        }

        Some("screenshot") => match take_screenshot(state) {
            Ok(base64_png) => {
                let combined = state.combined_output_geometry();
                let max_scale =
                    state.outputs.iter().map(|o| o.current_scale().fractional_scale()).fold(1.0_f64, f64::max);
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let phys_w = (f64::from(combined.size.w) * max_scale).ceil() as i32;
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let phys_h = (f64::from(combined.size.h) * max_scale).ceil() as i32;
                serde_json::json!({
                    "status": "ok",
                    "format": "png",
                    "width": phys_w,
                    "height": phys_h,
                    "scale": max_scale,
                    "data": base64_png,
                })
            }
            Err(err) => {
                serde_json::json!({"status": "error", "message": format!("screenshot failed: {err}")})
            }
        },

        Some(cmd) => serde_json::json!({"status": "error", "message": format!("unknown command: {cmd}")}),

        None => serde_json::json!({"status": "error", "message": "missing or invalid command field"}),
    };

    Some(response.to_string())
}

// ---------------------------------------------------------------------------
//  Input command dispatch
// ---------------------------------------------------------------------------

/// Process input-related commands (key, pointer, scroll, keymap).
///
/// Returns `None` for fire-and-forget injection commands (no response sent
/// to the client) and `Some(response)` for query commands (`get_keymap`).
fn process_input_command(command: &str, request: &Request, state: &mut State) -> Option<String> {
    match command {
        "key_event" => {
            let Some(key) = request.key else {
                return Some(serde_json::json!({"status": "error", "message": "missing 'key' field"}).to_string());
            };
            let pressed = match request.state.as_deref() {
                Some("press") => true,
                Some("release") => false,
                _ => {
                    return Some(
                        serde_json::json!({"status": "error", "message": "'state' must be 'press' or 'release'"})
                            .to_string(),
                    );
                }
            };
            inject_key_event(state, key, pressed);
            None
        }

        "pointer_move_to" => {
            let Some(x) = request.x else {
                return Some(serde_json::json!({"status": "error", "message": "missing 'x' field"}).to_string());
            };
            let Some(y) = request.y else {
                return Some(serde_json::json!({"status": "error", "message": "missing 'y' field"}).to_string());
            };
            inject_pointer_move(state, x, y);
            None
        }

        "pointer_button" => {
            let Some(button) = request.button else {
                return Some(serde_json::json!({"status": "error", "message": "missing 'button' field"}).to_string());
            };
            let pressed = match request.state.as_deref() {
                Some("press") => true,
                Some("release") => false,
                _ => {
                    return Some(
                        serde_json::json!({"status": "error", "message": "'state' must be 'press' or 'release'"})
                            .to_string(),
                    );
                }
            };
            inject_pointer_button(state, button, pressed);
            None
        }

        "pointer_scroll" => {
            let dx = request.dx.unwrap_or(0.0);
            let dy = request.dy.unwrap_or(0.0);
            inject_pointer_scroll(state, dx, dy);
            None
        }

        "get_keymap" => match get_keymap_string(state) {
            Some(keymap) => Some(serde_json::json!({"status": "ok", "keymap": keymap}).to_string()),
            None => Some(serde_json::json!({"status": "error", "message": "keymap not available"}).to_string()),
        },

        _ => Some(
            serde_json::json!({"status": "error", "message": format!("unknown input command: {command}")}).to_string(),
        ),
    }
}

// ---------------------------------------------------------------------------
//  Input injection helpers
// ---------------------------------------------------------------------------

/// Get the current compositor time in milliseconds (for smithay event timestamps).
#[allow(clippy::cast_possible_truncation)]
fn current_time_msec(state: &State) -> u32 {
    state.start_time.elapsed().as_millis() as u32
}

/// Inject a keyboard key press or release via the smithay input stack.
fn inject_key_event(state: &mut State, evdev_key: u32, pressed: bool) {
    let key_state = if pressed { KeyState::Pressed } else { KeyState::Released };
    let serial = SERIAL_COUNTER.next_serial();
    let time = current_time_msec(state);
    // Evdev scancodes → XKB keycodes are offset by +8.
    let keycode = smithay::input::keyboard::Keycode::new(evdev_key + 8);
    let keyboard = state.keyboard();
    keyboard.input::<(), _>(state, keycode, key_state, serial, time, |_, _, _| FilterResult::Forward);
}

/// Inject an absolute pointer move via the smithay input stack.
fn inject_pointer_move(state: &mut State, x: f64, y: f64) {
    let serial = SERIAL_COUNTER.next_serial();
    state.pointer_location = Point::from((x, y));
    input::clamp_pointer_location(state);
    input::update_cursor_shape(state);

    let under = input::surface_under(state);
    let time = current_time_msec(state);
    let pointer = state.pointer();
    pointer.motion(state, under, &MotionEvent { location: state.pointer_location, serial, time });
    pointer.frame(state);
}

/// Inject a pointer button press or release via the smithay input stack.
fn inject_pointer_button(state: &mut State, button: u32, pressed: bool) {
    let button_state = if pressed { ButtonState::Pressed } else { ButtonState::Released };
    let time = current_time_msec(state);
    input::process_pointer_button(state, button, button_state, time);
}

/// Inject a scroll event via the smithay input stack.
#[allow(clippy::cast_possible_truncation)]
fn inject_pointer_scroll(state: &mut State, dx: f64, dy: f64) {
    let time = current_time_msec(state);
    let mut frame = AxisFrame::new(time).source(AxisSource::Finger);
    if dx != 0.0 {
        frame = frame.value(smithay::backend::input::Axis::Horizontal, dx);
    }
    if dy != 0.0 {
        frame = frame.value(smithay::backend::input::Axis::Vertical, dy);
    }
    let pointer = state.pointer();
    pointer.axis(state, frame);
    pointer.frame(state);
}

/// Get the compositor's active XKB keymap as a string.
fn get_keymap_string(state: &mut State) -> Option<String> {
    let keyboard = state.keyboard();
    keyboard.with_xkb_state(state, |xkb_context| {
        let xkb_state = xkb_context.xkb().lock().ok()?;
        // SAFETY: The keymap reference does not outlive this closure scope.
        #[allow(unsafe_code)]
        let keymap = unsafe { xkb_state.keymap() };
        Some(keymap.get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1))
    })
}

/// Build the JSON response for the `status` command.
fn build_status_response(state: &State) -> serde_json::Value {
    let outputs: Vec<OutputInfo> = state
        .outputs
        .iter()
        .enumerate()
        .map(|(i, o)| {
            let mode = o.current_mode().unwrap_or(smithay::output::Mode { size: (0, 0).into(), refresh: 0 });
            let loc = state.space.output_geometry(o).map(|g| g.loc).unwrap_or_default();
            OutputInfo {
                index: i,
                name: o.name(),
                width: mode.size.w,
                height: mode.size.h,
                x: loc.x,
                y: loc.y,
                scale: o.current_scale().fractional_scale(),
            }
        })
        .collect();

    serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "backend": state.backend_name,
        "uptime_secs": state.start_time.elapsed().as_secs(),
        "socket": state.socket_name,
        "xwayland": state.xwayland.is_some(),
        "windows": state.space.elements().count(),
        "minimized": state.minimized_windows.len(),
        "outputs": outputs,
    })
}

/// Resolve a window by `id` (index), `app_id` (exact match), or `title` (substring match).
///
/// If multiple selectors are present, `id` takes priority, followed by `app_id`,
/// then `title`. If `app_id` does not produce a match, falls through to `title`.
fn resolve_window_selector(
    state: &State,
    id: Option<u64>,
    app_id: Option<&str>,
    title: Option<&str>,
) -> Option<WindowInfo> {
    if let Some(id) = id {
        return get_window_info(state, id);
    }
    if let Some(app_id_query) = app_id {
        for (idx, window) in state.space.elements().enumerate() {
            if foreign_toplevel::window_app_id(window) == app_id_query {
                return Some(build_window_info(state, idx, window));
            }
        }
        // Fall through to title matching
    }
    if let Some(title_query) = title {
        let query_lower = title_query.to_lowercase();
        for (idx, window) in state.space.elements().enumerate() {
            if foreign_toplevel::window_title(window).to_lowercase().contains(&query_lower) {
                return Some(build_window_info(state, idx, window));
            }
        }
    }
    None
}

/// Resolve a window by selector and perform an action. Returns (title, `app_id`) on success.
fn resolve_and_act_on_window(
    state: &mut State,
    id: Option<u64>,
    app_id: Option<&str>,
    title: Option<&str>,
    action: impl FnOnce(&mut State, u64) -> bool,
) -> Option<(String, String)> {
    let resolved_idx = resolve_window_index(state, id, app_id, title)?;
    let window = state.space.elements().nth(resolved_idx)?;
    let t = foreign_toplevel::window_title(window);
    let a = foreign_toplevel::window_app_id(window);
    if action(state, resolved_idx as u64) { Some((t, a)) } else { None }
}

/// Resolve a window selector to an index.
///
/// Falls through from `app_id` to `title` if `app_id` does not match.
fn resolve_window_index(state: &State, id: Option<u64>, app_id: Option<&str>, title: Option<&str>) -> Option<usize> {
    if let Some(id) = id {
        return usize::try_from(id).ok();
    }
    if let Some(app_id_query) = app_id {
        for (idx, window) in state.space.elements().enumerate() {
            if foreign_toplevel::window_app_id(window) == app_id_query {
                return Some(idx);
            }
        }
        // Fall through to title matching
    }
    if let Some(title_query) = title {
        let query_lower = title_query.to_lowercase();
        for (idx, window) in state.space.elements().enumerate() {
            if foreign_toplevel::window_title(window).to_lowercase().contains(&query_lower) {
                return Some(idx);
            }
        }
    }
    None
}

/// Check if a window is maximized.
fn is_maximized(window: &Window) -> bool {
    window.toplevel().is_some_and(|t| {
        t.current_state()
            .states
            .contains(smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Maximized)
    })
}

/// Check if a window is fullscreen.
fn is_fullscreen(window: &Window) -> bool {
    window.toplevel().is_some_and(|t| {
        t.current_state()
            .states
            .contains(smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Fullscreen)
    })
}

/// Check if a window is the currently focused window.
fn is_focused(state: &State, window: &Window) -> bool {
    state.seat.get_keyboard().and_then(|kb| kb.current_focus()).is_some_and(|focus| {
        use smithay::wayland::seat::WaylandFocus;
        focus.wl_surface().zip(window.wl_surface()).is_some_and(|(a, b)| *a == *b)
    })
}

/// Format window info as a typed struct for serialization.
fn build_window_info(state: &State, idx: usize, window: &Window) -> WindowInfo {
    let loc = state.space.element_location(window).unwrap_or_default();
    let geo = window.geometry();
    WindowInfo {
        id: idx,
        title: foreign_toplevel::window_title(window),
        app_id: foreign_toplevel::window_app_id(window),
        x: loc.x,
        y: loc.y,
        width: geo.size.w,
        height: geo.size.h,
        focused: is_focused(state, window),
        maximized: is_maximized(window),
        fullscreen: is_fullscreen(window),
    }
}

/// List all mapped windows as typed structs.
fn list_windows(state: &State) -> Vec<WindowInfo> {
    state.space.elements().enumerate().map(|(idx, window)| build_window_info(state, idx, window)).collect()
}

/// List minimized windows as typed structs.
fn list_minimized_windows(state: &State) -> Vec<MinimizedWindowInfo> {
    state
        .minimized_windows
        .iter()
        .enumerate()
        .map(|(idx, (window, pos))| MinimizedWindowInfo {
            id: format!("minimized_{idx}"),
            title: foreign_toplevel::window_title(window),
            app_id: foreign_toplevel::window_app_id(window),
            x: pos.x,
            y: pos.y,
        })
        .collect()
}

/// Get info about a specific window by index.
fn get_window_info(state: &State, id: u64) -> Option<WindowInfo> {
    let id = usize::try_from(id).ok()?;
    let (idx, window) = state.space.elements().enumerate().nth(id)?;
    Some(build_window_info(state, idx, window))
}

/// Send close to a window by index.
fn close_window(state: &State, id: u64) -> bool {
    let Some(id) = usize::try_from(id).ok() else { return false };
    if let Some(toplevel) = state.space.elements().nth(id).and_then(Window::toplevel) {
        toplevel.send_close();
        return true;
    }
    false
}

/// Focus a window by index.
///
/// # Panics
///
/// Panics if the seat has no keyboard.
fn focus_window(state: &mut State, id: u64) -> bool {
    let Some(id) = usize::try_from(id).ok() else { return false };
    let window = state.space.elements().nth(id).cloned();
    if let Some(window) = window {
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        let keyboard = state.keyboard();
        keyboard.set_focus(state, Some(crate::focus::KeyboardFocusTarget::Window(window.clone())), serial);
        state.space.raise_element(&window, true);
        true
    } else {
        false
    }
}

// -- Screenshot implementation --

/// Capture the current compositor scene as a base64-encoded PNG.
///
/// Uses the [`GlowRenderer`](smithay::backend::renderer::glow::GlowRenderer)
/// stored in `State::screenshot_renderer`.  For winit and DRM backends this
/// is pre-initialized with a shared EGL context (same GL namespace as the
/// main renderer) so that client surface textures and egui titlebar textures
/// are accessible.  For headless (no main renderer) a standalone offscreen
/// renderer is lazily created from a DRI render node.
///
/// Renders all windows with their decorations into an offscreen GL texture,
/// reads back the pixels, and encodes them as a PNG.
///
/// For multi-output setups with different scales, the screenshot uses the
/// maximum scale across all outputs so that `HiDPI` content remains sharp.
fn take_screenshot(state: &mut crate::state::State) -> Result<String, String> {
    let combined_geo = state.combined_output_geometry();
    let width = combined_geo.size.w;
    let height = combined_geo.size.h;

    if width <= 0 || height <= 0 {
        return Err("invalid output size".to_string());
    }

    // Use the maximum output scale so HiDPI outputs look sharp in screenshots.
    let max_scale = state.outputs.iter().map(|o| o.current_scale().fractional_scale()).fold(1.0_f64, f64::max);

    // Lazily initialize the screenshot renderer on first use (headless fallback).
    // For winit/DRM backends this is already pre-initialized with a shared
    // EGL context; the fallback creates a standalone offscreen renderer.
    if state.screenshot_renderer.is_none() {
        state.screenshot_renderer = Some(
            crate::backend::create_offscreen_glow_renderer()
                .map_err(|e| format!("failed to create screenshot renderer: {e}"))?,
        );
    }

    // Temporarily take the renderer to avoid borrow conflicts with `state`
    // (collect_render_elements needs `&mut renderer` and `&mut state`).
    let mut renderer = state.screenshot_renderer.take().expect("screenshot renderer was just initialized above");

    // Scale logical dimensions to physical pixels.
    #[allow(clippy::cast_possible_truncation)]
    let phys_w = (f64::from(width) * max_scale).ceil() as i32;
    #[allow(clippy::cast_possible_truncation)]
    let phys_h = (f64::from(height) * max_scale).ceil() as i32;
    let size: Size<i32, Physical> = (phys_w, phys_h).into();
    let output = state.output.clone();

    let result =
        crate::render::render_to_pixels(&mut renderer, state, &output, size, max_scale, true).and_then(|pixel_data| {
            // Abgr8888 in DRM fourcc = GL's RGBA byte order → already R, G, B, A in memory.
            let w = u32::try_from(phys_w).map_err(|e| format!("width: {e}"))?;
            let h = u32::try_from(phys_h).map_err(|e| format!("height: {e}"))?;

            // Encode as PNG and base64 for JSON transport
            let png_data = encode_png(&pixel_data, w, h).map_err(|e| format!("PNG encode: {e}"))?;
            Ok(base64_encode(&png_data))
        });

    // Put the renderer back for reuse
    state.screenshot_renderer = Some(renderer);

    result
}

/// Encode RGBA pixel data as a PNG using the `png` crate.
fn encode_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, png::EncodingError> {
    let mut data = Vec::new();
    let mut encoder = png::Encoder::new(&mut data, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgba)?;
    drop(writer);
    Ok(data)
}

/// Encode bytes as base64 (RFC 4648, no line breaks).
///
/// Self-contained implementation to avoid adding an external base64 crate.
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut result = String::with_capacity(data.len().div_ceil(3) * 4);

    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };

        let triple = u32::from(b0) << 16 | u32::from(b1) << 8 | u32::from(b2);

        result.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        result.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            result.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(ALPHABET[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_encode_empty() {
        assert_eq!(base64_encode(b""), "");
    }

    #[test]
    fn base64_encode_hello() {
        assert_eq!(base64_encode(b"Hello"), "SGVsbG8=");
    }

    #[test]
    fn base64_encode_roundtrip() {
        let data = b"PlatynUI Wayland Compositor";
        let encoded = base64_encode(data);
        assert_eq!(encoded, "UGxhdHluVUkgV2F5bGFuZCBDb21wb3NpdG9y");
    }

    #[test]
    fn base64_encode_binary() {
        let data = [0u8, 1, 2, 3, 255, 254, 253];
        let encoded = base64_encode(&data);
        assert!(!encoded.is_empty());
        // Verify padding
        assert!(encoded.len().is_multiple_of(4));
    }
}
