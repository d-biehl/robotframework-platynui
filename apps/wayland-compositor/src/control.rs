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
//! - `{"command": "list_popups"}` → currently mapped `xdg_popups` with **global** rectangles
//!   (`popups: [{parent_window_id, pid, x, y, width, height}]`; empty array when none)
//! - `{"command": "get_modifiers"}` → current seat keyboard modifier state (`ctrl`, `alt`, `shift`, `logo`)
//! - `{"command": "get_window", "id": <n>}` → get details of a specific window by index
//! - `{"command": "get_window", "app_id": "..."}` → get window by `app_id` (exact match)
//! - `{"command": "get_window", "title": "..."}` → get window by title (case-insensitive substring)
//! - `{"command": "close_window", "id"|"app_id"|"title": ...}` → send close to a window
//! - `{"command": "focus_window", "id"|"app_id"|"title"|"window_id": ...}` → activate a window (a minimized one
//!   addressed by `window_id` comes back first, keeping its maximized state)
//! - `{"command": "screenshot"}` → capture the current frame (base64 PNG)
//! - `{"command": "get_pointer_position"}` → current pointer coordinates (`x`, `y`)
//! - `{"command": "window_at_point", "x": <f64>, "y": <f64>}` → frontmost window at the point that the
//!   asking process does not own (or null)
//! - `{"command": "key_event", "key": <evdev_code>, "state": "press"|"release"}` → inject a keyboard event
//! - `{"command": "pointer_move_to", "x": <f64>, "y": <f64>}` → move pointer to absolute position
//! - `{"command": "pointer_button", "button": <evdev_code>, "state": "press"|"release"}` → inject pointer button
//! - `{"command": "pointer_scroll", "dx": <f64>, "dy": <f64>}` → inject scroll event
//! - `{"command": "get_keymap"}` → current XKB keymap string
//! - `{"command": "move_window", ...}` → move a window to an absolute logical position
//! - `{"command": "resize_window", ...}` → resize a window to an absolute logical size
//! - `{"command": "show_highlight", "rects": [{"x": ..., "y": ..., "width": ..., "height": ...}], "duration_ms": <u64>}` → show compositor highlight frames
//! - `{"command": "clear_highlight"}` → clear compositor highlight frames
//! - `{"command": "ping"}` → alias for `status`
//! - `{"command": "shutdown"}` → request compositor shutdown

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::os::fd::AsFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use smithay::backend::input::{AxisSource, ButtonState, KeyState};
use smithay::desktop::space::SpaceElement;
use smithay::desktop::{PopupManager, Window};
use smithay::input::keyboard::{FilterResult, xkb};
use smithay::input::pointer::{AxisFrame, MotionEvent};
use smithay::reexports::wayland_server::Resource;
use smithay::utils::{Logical, Physical, Point, Rectangle, SERIAL_COUNTER, Size};
use smithay::wayland::compositor::{self, SurfaceAttributes};
use smithay::wayland::seat::WaylandFocus;
use smithay::wayland::shell::xdg::XdgPopupSurfaceData;

use crate::client::ClientState;
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
    window_id: Option<u64>,
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
    /// Width for `resize_window`.
    #[serde(default)]
    width: Option<f64>,
    /// Height for `resize_window`.
    #[serde(default)]
    height: Option<f64>,
    /// Rectangles for `show_highlight`.
    #[serde(default)]
    rects: Option<Vec<HighlightRectRequest>>,
    /// Optional auto-clear timeout for `show_highlight`.
    #[serde(default)]
    duration_ms: Option<u64>,
}

#[derive(Deserialize)]
struct HighlightRectRequest {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// Rectangle in the opaque region of a window surface.
#[derive(Serialize)]
struct OpaqueRegionRect {
    kind: &'static str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

/// Window information returned in IPC responses.
#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)] // wire format: independent window state flags
struct WindowInfo {
    id: usize,
    window_id: u64,
    title: String,
    app_id: String,
    pid: Option<u32>,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    content_x: i32,
    content_y: i32,
    content_width: i32,
    content_height: i32,
    /// Geometry offset within the surface buffer (CSD shadow inset).
    /// For CSD windows this is typically `(shadow_left, shadow_top)`,
    /// for SSD windows it is `(0, 0)`.
    geometry_x: i32,
    geometry_y: i32,
    focused: bool,
    /// Always `false` here: mapped windows are never minimized. Present so both window
    /// shapes carry the same state fields.
    minimized: bool,
    maximized: bool,
    fullscreen: bool,
    /// Decoration mode: `"csd"` (client-side) or `"ssd"` (server-side).
    decoration_mode: &'static str,
    opaque_region: Option<Vec<OpaqueRegionRect>>,
}

/// Popup information returned by `list_popups`.
///
/// `x`/`y` are **global** logical coordinates of the popup's visible geometry
/// (its xdg geometry rect placed on screen). Popups draw no server-side
/// decorations and their geometry excludes any client shadow, so no offset
/// applies. Popups have no title or id of their own; consumers correlate via
/// the parent toplevel's `parent_window_id`/`pid` (and the rect itself).
#[derive(Serialize)]
struct PopupInfo {
    parent_window_id: u64,
    pid: Option<u32>,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

/// Minimized window information.
///
/// `x`/`y` are the position the window returns to. The content size and the
/// maximized flag let clients match a minimized window against its accessibility
/// node and tell whether it will come back maximized.
#[derive(Serialize)]
struct MinimizedWindowInfo {
    id: String,
    window_id: u64,
    title: String,
    app_id: String,
    pid: Option<u32>,
    x: i32,
    y: i32,
    content_width: i32,
    content_height: i32,
    /// Always `true`: the window is in the compositor's minimized list.
    minimized: bool,
    maximized: bool,
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
    /// The process that opened this connection, as established when it was
    /// accepted. `None` means it could not be established. This is who "the
    /// caller" is for commands that answer relative to the asking process.
    caller: Option<u32>,
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

