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
//! - `{"command": "ping"}` → alias for `status`
//! - `{"command": "shutdown"}` → request compositor shutdown

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::gles::GlesTexture;
use smithay::backend::renderer::{Bind, ExportMem, Offscreen};
use smithay::desktop::Window;
use smithay::utils::{Physical, Size, Transform};
use smithay::wayland::compositor;
use smithay::wayland::shell::xdg::XdgToplevelSurfaceData;

use crate::state::State;

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

    // Remove stale socket if it exists
    if path.exists() {
        std::fs::remove_file(&path)?;
    }

    let listener = UnixListener::bind(&path)?;
    listener.set_nonblocking(true)?;

    tracing::info!(path = %path.display(), "control socket listening");

    // Register with calloop using a Generic source
    loop_handle.insert_source(
        calloop::generic::Generic::new(listener, calloop::Interest::READ, calloop::Mode::Level),
        |_, listener, state| {
            // Accept all pending connections
            loop {
                match listener.accept() {
                    Ok((stream, _addr)) => {
                        if let Err(err) = handle_client(stream, state) {
                            tracing::warn!(%err, "error handling control client");
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

/// Handle a single control client connection.
///
/// Reads commands line by line and sends responses.
fn handle_client(stream: UnixStream, state: &mut State) -> Result<(), Box<dyn std::error::Error>> {
    #![allow(clippy::needless_pass_by_value)] // UnixStream needs ownership for set_* calls
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(5)))?;

    let reader = BufReader::new(&stream);
    let mut writer = &stream;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(err) => return Err(err.into()),
        };

        if line.trim().is_empty() {
            continue;
        }

        let response = process_command(&line, state);
        writeln!(writer, "{response}")?;
        writer.flush()?;
    }

    Ok(())
}

/// Process a single JSON command and return a JSON response string.
fn process_command(input: &str, state: &mut State) -> String {
    // Simple JSON parsing without external dependencies.
    // We parse the "command" field manually.
    let input = input.trim();

    let command = extract_json_string(input, "command");
    let id = extract_json_u64(input, "id");
    let app_id = extract_json_string(input, "app_id");
    let title = extract_json_string(input, "title");

    match command.as_deref() {
        Some("ping" | "status") => build_status_response(state),

        Some("shutdown") => {
            state.running = false;
            r#"{"status":"ok","message":"shutting down"}"#.to_string()
        }

        Some("list_windows") => {
            let windows = list_windows(state);
            let minimized = list_minimized_windows(state);
            format!(r#"{{"status":"ok","windows":{windows},"minimized":{minimized}}}"#)
        }

        Some("get_window") => match resolve_window_selector(state, id, app_id.as_deref(), title.as_deref()) {
            Some(info) => format!(r#"{{"status":"ok","window":{info}}}"#),
            None => r#"{"status":"error","message":"window not found"}"#.to_string(),
        },

        Some("close_window") => {
            match resolve_and_act_on_window(state, id, app_id.as_deref(), title.as_deref(), |state, idx| {
                close_window(state, idx)
            }) {
                Some((t, a)) => format!(r#"{{"status":"ok","message":"close sent","title":"{t}","app_id":"{a}"}}"#),
                None => r#"{"status":"error","message":"window not found"}"#.to_string(),
            }
        }

        Some("focus_window") => {
            match resolve_and_act_on_window(state, id, app_id.as_deref(), title.as_deref(), |state, idx| {
                focus_window(state, idx)
            }) {
                Some((t, a)) => {
                    format!(r#"{{"status":"ok","message":"window focused","title":"{t}","app_id":"{a}"}}"#)
                }
                None => r#"{"status":"error","message":"window not found"}"#.to_string(),
            }
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
                format!(
                    r#"{{"status":"ok","format":"png","width":{phys_w},"height":{phys_h},"scale":{max_scale},"data":"{base64_png}"}}"#,
                )
            }
            Err(err) => {
                format!(r#"{{"status":"error","message":"screenshot failed: {err}"}}"#)
            }
        },

        Some(cmd) => {
            format!(r#"{{"status":"error","message":"unknown command: {cmd}"}}"#)
        }

        None => r#"{"status":"error","message":"missing or invalid command field"}"#.to_string(),
    }
}

/// Build the JSON response for the `status` command.
fn build_status_response(state: &State) -> String {
    let uptime_secs = state.start_time.elapsed().as_secs();
    let version = env!("CARGO_PKG_VERSION");
    let backend = state.backend_name;
    let window_count = state.space.elements().count();
    let minimized_count = state.minimized_windows.len();
    let socket = &state.socket_name;
    let xwayland = state.xwayland.is_some();

    // Build outputs array
    let outputs: Vec<String> = state
        .outputs
        .iter()
        .enumerate()
        .map(|(i, o)| {
            let name = o.name();
            let mode = o.current_mode().unwrap_or(smithay::output::Mode { size: (0, 0).into(), refresh: 0 });
            let scale = o.current_scale().fractional_scale();
            let loc = state.space.output_geometry(o).map(|g| g.loc).unwrap_or_default();
            format!(
                r#"{{"index":{i},"name":"{name}","width":{w},"height":{h},"x":{x},"y":{y},"scale":{scale}}}"#,
                w = mode.size.w,
                h = mode.size.h,
                x = loc.x,
                y = loc.y,
            )
        })
        .collect();

    format!(
        r#"{{"status":"ok","version":"{version}","backend":"{backend}","uptime_secs":{uptime_secs},"socket":"{socket}","xwayland":{xwayland},"windows":{window_count},"minimized":{minimized_count},"outputs":[{outputs}]}}"#,
        outputs = outputs.join(","),
    )
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
) -> Option<String> {
    if let Some(id) = id {
        return get_window_info(state, id);
    }
    if let Some(app_id_query) = app_id {
        for (idx, window) in state.space.elements().enumerate() {
            if window_app_id(window) == app_id_query {
                return Some(format_window_info(state, idx, window));
            }
        }
        // Fall through to title matching
    }
    if let Some(title_query) = title {
        let query_lower = title_query.to_lowercase();
        for (idx, window) in state.space.elements().enumerate() {
            if window_title(window).to_lowercase().contains(&query_lower) {
                return Some(format_window_info(state, idx, window));
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
    let t = window_title(window);
    let a = window_app_id(window);
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
            if window_app_id(window) == app_id_query {
                return Some(idx);
            }
        }
        // Fall through to title matching
    }
    if let Some(title_query) = title {
        let query_lower = title_query.to_lowercase();
        for (idx, window) in state.space.elements().enumerate() {
            if window_title(window).to_lowercase().contains(&query_lower) {
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

/// Format window info as a JSON object, including state flags.
fn format_window_info(state: &State, idx: usize, window: &Window) -> String {
    let title = window_title(window);
    let app_id = window_app_id(window);
    let loc = state.space.element_location(window).unwrap_or_default();
    let geo = window.geometry();
    let focused = is_focused(state, window);
    let maximized = is_maximized(window);
    let fullscreen = is_fullscreen(window);

    format!(
        r#"{{"id":{idx},"title":"{title}","app_id":"{app_id}","x":{x},"y":{y},"width":{w},"height":{h},"focused":{focused},"maximized":{maximized},"fullscreen":{fullscreen}}}"#,
        x = loc.x,
        y = loc.y,
        w = geo.size.w,
        h = geo.size.h,
    )
}

/// List all mapped windows as a JSON array string.
fn list_windows(state: &State) -> String {
    let entries: Vec<String> =
        state.space.elements().enumerate().map(|(idx, window)| format_window_info(state, idx, window)).collect();

    format!("[{}]", entries.join(","))
}

/// List minimized windows as a JSON array string.
fn list_minimized_windows(state: &State) -> String {
    let entries: Vec<String> = state
        .minimized_windows
        .iter()
        .enumerate()
        .map(|(idx, (window, pos))| {
            let title = window_title(window);
            let app_id = window_app_id(window);
            format!(
                r#"{{"id":"minimized_{idx}","title":"{title}","app_id":"{app_id}","x":{x},"y":{y}}}"#,
                x = pos.x,
                y = pos.y,
            )
        })
        .collect();

    format!("[{}]", entries.join(","))
}

/// Get info about a specific window by index.
fn get_window_info(state: &State, id: u64) -> Option<String> {
    let id = usize::try_from(id).ok()?;
    let (idx, window) = state.space.elements().enumerate().nth(id)?;
    Some(format_window_info(state, idx, window))
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
        let keyboard = state.seat.get_keyboard().unwrap();
        keyboard.set_focus(state, Some(crate::focus::KeyboardFocusTarget::Window(window.clone())), serial);
        state.space.raise_element(&window, true);
        true
    } else {
        false
    }
}

/// Get the title of a window.
fn window_title(window: &Window) -> String {
    window
        .toplevel()
        .and_then(|t| {
            compositor::with_states(t.wl_surface(), |states| {
                states
                    .data_map
                    .get::<XdgToplevelSurfaceData>()
                    .and_then(|data| data.lock().ok())
                    .and_then(|data| data.title.clone())
            })
        })
        .unwrap_or_default()
        .replace('"', "\\\"")
}

/// Get the `app_id` of a window.
fn window_app_id(window: &Window) -> String {
    window
        .toplevel()
        .and_then(|t| {
            compositor::with_states(t.wl_surface(), |states| {
                states
                    .data_map
                    .get::<XdgToplevelSurfaceData>()
                    .and_then(|data| data.lock().ok())
                    .and_then(|data| data.app_id.clone())
            })
        })
        .unwrap_or_default()
        .replace('"', "\\\"")
}

// -- Simple JSON field extraction (no serde dependency needed) --

/// Extract a string value for a given key from a JSON object string.
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!(r#""{key}""#);
    let key_pos = json.find(&pattern)?;
    let after_key = &json[key_pos + pattern.len()..];
    // Skip whitespace and colon
    let after_colon = after_key.trim_start().strip_prefix(':')?;
    let after_colon = after_colon.trim_start();
    // Must start with a quote
    let after_quote = after_colon.strip_prefix('"')?;
    // Find the closing quote (handle escaped quotes)
    let mut chars = after_quote.chars();
    let mut result = String::new();
    let mut escaped = false;
    for ch in &mut chars {
        if escaped {
            result.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some(result);
        } else {
            result.push(ch);
        }
    }
    None
}

/// Extract a `u64` value for a given key from a JSON object string.
fn extract_json_u64(json: &str, key: &str) -> Option<u64> {
    let pattern = format!(r#""{key}""#);
    let key_pos = json.find(&pattern)?;
    let after_key = &json[key_pos + pattern.len()..];
    let after_colon = after_key.trim_start().strip_prefix(':')?.trim_start();
    // Parse digits
    let num_str: String = after_colon.chars().take_while(char::is_ascii_digit).collect();
    num_str.parse().ok()
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
    let mut renderer = state.screenshot_renderer.take().unwrap();

    let result = take_screenshot_impl(&mut renderer, state, width, height, max_scale);

    // Put the renderer back for reuse
    state.screenshot_renderer = Some(renderer);

    result
}

/// Inner screenshot implementation using a [`GlowRenderer`](smithay::backend::renderer::glow::GlowRenderer).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn take_screenshot_impl(
    renderer: &mut smithay::backend::renderer::glow::GlowRenderer,
    state: &mut crate::state::State,
    width: i32,
    height: i32,
    scale: f64,
) -> Result<String, String> {
    use smithay::backend::allocator::Fourcc as DrmFourcc;
    use smithay::utils::Rectangle;

    // Scale logical dimensions to physical pixels.
    let phys_w = (f64::from(width) * scale).ceil() as i32;
    let phys_h = (f64::from(height) * scale).ceil() as i32;

    let size: Size<i32, Physical> = (phys_w, phys_h).into();
    let buffer_size: Size<i32, smithay::utils::Buffer> = (phys_w, phys_h).into();

    // Lazy-init the screenshot titlebar painter on the screenshot renderer's
    // GL context.  VAOs are per-context in OpenGL — they are NOT shared even
    // when contexts share textures via EGLContext::new_shared.  Using the main
    // titlebar renderer here would cause GL_INVALID_OPERATION on glBindVertexArray.
    if !state.screenshot_titlebar_renderer.is_glow_initialized() {
        state.screenshot_titlebar_renderer.init_glow(renderer);
    }

    // Create offscreen GL texture (Abgr8888 for GL compatibility)
    let mut texture: GlesTexture =
        renderer.create_buffer(DrmFourcc::Abgr8888, buffer_size).map_err(|e| format!("create_buffer: {e}"))?;

    // Bind the texture as render target
    let mut framebuffer = renderer.bind(&mut texture).map_err(|e| format!("bind: {e}"))?;

    // Swap in the screenshot titlebar renderer (with its own VAO on this GL
    // context) so that collect_render_elements paints egui titlebars using
    // the correct context.
    std::mem::swap(&mut state.titlebar_renderer, &mut state.screenshot_titlebar_renderer);

    // Collect render elements
    let output = state.output.clone();
    let render_elements = crate::render::collect_render_elements(renderer, state, &output);

    // Swap back so the main render loop keeps its own titlebar renderer.
    std::mem::swap(&mut state.titlebar_renderer, &mut state.screenshot_titlebar_renderer);

    // Render into the offscreen buffer using damage tracker
    let mut damage_tracker = OutputDamageTracker::new(size, scale, Transform::Normal);
    damage_tracker
        .render_output(renderer, &mut framebuffer, 0, &render_elements, [0.1, 0.1, 0.1, 1.0])
        .map_err(|e| format!("render_output: {e}"))?;

    // Read back pixels
    let region = Rectangle::from_size(buffer_size);
    let mapping = renderer
        .copy_framebuffer(&framebuffer, region, DrmFourcc::Abgr8888)
        .map_err(|e| format!("copy_framebuffer: {e}"))?;

    let pixel_data = renderer.map_texture(&mapping).map_err(|e| format!("map_texture: {e}"))?;

    // Abgr8888 in DRM fourcc = GL's RGBA byte order → already R, G, B, A in memory.
    let w = u32::try_from(phys_w).map_err(|e| format!("width: {e}"))?;
    let h = u32::try_from(phys_h).map_err(|e| format!("height: {e}"))?;

    // Encode as PNG (pixel data is already RGBA)
    let png_data = encode_png(pixel_data, w, h).map_err(|e| format!("PNG encode: {e}"))?;

    // base64-encode the PNG for JSON transport
    Ok(base64_encode(&png_data))
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
    fn extract_string_simple() {
        let json = r#"{"command": "ping"}"#;
        assert_eq!(extract_json_string(json, "command").as_deref(), Some("ping"));
    }

    #[test]
    fn extract_string_with_spaces() {
        let json = r#"{ "command" : "list_windows" }"#;
        assert_eq!(extract_json_string(json, "command").as_deref(), Some("list_windows"));
    }

    #[test]
    fn extract_string_escaped_quote() {
        let json = r#"{"title": "hello \"world\""}"#;
        assert_eq!(extract_json_string(json, "title").as_deref(), Some(r#"hello "world""#));
    }

    #[test]
    fn extract_string_missing_key() {
        let json = r#"{"command": "ping"}"#;
        assert_eq!(extract_json_string(json, "missing"), None);
    }

    #[test]
    fn extract_u64_simple() {
        let json = r#"{"command": "get_window", "id": 42}"#;
        assert_eq!(extract_json_u64(json, "id"), Some(42));
    }

    #[test]
    fn extract_u64_missing() {
        let json = r#"{"command": "list_windows"}"#;
        assert_eq!(extract_json_u64(json, "id"), None);
    }

    #[test]
    fn extract_u64_zero() {
        let json = r#"{"id": 0}"#;
        assert_eq!(extract_json_u64(json, "id"), Some(0));
    }

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