    // Both outcomes log at `debug`: the runtime opens one connection per command
    // (`crates/platform-linux-wayland/src/control_ipc.rs`), so a warning here
    // would flood the log that the per-client warning already makes readable.
    let caller = match crate::client::peer_process(&stream) {
        Ok(pid) => {
            tracing::debug!(pid, "control connection peer identified");
            Some(pid)
        }
        Err(reason) => {
            tracing::debug!(%reason, "could not identify the control connection's peer");
            None
        }
    };

    let client = ControlClient { stream, buf: Vec::with_capacity(1024), caller };
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
    // Read all available data. EOF must not short-circuit: a client may write
    // a command and close immediately (fire-and-forget injection), so data and
    // EOF can arrive in the same wakeup — process buffered lines first.
    let mut eof = false;
    let mut tmp = [0u8; 4096];
    loop {
        match client.stream.read(&mut tmp) {
            Ok(0) => {
                eof = true;
                break;
            }
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
        let response = process_command(line, state, client.caller);
        if let Some(response) = response
            && let Err(err) = write_response(&mut client.stream, &response)
        {
            tracing::debug!(%err, "control client write error");
            return calloop::PostAction::Remove;
        }
    }

    if eof { calloop::PostAction::Remove } else { calloop::PostAction::Continue }
}

/// Write a newline-terminated response to a (non-blocking) client stream.
///
/// Large responses (a screenshot's base64 PNG can be several MiB) do not fit
/// the socket buffer in one write; a plain `writeln!` would truncate the
/// response at the first `WouldBlock`. Retry with a deadline instead, so slow
/// readers get the full response and only stalled clients are dropped.
fn write_response(stream: &mut UnixStream, response: &str) -> std::io::Result<()> {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut data = Vec::with_capacity(response.len() + 1);
    data.extend_from_slice(response.as_bytes());
    data.push(b'\n');
    let mut written = 0;
    while written < data.len() {
        match stream.write(&data[written..]) {
            Ok(n) => written += n,
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                if std::time::Instant::now() >= deadline {
                    return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "control client stalled"));
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

/// Process a single JSON command and return a JSON response string.
///
/// Returns `None` for fire-and-forget input injection commands that need
/// no client acknowledgement (key, pointer, scroll events).
///
/// `caller` is the process the connection belongs to (`None` when the compositor
/// could not establish it); commands whose answer depends on who asked — the
/// window at a point — take it from here rather than from the request.
#[allow(clippy::too_many_lines)]
fn process_command(input: &str, state: &mut State, caller: Option<u32>) -> Option<String> {
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

        Some("list_popups") => {
            serde_json::json!({"status": "ok", "popups": list_popups(state)})
        }

        Some("get_modifiers") => {
            let mods = state.keyboard().modifier_state();
            serde_json::json!({
                "status": "ok",
                "ctrl": mods.ctrl,
                "alt": mods.alt,
                "shift": mods.shift,
                "logo": mods.logo,
            })
        }

        Some("get_window") => {
            let mapped = resolve_window_selector(
                state,
                request.window_id,
                request.id,
                request.app_id.as_deref(),
                request.title.as_deref(),
            )
            .map(|info| serde_json::json!(info));
            // A minimized window is unmapped, so only the stable window_id can still
            // address it.
            let window = mapped.or_else(|| {
                request
                    .window_id
                    .and_then(|window_id| minimized_window_info(state, window_id))
                    .map(|info| serde_json::json!(info))
            });
            match window {
                Some(window) => serde_json::json!({"status": "ok", "window": window}),
                None => serde_json::json!({"status": "error", "message": "window not found"}),
            }
        }

        Some("close_window") => {
            match resolve_and_act_on_window(
                state,
                request.window_id,
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
            match focus_window_by_selector(
                state,
                request.window_id,
                request.id,
                request.app_id.as_deref(),
                request.title.as_deref(),
            ) {
                Some((t, a)) => {
                    serde_json::json!({"status": "ok", "message": "window focused", "title": t, "app_id": a})
                }
                None => serde_json::json!({"status": "error", "message": "window not found"}),
            }
        }

        Some("minimize_window") => {
            match resolve_and_act_on_window(
                state,
                request.window_id,
                request.id,
                request.app_id.as_deref(),
                request.title.as_deref(),
                minimize_window,
            ) {
                Some((t, a)) => {
                    serde_json::json!({"status": "ok", "message": "window minimized", "title": t, "app_id": a})
                }
                None => serde_json::json!({"status": "error", "message": "window not found"}),
            }
        }

        Some("maximize_window") => {
            match resolve_and_act_on_window(
                state,
                request.window_id,
                request.id,
                request.app_id.as_deref(),
                request.title.as_deref(),
                maximize_window,
            ) {
                Some((t, a)) => {
                    serde_json::json!({"status": "ok", "message": "window maximized", "title": t, "app_id": a})
                }
                None => serde_json::json!({"status": "error", "message": "window not found"}),
            }
        }

        Some("restore_window") => {
            match restore_window_by_selector(
                state,
                request.window_id,
                request.id,
                request.app_id.as_deref(),
                request.title.as_deref(),
            ) {
                Some((t, a)) => {
                    serde_json::json!({"status": "ok", "message": "window restored", "title": t, "app_id": a})
                }
                None => serde_json::json!({"status": "error", "message": "window not found"}),
            }
        }

        Some("move_window") => {
            match move_window_by_selector(
                state,
                request.window_id,
                request.id,
                request.app_id.as_deref(),
                request.title.as_deref(),
                request.x,
                request.y,
            ) {
                Ok(Some((t, a))) => {
                    serde_json::json!({"status": "ok", "message": "window moved", "title": t, "app_id": a})
                }
                Ok(None) => serde_json::json!({"status": "error", "message": "window not found"}),
                Err(message) => {
                    serde_json::json!({"status": "error", "message": format!("invalid or missing '{message}' field")})
                }
            }
        }

        Some("resize_window") => {
            match resize_window_by_selector(
                state,
                request.window_id,
                request.id,
                request.app_id.as_deref(),
                request.title.as_deref(),
                request.width,
                request.height,
            ) {
                Ok(Some((t, a))) => {
                    serde_json::json!({"status": "ok", "message": "window resized", "title": t, "app_id": a})
                }
                Ok(None) => serde_json::json!({"status": "error", "message": "window not found"}),
                Err(message) => {
                    serde_json::json!({"status": "error", "message": format!("invalid or missing '{message}' field")})
                }
            }
        }

        Some("get_pointer_position") => {
            let loc = state.pointer_location;
            serde_json::json!({"status": "ok", "x": loc.x, "y": loc.y})
        }

        Some("window_at_point") => match (request.x, request.y) {
            (Some(x), Some(y)) => match window_at_point(state, (x, y), caller) {
                Some((idx, window)) => {
                    let info = build_window_info(state, idx, &window);
                    serde_json::json!({"status": "ok", "window": info})
                }
                None => serde_json::json!({"status": "ok", "window": serde_json::Value::Null}),
            },
            _ => serde_json::json!({"status": "error", "message": "window_at_point requires x and y"}),
        },

        Some("show_highlight") => match show_highlight(state, &request) {
            Ok(rect_count) => serde_json::json!({"status": "ok", "message": "highlight updated", "rects": rect_count}),
            Err(message) => serde_json::json!({"status": "error", "message": message}),
        },

        Some("clear_highlight") => {
            state.highlight_overlay.clear();
            serde_json::json!({"status": "ok", "message": "highlight cleared"})
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

fn show_highlight(state: &mut State, request: &Request) -> Result<usize, String> {
    let rects = if let Some(rects) = request.rects.as_ref() {
        rects
            .iter()
            .map(|rect| {
                crate::highlight::logical_rectangle(rect.x, rect.y, rect.width, rect.height)
                    .ok_or_else(|| "invalid highlight rectangle".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let (Some(x), Some(y), Some(width), Some(height)) = (request.x, request.y, request.width, request.height)
        else {
            return Err("missing highlight rects or x/y/width/height fields".into());
        };

        vec![
            crate::highlight::logical_rectangle(x, y, width, height)
                .ok_or_else(|| "invalid highlight rectangle".to_string())?,
        ]
    };

    let duration = request.duration_ms.map(Duration::from_millis);
    state.highlight_overlay.show(rects, duration);
    state.schedule_render();
    Ok(state.highlight_overlay.rects().len())
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
    // The control socket carries PlatynUI MOUSE-WHEEL deltas in v120 units (120 = one notch), in the
    // platform's "down/right is negative" convention (matching the `scroll_step` (0, -120) default and
    // the X11 wheel buttons). Wayland's axis convention is the opposite — positive is down/right — so
    // negate to translate before injecting.
    let (wl_horizontal, wl_vertical) = (-dx, -dy);
    // Emit a wheel-source axis event carrying both the continuous pixel value and the discrete v120
    // amount, exactly like real wheel input (see `input::handle_pointer_axis`). `AxisSource::Finger`
    // must NOT be used here: a finger/touchpad axis carries gesture-continuity semantics (it needs an
    // `axis_stop` when the gesture ends), which a wheel does not.
    let mut frame = AxisFrame::new(time).source(AxisSource::Wheel);
    if wl_horizontal != 0.0 {
        frame = frame
            .value(
                smithay::backend::input::Axis::Horizontal,
                wl_horizontal * input::SCROLL_PIXELS_PER_NOTCH / input::V120_UNITS_PER_NOTCH,
            )
            .v120(smithay::backend::input::Axis::Horizontal, wl_horizontal as i32);
    }
    if wl_vertical != 0.0 {
        frame = frame
            .value(
                smithay::backend::input::Axis::Vertical,
                wl_vertical * input::SCROLL_PIXELS_PER_NOTCH / input::V120_UNITS_PER_NOTCH,
            )
            .v120(smithay::backend::input::Axis::Vertical, wl_vertical as i32);
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
    window_id: Option<u64>,
    id: Option<u64>,
    app_id: Option<&str>,
    title: Option<&str>,
) -> Option<WindowInfo> {
    if let Some(window_id) = window_id {
        return state
            .space
            .elements()
            .enumerate()
            .find(|(_, window)| window_stable_id(window) == window_id)
            .map(|(idx, window)| build_window_info(state, idx, window));
    }
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
    window_id: Option<u64>,
    id: Option<u64>,
    app_id: Option<&str>,
    title: Option<&str>,
    action: impl FnOnce(&mut State, u64) -> bool,
) -> Option<(String, String)> {
    let resolved_idx = resolve_window_index(state, window_id, id, app_id, title)?;
    let window = state.space.elements().nth(resolved_idx)?;
    let t = foreign_toplevel::window_title(window);
    let a = foreign_toplevel::window_app_id(window);
    if action(state, resolved_idx as u64) { Some((t, a)) } else { None }
}

fn restore_window_by_selector(
    state: &mut State,
    window_id: Option<u64>,
    id: Option<u64>,
    app_id: Option<&str>,
    title: Option<&str>,
) -> Option<(String, String)> {
    if let Some(window_id) = window_id {
        let mapped_window = { state.space.elements().find(|window| window_stable_id(window) == window_id).cloned() };
        if let Some(window) = mapped_window {
            let resolved_title = foreign_toplevel::window_title(&window);
            let resolved_app_id = foreign_toplevel::window_app_id(&window);
            crate::handlers::foreign_toplevel::restore_window(state, &window);
            return Some((resolved_title, resolved_app_id));
        }

        let minimized_window = {
            state
                .minimized_windows
                .iter()
                .find(|(window, _)| window_stable_id(window) == window_id)
                .map(|(window, _)| window.clone())
        };
        if let Some(window) = minimized_window {
            let resolved_title = foreign_toplevel::window_title(&window);
            let resolved_app_id = foreign_toplevel::window_app_id(&window);
            crate::handlers::foreign_toplevel::restore_window(state, &window);
            return Some((resolved_title, resolved_app_id));
        }
    }

    resolve_and_act_on_window(state, None, id, app_id, title, restore_window)
}

/// Activate a window: bring it back if minimized, then focus and raise it.
///
/// A window addressed by `window_id` is looked up among mapped and minimized
/// windows; the index, `app_id` and `title` selectors only reach mapped windows.
fn focus_window_by_selector(
    state: &mut State,
    window_id: Option<u64>,
    id: Option<u64>,
    app_id: Option<&str>,
    title: Option<&str>,
) -> Option<(String, String)> {
    if let Some(window_id) = window_id {
        let window = state
            .space
            .elements()
            .find(|window| window_stable_id(window) == window_id)
            .or_else(|| {
                state
                    .minimized_windows
                    .iter()
                    .map(|(window, _)| window)
                    .find(|window| window_stable_id(window) == window_id)
            })
            .cloned();
        if let Some(window) = window {
            let resolved_title = foreign_toplevel::window_title(&window);
            let resolved_app_id = foreign_toplevel::window_app_id(&window);
            crate::handlers::foreign_toplevel::activate_window(state, &window);
            return Some((resolved_title, resolved_app_id));
        }
    }

    resolve_and_act_on_window(state, None, id, app_id, title, focus_window)
}

fn move_window_by_selector(
    state: &mut State,
    window_id: Option<u64>,
    id: Option<u64>,
    app_id: Option<&str>,
    title: Option<&str>,
    x: Option<f64>,
    y: Option<f64>,
) -> Result<Option<(String, String)>, &'static str> {
    let x = parse_i32_coordinate(x, "x")?;
    let y = parse_i32_coordinate(y, "y")?;
    Ok(resolve_and_act_on_window(state, window_id, id, app_id, title, |state, idx| move_window(state, idx, x, y)))
}

fn resize_window_by_selector(
    state: &mut State,
    window_id: Option<u64>,
    id: Option<u64>,
    app_id: Option<&str>,
    title: Option<&str>,
    width: Option<f64>,
    height: Option<f64>,
) -> Result<Option<(String, String)>, &'static str> {
    let width = parse_i32_size(width, "width")?;
    let height = parse_i32_size(height, "height")?;
    Ok(resolve_and_act_on_window(state, window_id, id, app_id, title, |state, idx| {
        resize_window(state, idx, width, height)
    }))
}

/// Resolve a window selector to an index.
///
/// Falls through from `app_id` to `title` if `app_id` does not match.
fn resolve_window_index(
    state: &State,
    window_id: Option<u64>,
    id: Option<u64>,
    app_id: Option<&str>,
    title: Option<&str>,
) -> Option<usize> {
    if let Some(window_id) = window_id {
        for (idx, window) in state.space.elements().enumerate() {
            if window_stable_id(window) == window_id {
                return Some(idx);
            }
        }
    }
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
    state
        .seat
        .get_keyboard()
        .and_then(|kb| kb.current_focus())
        .is_some_and(|focus| focus.wl_surface().zip(window.wl_surface()).is_some_and(|(a, b)| *a == *b))
}

/// Format window info as a typed struct for serialization.
fn build_window_info(state: &State, idx: usize, window: &Window) -> WindowInfo {
    let content_bounds = window_content_bounds(state, window);
    let frame_bounds = window_frame_bounds(window, content_bounds);
    let geo = window.geometry();
    WindowInfo {
        id: idx,
        window_id: window_stable_id(window),
        title: foreign_toplevel::window_title(window),
        app_id: foreign_toplevel::window_app_id(window),
        pid: window_pid(window),
        x: frame_bounds.loc.x,
        y: frame_bounds.loc.y,
        width: frame_bounds.size.w,
        height: frame_bounds.size.h,
        content_x: content_bounds.loc.x,
        content_y: content_bounds.loc.y,
        content_width: content_bounds.size.w,
        content_height: content_bounds.size.h,
        geometry_x: geo.loc.x,
        geometry_y: geo.loc.y,
        focused: is_focused(state, window),
        minimized: false,
        maximized: is_maximized(window),
        fullscreen: is_fullscreen(window),
        decoration_mode: if crate::decorations::window_has_ssd(window) { "ssd" } else { "csd" },
        opaque_region: window_opaque_region(window),
    }
}

fn window_content_bounds(state: &State, window: &Window) -> Rectangle<i32, Logical> {
    let loc = state.space.element_location(window).unwrap_or_default();
    let geo = window.geometry();
    Rectangle::new(loc, geo.size)
}

fn window_frame_bounds(window: &Window, content_bounds: Rectangle<i32, Logical>) -> Rectangle<i32, Logical> {
    if crate::decorations::window_has_ssd(window) {
        let frame_loc = (
            content_bounds.loc.x - crate::decorations::RESIZE_BORDER,
            content_bounds.loc.y - crate::decorations::TITLEBAR_HEIGHT - crate::decorations::RESIZE_BORDER,
        )
            .into();
        let frame_size = (
            content_bounds.size.w + crate::decorations::RESIZE_BORDER * 2,
            content_bounds.size.h + crate::decorations::TITLEBAR_HEIGHT + crate::decorations::RESIZE_BORDER * 2,
        )
            .into();
        return Rectangle::new(frame_loc, frame_size);
    }

    let geo = window.geometry();
    Rectangle::new((content_bounds.loc.x + geo.loc.x, content_bounds.loc.y + geo.loc.y).into(), geo.size)
}

/// Extract the opaque region from a window's root surface, if set.
///
/// Coordinates are in surface-local space (relative to the buffer origin).
/// For a typical GTK4 CSD window with shadow inset (6, 8), the opaque
/// region will be `{6, 8, content_w, content_h}`.
fn window_opaque_region(window: &Window) -> Option<Vec<OpaqueRegionRect>> {
    let surface = window.wl_surface()?;
    compositor::with_states(&surface, |states| {
        states.cached_state.get::<SurfaceAttributes>().current().opaque_region.as_ref().map(|region| {
            region
                .rects
                .iter()
                .map(|(kind, rect)| OpaqueRegionRect {
                    kind: match kind {
                        compositor::RectangleKind::Add => "add",
                        compositor::RectangleKind::Subtract => "subtract",
                    },
                    x: rect.loc.x,
                    y: rect.loc.y,
                    width: rect.size.w,
                    height: rect.size.h,
                })
                .collect()
        })
    })
}

/// List all mapped windows as typed structs.
fn list_windows(state: &State) -> Vec<WindowInfo> {
    state.space.elements().enumerate().map(|(idx, window)| build_window_info(state, idx, window)).collect()
}

/// List the currently mapped `xdg_popups` of all toplevels with their global
/// rectangles.
///
/// The global position follows the same arithmetic smithay's window render
/// path uses: the root toplevel's `element_location` (its geometry origin on
/// screen) plus the popup's accumulated location relative to that geometry
/// ([`PopupManager::popups_for_surface`], which already sums nested cascade
/// offsets including the popup's own geometry offset). Within one popup chain
/// deeper (more recently opened) levels are yielded first. Layer-shell
/// popups (panel menus) are intentionally not listed — this query serves
/// application popup geometry.
fn list_popups(state: &State) -> Vec<PopupInfo> {
    let mut popups = Vec::new();
    for window in state.space.elements() {
        let Some(surface) = window.wl_surface() else {
            continue;
        };
        // This compositor renders window buffers at `element_location` (see
        // render.rs), so a popup's visible rect lands at element_location +
        // the parent's geometry offset (CSD shadow inset; (0,0) for SSD) +
        // the accumulated placement. Verified against Qt (SSD) and GTK4 (CSD)
        // popups via compositor screenshots.
        let root_loc = state.space.element_location(window).unwrap_or_default() + window.geometry().loc;
        for (popup, location) in PopupManager::popups_for_surface(&surface) {
            // The placed rect is the positioner-computed popup geometry (the
            // same committed state `location` accumulates) — NOT
            // `PopupKind::geometry()`, which is the client's optional
            // `set_window_geometry` and may be unset. Until the client acks
            // the initial configure and commits, this rect is empty: such a
            // popup is not mapped yet and must not be reported.
            let placed = compositor::with_states(popup.wl_surface(), |states| {
                states
                    .data_map
                    .get::<XdgPopupSurfaceData>()
                    .map(|attrs| attrs.lock().expect("xdg popup attributes poisoned").current.geometry)
                    .unwrap_or_default()
            });
            if placed.size.w <= 0 || placed.size.h <= 0 {
                continue;
            }
            popups.push(PopupInfo {
                parent_window_id: window_stable_id(window),
                pid: window_pid(window),
                x: root_loc.x + location.x,
                y: root_loc.y + location.y,
                width: placed.size.w,
                height: placed.size.h,
            });
        }
    }
    popups
}

/// List minimized windows as typed structs.
fn list_minimized_windows(state: &State) -> Vec<MinimizedWindowInfo> {
    state
        .minimized_windows
        .iter()
        .enumerate()
        .map(|(idx, (window, pos))| build_minimized_window_info(idx, window, *pos))
        .collect()
}

fn build_minimized_window_info(
    idx: usize,
    window: &Window,
    pos: smithay::utils::Point<i32, Logical>,
) -> MinimizedWindowInfo {
    let size = window.geometry().size;
    MinimizedWindowInfo {
        id: format!("minimized_{idx}"),
        window_id: window_stable_id(window),
        title: foreign_toplevel::window_title(window),
        app_id: foreign_toplevel::window_app_id(window),
        pid: window_pid(window),
        x: pos.x,
        y: pos.y,
        content_width: size.w,
        content_height: size.h,
        minimized: true,
        maximized: is_maximized(window),
    }
}

/// Info about a minimized window, found by its stable id.
fn minimized_window_info(state: &State, window_id: u64) -> Option<MinimizedWindowInfo> {
    state
        .minimized_windows
        .iter()
        .enumerate()
        .find(|(_, (window, _))| window_stable_id(window) == window_id)
        .map(|(idx, (window, pos))| build_minimized_window_info(idx, window, *pos))
}

fn window_stable_id(window: &Window) -> u64 {
    if let Some(x11) = window.x11_surface() {
        return u64::from(x11.window_id());
    }

    if let Some(surface) = window.wl_surface() {
        let mut hasher = DefaultHasher::new();
        format!("{:?}", surface.id()).hash(&mut hasher);
        return hasher.finish();
    }

    0
}

/// What the compositor knows about the process behind a window.
///
/// The two cases are kept apart because only one of them is an identity the
/// compositor established itself, and only such an identity may be used for a
/// decision (see [`excludes_caller`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowOwner {
    /// The peer credentials the compositor read when it accepted the client's
    /// connection. `None` when the peer could not be identified.
    Established(Option<u32>),
    /// The process id an X11 client declared about itself (`_NET_WM_PID`), which
    /// the compositor cannot verify. `None` when it declared none.
    Declared(Option<u32>),
}

impl WindowOwner {
    /// The process id to report, whichever source it came from.
    fn pid(self) -> Option<u32> {
        match self {
            Self::Established(pid) | Self::Declared(pid) => pid,
        }
    }
}

/// Who owns a window, as far as the compositor can tell.
///
/// `XWayland` is a separate source and stays one: an X11-backed window reports
/// what its own client declared, never the peer credentials of `XWayland`'s
/// Wayland connection — which carries no [`ClientState`] at all, because smithay
/// opens that connection itself (`src/xwayland.rs`), and therefore reads as
/// unidentified here.
fn window_owner(window: &Window) -> WindowOwner {
    if let Some(x11) = window.x11_surface() {
        return WindowOwner::Declared(declared_pid(x11.pid()));
    }

    WindowOwner::Established(
        window
            .wl_surface()
            .and_then(|surface| surface.client())
            .and_then(|client| client.get_data::<ClientState>().and_then(|data| data.peer_process)),
    )
}

/// A process id an X11 client declared about itself, normalized.
///
/// The client writes `_NET_WM_PID` and smithay reports the property as it stands
/// (`smithay-0.7.0/src/xwayland/xwm/surface.rs:767` stores any `CARDINAL`), so
/// `0` can arrive here. `0` is the one value this capability never puts on the
/// wire — a consumer filtering by process would match against it — so a client
/// that declares it is reported like a client that declares nothing.
fn declared_pid(declared: Option<u32>) -> Option<u32> {
    declared.filter(|pid| *pid > 0)
}

/// The process id reported for a window, or `None` when it is unknown.
fn window_pid(window: &Window) -> Option<u32> {
    window_owner(window).pid()
}

/// Whether the window at a point belongs to the process that asked, and must
/// therefore be skipped in favour of the window behind it.
///
/// A window is excluded **only on a positive match** of two identities the
/// compositor established itself: the one it read when it accepted the client's
/// connection, and the peer of the control connection the request arrived on.
/// When either side is unknown nothing is excluded — answering with an occluded
/// window on the strength of an identity that was never established is a worse
/// answer than answering with the caller's own window.
///
/// A declared `_NET_WM_PID` is not such an identity: the compositor did not
/// establish it and cannot verify it, so an X11 client must not be able to make
/// its window unpickable by claiming the caller's number.
fn excludes_caller(owner: WindowOwner, caller: Option<u32>) -> bool {
    match (owner, caller) {
        (WindowOwner::Established(Some(owner)), Some(caller)) => owner == caller,
        _ => false,
    }
}

/// The frontmost window at a point that the asking process does not own.
///
/// The compositor owns the authoritative stacking order, so it walks that order
/// itself: `Space::element_under` returns only the frontmost hit and cannot be
/// asked for the next one down, which is what skipping the caller's own window
/// needs. The walk applies smithay's own criterion — the bounding box, then the
/// window's input region at its render location — over `elements()` reversed,
/// front-to-back, the order the renderer relies on (see `render.rs`).
///
/// The window comes back with its index in `elements()`, which is what the
/// response reports as `id`.
fn window_at_point(state: &State, point: (f64, f64), caller: Option<u32>) -> Option<(usize, Window)> {
    let point = Point::<f64, Logical>::from(point);
    state
        .space
        .elements()
        .enumerate()
        .rev()
        .find(|(_, window)| hits_input_region(state, window, point) && !excludes_caller(window_owner(window), caller))
        .map(|(idx, window)| (idx, window.clone()))
}

/// Whether a point hits a window, by the same criterion `Space::element_under`
/// applies: the bounding box first, then the input region at the location the
/// window is actually drawn at — its location in the space minus its geometry
/// offset.
fn hits_input_region(state: &State, window: &Window, point: Point<f64, Logical>) -> bool {
    let Some(bbox) = state.space.element_bbox(window) else {
        return false;
    };
    if !bbox.to_f64().contains(point) {
        return false;
    }

    let Some(location) = state.space.element_location(window) else {
        return false;
    };
    let render_location = location - window.geometry().loc;
    window.is_in_input_region(&(point - render_location.to_f64()))
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

/// Activate a window by index: focus and raise it, like a foreign-toplevel
/// activation request.
///
/// # Panics
///
/// Panics if the seat has no keyboard.
fn focus_window(state: &mut State, id: u64) -> bool {
    let Some(id) = usize::try_from(id).ok() else { return false };
    let window = state.space.elements().nth(id).cloned();
    if let Some(window) = window {
        crate::handlers::foreign_toplevel::activate_window(state, &window);
        true
    } else {
        false
    }
}

fn minimize_window(state: &mut State, id: u64) -> bool {
    let Some(id) = usize::try_from(id).ok() else { return false };
    let window = state.space.elements().nth(id).cloned();
    if let Some(window) = window {
        crate::handlers::foreign_toplevel::minimize_window(state, &window);
        true
    } else {
        false
    }
}

fn maximize_window(state: &mut State, id: u64) -> bool {
    let Some(id) = usize::try_from(id).ok() else { return false };
    let window = state.space.elements().nth(id).cloned();
    if let Some(window) = window {
        crate::handlers::foreign_toplevel::maximize_window(state, &window);
        true
    } else {
        false
    }
}

fn restore_window(state: &mut State, id: u64) -> bool {
    let Some(id) = usize::try_from(id).ok() else { return false };
    let window = state.space.elements().nth(id).cloned();
    if let Some(window) = window {
        crate::handlers::foreign_toplevel::restore_window(state, &window);
        true
    } else {
        false
    }
}

fn move_window(state: &mut State, id: u64, x: i32, y: i32) -> bool {
    let Some(id) = usize::try_from(id).ok() else { return false };
    let window = state.space.elements().nth(id).cloned();
    if let Some(window) = window {
        crate::handlers::foreign_toplevel::move_window(state, &window, (x, y).into());
        true
    } else {
        false
    }
}

fn resize_window(state: &mut State, id: u64, width: i32, height: i32) -> bool {
    let Some(id) = usize::try_from(id).ok() else { return false };
    let window = state.space.elements().nth(id).cloned();
    if let Some(window) = window {
        crate::handlers::foreign_toplevel::resize_window(state, &window, (width, height).into());
        true
    } else {
        false
    }
}

fn parse_i32_coordinate(value: Option<f64>, field: &'static str) -> Result<i32, &'static str> {
    let value = value.ok_or(field)?;
    if !value.is_finite() || value < f64::from(i32::MIN) || value > f64::from(i32::MAX) {
        return Err(field);
    }
    #[allow(clippy::cast_possible_truncation)]
    Ok(value.round() as i32)
}

fn parse_i32_size(value: Option<f64>, field: &'static str) -> Result<i32, &'static str> {
    let parsed = parse_i32_coordinate(value, field)?;
    if parsed <= 0 {
        return Err(field);
    }
    Ok(parsed)
}

// -- Screenshot implementation --

/// Capture the current compositor scene as a base64-encoded PNG.
///
/// Uses the [`GlowRenderer`](smithay::backend::renderer::glow::GlowRenderer)
/// stored in `State::screenshot_renderer`.  For winit and DRM backends this
/// is pre-initialized with a shared EGL context (same GL namespace as the
/// main renderer) so that client surface textures are accessible.  For headless (no main renderer) a standalone offscreen
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

    #[test]
    fn window_frame_bounds_for_ssd_expand_client_rect() {
        let content_bounds: Rectangle<i32, Logical> = Rectangle::new((100, 200).into(), (640, 480).into());
        let frame: Rectangle<i32, Logical> = Rectangle::new(
            (
                content_bounds.loc.x - crate::decorations::RESIZE_BORDER,
                content_bounds.loc.y - crate::decorations::TITLEBAR_HEIGHT - crate::decorations::RESIZE_BORDER,
            )
                .into(),
            (
                content_bounds.size.w + crate::decorations::RESIZE_BORDER * 2,
                content_bounds.size.h + crate::decorations::TITLEBAR_HEIGHT + crate::decorations::RESIZE_BORDER * 2,
            )
                .into(),
        );

        assert_eq!(frame.loc.x, 92);
        assert_eq!(frame.loc.y, 162);
        assert_eq!(frame.size.w, 656);
        assert_eq!(frame.size.h, 526);
    }

    #[test]
    fn csd_frame_origin_applies_geometry_offset() {
        let content_bounds: Rectangle<i32, Logical> = Rectangle::new((50, 70).into(), (800, 600).into());
        let geometry: Rectangle<i32, Logical> = Rectangle::new((6, 8).into(), (800, 600).into());
        let frame: Rectangle<i32, Logical> = Rectangle::new(
            (content_bounds.loc.x + geometry.loc.x, content_bounds.loc.y + geometry.loc.y).into(),
            geometry.size,
        );

        assert_eq!(frame.loc.x, 56);
        assert_eq!(frame.loc.y, 78);
        assert_eq!(frame.size.w, 800);
        assert_eq!(frame.size.h, 600);
    }

    fn window_info(pid: Option<u32>) -> WindowInfo {
        WindowInfo {
            id: 3,
            window_id: 0x1234_5678,
            title: "Fixture".to_string(),
            app_id: "test.fixture".to_string(),
            pid,
            x: 10,
            y: 20,
            width: 300,
            height: 200,
            content_x: 10,
            content_y: 20,
            content_width: 300,
            content_height: 200,
            geometry_x: 0,
            geometry_y: 0,
            focused: true,
            minimized: false,
            maximized: false,
            fullscreen: false,
            decoration_mode: "csd",
            opaque_region: None,
        }
    }

    fn popup_info(pid: Option<u32>) -> PopupInfo {
        PopupInfo { parent_window_id: 0x1234_5678, pid, x: 41, y: 61, width: 100, height: 80 }
    }

    fn minimized_window_info(pid: Option<u32>) -> MinimizedWindowInfo {
        MinimizedWindowInfo {
            id: "minimized_0".to_string(),
            window_id: 0x1234_5678,
            title: "Fixture".to_string(),
            app_id: "test.fixture".to_string(),
            pid,
            x: 10,
            y: 20,
            content_width: 300,
            content_height: 200,
            minimized: true,
            maximized: false,
        }
    }

    /// A client whose process the compositor could not identify is reported as
    /// JSON `null` — never as `0`, which a consumer filtering by PID would
    /// take for a real process. Every other field keeps its ordinary value, so
    /// the entry differs from an identified one in the `pid` alone.
    #[test]
    fn an_unknown_process_is_reported_as_null_and_never_as_zero() {
        let entries = [
            (
                serde_json::to_string(&window_info(None)).expect("window info serializes"),
                serde_json::to_string(&window_info(Some(4242))).expect("window info serializes"),
            ),
            (
                serde_json::to_string(&popup_info(None)).expect("popup info serializes"),
                serde_json::to_string(&popup_info(Some(4242))).expect("popup info serializes"),
            ),
            (
                serde_json::to_string(&minimized_window_info(None)).expect("minimized info serializes"),
                serde_json::to_string(&minimized_window_info(Some(4242))).expect("minimized info serializes"),
            ),
        ];

        for (unknown, identified) in &entries {
            assert!(unknown.contains(r#""pid":null"#), "unknown process must serialize as null: {unknown}");
            assert!(!unknown.contains(r#""pid":0"#), "0 is never reported: {unknown}");
            assert!(
                serde_json::from_str::<serde_json::Value>(unknown).is_ok(),
                "entry must stay valid JSON: {unknown}"
            );
            assert_eq!(
                &unknown.replace(r#""pid":null"#, r#""pid":4242"#),
                identified,
                "the two entries must differ in the pid alone"
            );
        }
    }

    /// The point lookup excludes a window only on a positive match of two
    /// identities the compositor established itself.
    #[test]
    fn a_window_of_the_calling_process_is_skipped() {
        assert!(excludes_caller(WindowOwner::Established(Some(4242)), Some(4242)));
    }

    #[test]
    fn a_window_of_another_process_is_kept() {
        assert!(!excludes_caller(WindowOwner::Established(Some(4242)), Some(99)));
    }

    #[test]
    fn a_window_whose_client_was_not_identified_is_kept() {
        assert!(!excludes_caller(WindowOwner::Established(None), Some(4242)));
        assert!(!excludes_caller(WindowOwner::Established(None), None));
    }

    #[test]
    fn an_unidentified_caller_excludes_nothing() {
        assert!(!excludes_caller(WindowOwner::Established(Some(4242)), None));
    }

    /// `_NET_WM_PID` is whatever the X11 client wrote, so `0` can arrive — and
    /// `0` is the one value that never reaches the wire, whatever its source.
    #[test]
    fn a_declared_process_id_of_zero_is_no_process() {
        assert_eq!(declared_pid(Some(0)), None);
        assert_eq!(declared_pid(None), None);
        assert_eq!(declared_pid(Some(4242)), Some(4242));
    }

    /// A declared `_NET_WM_PID` is not an identity: an X11 client could
    /// otherwise make its window unpickable by claiming the caller's number.
    #[test]
    fn a_declared_process_id_never_excludes() {
        assert!(!excludes_caller(WindowOwner::Declared(Some(4242)), Some(4242)));
    }
}
