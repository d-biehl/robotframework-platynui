#![allow(unused_crate_dependencies)]

//! IPC integration tests — start the compositor and exercise control commands.
//!
//! These tests start `platynui-wayland-compositor` as a subprocess, connect to the control
//! socket, and verify all IPC commands.
//!
//! By default the **headless** backend is used. Set `PLATYNUI_TEST_BACKEND=winit` to use
//! the winit backend instead — this opens a visible window so you can watch what happens:
//!
//! ```sh
//! PLATYNUI_TEST_BACKEND=winit cargo nextest run -p platynui-wayland-compositor --test ipc_tests
//! ```
//!
//! The tests require EGL support (hardware GPU or `LIBGL_ALWAYS_SOFTWARE=1`).
//! They are skipped gracefully if the compositor cannot start.

// This entire test suite only applies to Linux (Wayland compositor).
#![cfg(target_os = "linux")]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use platynui_wayland_compositor as _;
use serde_json::Value;

// ─── Helpers ────────────────────────────────────────────────────────────

/// Helper: determine the backend to use for tests.
///
/// Returns `"winit"` when `PLATYNUI_TEST_BACKEND=winit` is set, otherwise `"headless"`.
fn test_backend() -> &'static str {
    static BACKEND: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let val = BACKEND.get_or_init(|| {
        std::env::var("PLATYNUI_TEST_BACKEND").unwrap_or_else(|_| "headless".to_string()).to_lowercase()
    });
    val.as_str()
}

/// Helper: start the compositor with a unique socket name and return (child, socket name).
fn start_compositor(test_name: &str) -> Option<(Child, String)> {
    let socket_name = format!("platynui-test-{test_name}-{}", std::process::id());
    let binary = env!("CARGO_BIN_EXE_platynui-wayland-compositor");
    let backend = test_backend();

    let mut cmd = Command::new(binary);
    cmd.args(["--backend", backend, "--socket-name", &socket_name, "--timeout", "30"]);

    // Only suppress stdout/stderr in headless mode — with winit we want to see output.
    if backend == "headless" {
        cmd.env("LIBGL_ALWAYS_SOFTWARE", "1").stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    }

    match cmd.spawn() {
        Ok(child) => Some((child, socket_name)),
        Err(err) => {
            eprintln!("skipping IPC test: cannot start compositor: {err}");
            None
        }
    }
}

/// Helper: wait for the control socket to appear.
fn wait_for_socket(socket_name: &str, timeout: Duration) -> Option<PathBuf> {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    let socket_path = PathBuf::from(&runtime_dir).join(format!("{socket_name}.control"));

    let start = Instant::now();
    while start.elapsed() < timeout {
        if socket_path.exists() {
            // Give the compositor a moment to start listening
            std::thread::sleep(Duration::from_millis(100));
            return Some(socket_path);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

/// Helper: send a JSON command and receive the response.
fn send_command(socket_path: &PathBuf, command: &str) -> Result<String, Box<dyn std::error::Error>> {
    let stream = UnixStream::connect(socket_path)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let mut writer = &stream;
    writeln!(writer, "{command}")?;
    writer.flush()?;

    let mut reader = BufReader::new(&stream);
    let mut response = String::new();
    reader.read_line(&mut response)?;
    Ok(response)
}

/// Helper: cleanup — send shutdown and wait for the child to exit.
fn shutdown_compositor(socket_path: &PathBuf, mut child: Child) {
    let _ = send_command(socket_path, r#"{"command": "shutdown"}"#);
    let _ = child.wait();
}

/// Kill the compositor and return its captured stdout+stderr (headless mode pipes
/// both). Used to surface the real cause when a command yields no proper response —
/// e.g. the compositor dying mid-render instead of returning an ok/error reply.
fn dump_child_output(mut child: Child) -> String {
    use std::io::Read;

    let _ = child.kill();
    let _ = child.wait();

    let mut out = String::new();
    if let Some(mut stdout) = child.stdout.take() {
        let mut s = String::new();
        let _ = stdout.read_to_string(&mut s);
        out.push_str("[stdout]\n");
        out.push_str(&s);
    }
    if let Some(mut stderr) = child.stderr.take() {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        out.push_str("\n[stderr]\n");
        out.push_str(&s);
    }
    out
}

#[test]
fn ipc_ping() {
    let Some((child, socket_name)) = start_compositor("ping") else {
        return;
    };

    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    // `ping` is an alias for `status` — returns compositor info
    let response = send_command(&socket_path, r#"{"command": "ping"}"#).expect("failed to send ping");
    assert!(response.contains(r#""status":"ok"#), "unexpected response: {response}");
    assert!(response.contains(r#""version":"#), "expected version in response: {response}");
    let expected_backend = format!(r#""backend":"{}""#, test_backend());
    assert!(response.contains(&expected_backend), "expected {expected_backend} in response: {response}");

    shutdown_compositor(&socket_path, child);
}

#[test]
fn ipc_status() {
    let Some((child, socket_name)) = start_compositor("status") else {
        return;
    };

    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    let response = send_command(&socket_path, r#"{"command": "status"}"#).expect("failed to send status");
    assert!(response.contains(r#""status":"ok"#), "unexpected response: {response}");
    assert!(response.contains(r#""version":"#), "expected version: {response}");
    let expected_backend = format!(r#""backend":"{}""#, test_backend());
    assert!(response.contains(&expected_backend), "expected {expected_backend} in response: {response}");
    assert!(response.contains(r#""uptime_secs":"#), "expected uptime: {response}");
    assert!(response.contains(r#""windows":"#), "expected windows count: {response}");
    assert!(response.contains(r#""outputs":["#), "expected outputs array: {response}");

    shutdown_compositor(&socket_path, child);
}

#[test]
fn ipc_list_windows_empty() {
    let Some((child, socket_name)) = start_compositor("list_empty") else {
        return;
    };

    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    let response = send_command(&socket_path, r#"{"command": "list_windows"}"#).expect("failed to send list_windows");
    assert!(response.contains(r#""status":"ok"#), "unexpected response: {response}");
    assert!(response.contains(r#""windows":[]"#), "expected empty windows list: {response}");
    assert!(response.contains(r#""minimized":[]"#), "expected empty minimized list: {response}");

    shutdown_compositor(&socket_path, child);
}

#[test]
fn ipc_get_window_not_found() {
    let Some((child, socket_name)) = start_compositor("get_notfound") else {
        return;
    };

    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    let response =
        send_command(&socket_path, r#"{"command": "get_window", "id": 0}"#).expect("failed to send get_window");
    assert!(response.contains(r#""status":"error"#), "unexpected response: {response}");
    assert!(response.contains("window not found"), "unexpected response: {response}");

    shutdown_compositor(&socket_path, child);
}

#[test]
fn ipc_close_window_not_found() {
    let Some((child, socket_name)) = start_compositor("close_notfound") else {
        return;
    };

    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    let response =
        send_command(&socket_path, r#"{"command": "close_window", "id": 999}"#).expect("failed to send close_window");
    assert!(response.contains(r#""status":"error"#), "unexpected: {response}");
    assert!(response.contains("window not found"), "unexpected: {response}");

    shutdown_compositor(&socket_path, child);
}

#[test]
fn ipc_focus_window_not_found() {
    let Some((child, socket_name)) = start_compositor("focus_notfound") else {
        return;
    };

    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    let response =
        send_command(&socket_path, r#"{"command": "focus_window", "id": 999}"#).expect("failed to send focus_window");
    assert!(response.contains(r#""status":"error"#), "unexpected: {response}");
    assert!(response.contains("window not found"), "unexpected: {response}");

    shutdown_compositor(&socket_path, child);
}

#[test]
fn ipc_unknown_command() {
    let Some((child, socket_name)) = start_compositor("unknown_cmd") else {
        return;
    };

    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    let response = send_command(&socket_path, r#"{"command": "nonexistent"}"#).expect("failed to send unknown command");
    assert!(response.contains(r#""status":"error"#), "unexpected response: {response}");
    assert!(response.contains("unknown command"), "unexpected response: {response}");

    shutdown_compositor(&socket_path, child);
}

#[test]
fn ipc_invalid_json() {
    let Some((child, socket_name)) = start_compositor("invalid_json") else {
        return;
    };

    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    let response = send_command(&socket_path, "not valid json").expect("failed to send invalid json");
    assert!(response.contains(r#""status":"error"#), "unexpected response: {response}");

    shutdown_compositor(&socket_path, child);
}

#[test]
fn ipc_highlight_commands() {
    let Some((child, socket_name)) = start_compositor("highlight") else {
        return;
    };

    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    let show = send_command(
        &socket_path,
        r#"{"command": "show_highlight", "rects": [{"x": 10, "y": 20, "width": 120, "height": 80}], "duration_ms": 250}"#,
    )
    .expect("failed to send show_highlight");
    assert!(show.contains(r#""status":"ok""#), "unexpected response: {show}");

    let clear =
        send_command(&socket_path, r#"{"command": "clear_highlight"}"#).expect("failed to send clear_highlight");
    assert!(clear.contains(r#""status":"ok""#), "unexpected response: {clear}");

    shutdown_compositor(&socket_path, child);
}

#[test]
fn ipc_missing_command_field() {
    let Some((child, socket_name)) = start_compositor("no_cmd_field") else {
        return;
    };

    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    // Valid JSON but no "command" key
    let response =
        send_command(&socket_path, r#"{"action": "ping"}"#).expect("failed to send command without command field");
    assert!(response.contains(r#""status":"error"#), "unexpected: {response}");

    shutdown_compositor(&socket_path, child);
}

#[test]
fn ipc_screenshot() {
    let Some((child, socket_name)) = start_compositor("screenshot") else {
        return;
    };

    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    let response = send_command(&socket_path, r#"{"command": "screenshot"}"#).expect("failed to send screenshot");

    // Screenshot may succeed or fail depending on GPU availability — both are valid.
    // The test only verifies the IPC protocol flow (send command → receive response).
    // A response that is neither ok nor error means the compositor died mid-render;
    // dump its captured output so the real cause is visible instead of a bare assert.
    let is_ok = response.contains(r#""status":"ok"#);
    let is_err = response.contains(r#""status":"error"#);
    if !is_ok && !is_err {
        let logs = dump_child_output(child);
        panic!("unexpected screenshot response: {response:?}\n--- compositor output ---\n{logs}");
    }

    if is_ok {
        // Verify the ok response contains the expected metadata fields.
        // We intentionally skip checking `"data":` — the base64 payload can be very large
        // and read_line may behave differently depending on transport buffering.
        assert!(
            response.contains(r#""format":"png"#),
            "missing format field: {}",
            &response[..response.len().min(200)]
        );
    } else {
        // Screenshot failed due to missing GPU — acceptable in CI
        eprintln!("screenshot failed (expected in environments without GPU)");
    }

    shutdown_compositor(&socket_path, child);
}

// ─── Raw Wayland popup client fixture ───────────────────────────────────
//
// The egui test app cannot create xdg_popups (its menus render in-window), so
// the popup tests speak the Wayland protocol directly: map a toplevel, then an
// xdg_popup anchored inside it, and keep the connection alive while the test
// queries `list_popups`.

mod popup_client {
    use std::os::fd::AsFd;
    use std::os::unix::net::UnixStream;
    use std::path::Path;
    use std::time::{Duration, Instant};

    use wayland_client::protocol::{wl_buffer, wl_compositor, wl_registry, wl_shm, wl_shm_pool, wl_surface};
    use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};
    use wayland_protocols::xdg::shell::client::{xdg_popup, xdg_positioner, xdg_surface, xdg_toplevel, xdg_wm_base};

    pub const TOPLEVEL_SIZE: (i32, i32) = (300, 200);
    /// Anchor rect origin inside the toplevel; with a 1×1 rect and
    /// bottom-right anchor/gravity the popup's top-left lands at +1/+1 of it.
    pub const POPUP_ANCHOR: (i32, i32) = (30, 40);
    pub const POPUP_OFFSET: (i32, i32) = (POPUP_ANCHOR.0 + 1, POPUP_ANCHOR.1 + 1);
    pub const POPUP_SIZE: (i32, i32) = (100, 80);

    #[derive(Clone, Copy)]
    pub enum SurfaceRole {
        Toplevel,
        Popup,
    }

    #[derive(Default)]
    pub struct State {
        compositor: Option<wl_compositor::WlCompositor>,
        shm: Option<wl_shm::WlShm>,
        wm_base: Option<xdg_wm_base::XdgWmBase>,
        toplevel_configured: bool,
        popup_configured: bool,
    }

    impl Dispatch<wl_registry::WlRegistry, ()> for State {
        fn event(
            state: &mut Self,
            registry: &wl_registry::WlRegistry,
            event: wl_registry::Event,
            (): &(),
            _conn: &Connection,
            qh: &QueueHandle<Self>,
        ) {
            if let wl_registry::Event::Global { name, interface, version } = event {
                match interface.as_str() {
                    "wl_compositor" => {
                        state.compositor = Some(registry.bind(name, version.min(4), qh, ()));
                    }
                    "wl_shm" => {
                        state.shm = Some(registry.bind(name, 1, qh, ()));
                    }
                    "xdg_wm_base" => {
                        state.wm_base = Some(registry.bind(name, version.min(2), qh, ()));
                    }
                    _ => {}
                }
            }
        }
    }

    impl Dispatch<xdg_wm_base::XdgWmBase, ()> for State {
        fn event(
            _state: &mut Self,
            wm_base: &xdg_wm_base::XdgWmBase,
            event: xdg_wm_base::Event,
            (): &(),
            _conn: &Connection,
            _qh: &QueueHandle<Self>,
        ) {
            if let xdg_wm_base::Event::Ping { serial } = event {
                wm_base.pong(serial);
            }
        }
    }

    impl Dispatch<xdg_surface::XdgSurface, SurfaceRole> for State {
        fn event(
            state: &mut Self,
            surface: &xdg_surface::XdgSurface,
            event: xdg_surface::Event,
            role: &SurfaceRole,
            _conn: &Connection,
            _qh: &QueueHandle<Self>,
        ) {
            if let xdg_surface::Event::Configure { serial } = event {
                surface.ack_configure(serial);
                match role {
                    SurfaceRole::Toplevel => state.toplevel_configured = true,
                    SurfaceRole::Popup => state.popup_configured = true,
                }
            }
        }
    }

    wayland_client::delegate_noop!(State: ignore wl_compositor::WlCompositor);
    wayland_client::delegate_noop!(State: ignore wl_surface::WlSurface);
    wayland_client::delegate_noop!(State: ignore wl_shm::WlShm);
    wayland_client::delegate_noop!(State: ignore wl_shm_pool::WlShmPool);
    wayland_client::delegate_noop!(State: ignore wl_buffer::WlBuffer);
    wayland_client::delegate_noop!(State: ignore xdg_positioner::XdgPositioner);
    wayland_client::delegate_noop!(State: ignore xdg_toplevel::XdgToplevel);
    wayland_client::delegate_noop!(State: ignore xdg_popup::XdgPopup);

    /// A connected client holding a mapped toplevel and one mapped popup.
    /// Dropping it closes the connection (and thereby dismisses everything).
    pub struct Fixture {
        state: State,
        queue: EventQueue<State>,
        popup: xdg_popup::XdgPopup,
        popup_xdg_surface: xdg_surface::XdgSurface,
        popup_surface: wl_surface::WlSurface,
    }

    impl Fixture {
        /// Destroy the popup and flush, so the compositor drops it from its
        /// popup tree while toplevel and connection stay alive.
        pub fn dismiss_popup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
            self.popup.destroy();
            self.popup_xdg_surface.destroy();
            self.popup_surface.destroy();
            self.queue.roundtrip(&mut self.state)?;
            Ok(())
        }
    }

    fn create_buffer(
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<State>,
        (width, height): (i32, i32),
    ) -> Result<wl_buffer::WlBuffer, Box<dyn std::error::Error>> {
        let stride = width * 4;
        let size = stride * height;
        let file = tempfile::tempfile()?;
        file.set_len(u64::try_from(size)?)?;
        let pool = shm.create_pool(file.as_fd(), size, qh, ());
        Ok(pool.create_buffer(0, width, height, stride, wl_shm::Format::Xrgb8888, qh, ()))
    }

    fn dispatch_until(
        queue: &mut EventQueue<State>,
        state: &mut State,
        what: &str,
        cond: impl Fn(&State) -> bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !cond(state) {
            if Instant::now() > deadline {
                return Err(format!("timed out waiting for {what}").into());
            }
            queue.blocking_dispatch(state)?;
        }
        Ok(())
    }

    /// Connect to the compositor socket, map a [`TOPLEVEL_SIZE`] toplevel and
    /// a [`POPUP_SIZE`] popup at [`POPUP_OFFSET`] inside it.
    pub fn open_toplevel_with_popup(
        runtime_dir: &Path,
        socket_name: &str,
        app_id: &str,
    ) -> Result<Fixture, Box<dyn std::error::Error>> {
        let stream = UnixStream::connect(runtime_dir.join(socket_name))?;
        let conn = Connection::from_socket(stream)?;
        let mut queue = conn.new_event_queue();
        let qh = queue.handle();
        let display = conn.display();
        let _registry = display.get_registry(&qh, ());

        let mut state = State::default();
        queue.roundtrip(&mut state)?;
        let (Some(compositor), Some(shm), Some(wm_base)) =
            (state.compositor.clone(), state.shm.clone(), state.wm_base.clone())
        else {
            return Err("compositor did not advertise wl_compositor/wl_shm/xdg_wm_base".into());
        };

        // Map the toplevel: initial commit → configure/ack → buffer commit.
        let surface = compositor.create_surface(&qh, ());
        let xdg = wm_base.get_xdg_surface(&surface, &qh, SurfaceRole::Toplevel);
        let toplevel = xdg.get_toplevel(&qh, ());
        toplevel.set_app_id(app_id.into());
        toplevel.set_title("Popup Fixture".into());
        surface.commit();
        dispatch_until(&mut queue, &mut state, "toplevel configure", |s| s.toplevel_configured)?;
        surface.attach(Some(&create_buffer(&shm, &qh, TOPLEVEL_SIZE)?), 0, 0);
        surface.commit();
        queue.roundtrip(&mut state)?;

        // Map the popup the same way, anchored bottom-right of a 1×1 rect.
        let positioner = wm_base.create_positioner(&qh, ());
        positioner.set_size(POPUP_SIZE.0, POPUP_SIZE.1);
        positioner.set_anchor_rect(POPUP_ANCHOR.0, POPUP_ANCHOR.1, 1, 1);
        positioner.set_anchor(xdg_positioner::Anchor::BottomRight);
        positioner.set_gravity(xdg_positioner::Gravity::BottomRight);
        let popup_surface = compositor.create_surface(&qh, ());
        let popup_xdg = wm_base.get_xdg_surface(&popup_surface, &qh, SurfaceRole::Popup);
        let popup = popup_xdg.get_popup(Some(&xdg), &positioner, &qh, ());
        popup_surface.commit();
        dispatch_until(&mut queue, &mut state, "popup configure", |s| s.popup_configured)?;
        popup_surface.attach(Some(&create_buffer(&shm, &qh, POPUP_SIZE)?), 0, 0);
        popup_surface.commit();
        queue.roundtrip(&mut state)?;

        Ok(Fixture { state, queue, popup, popup_xdg_surface: popup_xdg, popup_surface })
    }
}

/// Helper: poll `list_popups` until `count` popups are reported, or time out.
fn wait_for_popups(socket_path: &PathBuf, count: usize, timeout: Duration) -> Option<Value> {
    let start = Instant::now();
    let mut last = None;
    while start.elapsed() < timeout {
        if let Ok(response) = send_command(socket_path, r#"{"command": "list_popups"}"#)
            && let Ok(value) = serde_json::from_str::<Value>(&response)
        {
            if value["popups"].as_array().is_some_and(|popups| popups.len() == count) {
                return Some(value);
            }
            last = Some(value);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    eprintln!("wait_for_popups timed out; last response: {last:?}");
    None
}

#[test]
fn ipc_list_popups_reports_global_popup_rect() {
    let Some((child, socket_name)) = start_compositor("popups_client") else {
        return;
    };
    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    let runtime_dir = PathBuf::from(std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string()));
    let mut fixture = match popup_client::open_toplevel_with_popup(&runtime_dir, &socket_name, "test.popup") {
        Ok(fixture) => fixture,
        Err(err) => {
            eprintln!("skipping: popup client could not connect/map: {err}");
            shutdown_compositor(&socket_path, child);
            return;
        }
    };

    let Some(windows) = wait_for_windows(&socket_path, 1, Duration::from_secs(10)) else {
        eprintln!("skipping: fixture toplevel did not appear in compositor");
        shutdown_compositor(&socket_path, child);
        return;
    };
    let windows: Value = serde_json::from_str(&windows).expect("list_windows should return valid JSON");
    let window = &windows["windows"][0];
    let window_id = window["window_id"].as_u64().expect("missing stable window_id");
    let content_x = window["content_x"].as_i64().expect("missing content_x");
    let content_y = window["content_y"].as_i64().expect("missing content_y");

    let popups = wait_for_popups(&socket_path, 1, Duration::from_secs(10)).expect("popup should be listed");
    let popup = &popups["popups"][0];
    assert_eq!(popup["parent_window_id"].as_u64(), Some(window_id), "wrong parent window: {popup}");
    assert_eq!(popup["pid"].as_u64(), Some(u64::from(std::process::id())), "wrong pid: {popup}");
    // Global rect = parent's on-screen geometry origin + the positioner
    // placement (anchor rect + 1 for bottom-right anchor/gravity).
    assert_eq!(popup["x"].as_i64(), Some(content_x + i64::from(popup_client::POPUP_OFFSET.0)), "wrong x: {popup}");
    assert_eq!(popup["y"].as_i64(), Some(content_y + i64::from(popup_client::POPUP_OFFSET.1)), "wrong y: {popup}");
    assert_eq!(popup["width"].as_i64(), Some(i64::from(popup_client::POPUP_SIZE.0)), "wrong width: {popup}");
    assert_eq!(popup["height"].as_i64(), Some(i64::from(popup_client::POPUP_SIZE.1)), "wrong height: {popup}");

    // A dismissed popup disappears from the listing (the toplevel stays).
    fixture.dismiss_popup().expect("failed to dismiss popup");
    assert!(
        wait_for_popups(&socket_path, 0, Duration::from_secs(10)).is_some(),
        "dismissed popup must no longer be listed"
    );

    drop(fixture);
    shutdown_compositor(&socket_path, child);
}

#[test]
fn ipc_shutdown() {
    let Some((mut child, socket_name)) = start_compositor("shutdown") else {
        return;
    };

    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    let response = send_command(&socket_path, r#"{"command": "shutdown"}"#).expect("failed to send shutdown");
    assert!(response.contains(r#""status":"ok"#), "unexpected response: {response}");
    assert!(response.contains("shutting down"), "unexpected response: {response}");

    // Wait for the compositor to actually exit
    let exit = child.wait().expect("failed to wait for child");
    assert!(exit.success(), "compositor did not exit cleanly: {exit}");
}

// ─── Client helper functions ────────────────────────────────────────────

/// Helper: start the egui test app as a Wayland client in the given compositor.
///
/// Returns the child process. The app auto-closes after the given timeout.
fn start_test_app(socket_name: &str, app_id: &str, title: &str, auto_close: u64) -> Option<Child> {
    // The test app binary lives in the same target directory as the compositor.
    let compositor = PathBuf::from(env!("CARGO_BIN_EXE_platynui-wayland-compositor"));
    let binary = compositor.parent().unwrap().join("platynui-test-app-egui");
    let backend = test_backend();

    let mut cmd = Command::new(&binary);
    cmd.args(["--app-id", app_id, "--title", title, "--auto-close", &auto_close.to_string()])
        .env("WAYLAND_DISPLAY", socket_name);

    if backend == "headless" {
        cmd.env("LIBGL_ALWAYS_SOFTWARE", "1").stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    }

    match cmd.spawn() {
        Ok(child) => Some(child),
        Err(err) => {
            eprintln!("skipping: cannot start test app: {err}");
            None
        }
    }
}

/// Helper: poll `list_windows` until at least `count` windows appear, or timeout.
fn wait_for_windows(socket_path: &PathBuf, count: usize, timeout: Duration) -> Option<String> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Ok(response) = send_command(socket_path, r#"{"command": "list_windows"}"#) {
            // Count window entries by counting `"id":` occurrences
            let window_count = response.matches(r#""id":"#).count();
            if window_count >= count {
                return Some(response);
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    None
}

// ─── Tests with egui client windows ─────────────────────────────────────

#[test]
fn ipc_list_windows_with_client() {
    let Some((child, socket_name)) = start_compositor("list_client") else {
        return;
    };
    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    let Some(mut app) = start_test_app(&socket_name, "test.list", "List Test Window", 20) else {
        shutdown_compositor(&socket_path, child);
        return;
    };

    // Wait for the window to appear
    let Some(response) = wait_for_windows(&socket_path, 1, Duration::from_secs(10)) else {
        eprintln!("skipping: test app window did not appear in compositor");
        let _ = app.kill();
        shutdown_compositor(&socket_path, child);
        return;
    };

    assert!(response.contains(r#""status":"ok"#), "unexpected: {response}");
    assert!(response.contains(r#""app_id":"test.list""#), "missing app_id: {response}");
    assert!(response.contains("List Test Window"), "missing title: {response}");

    let _ = app.kill();
    shutdown_compositor(&socket_path, child);
}

#[test]
fn ipc_get_window_by_app_id() {
    let Some((child, socket_name)) = start_compositor("get_appid") else {
        return;
    };
    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    let Some(mut app) = start_test_app(&socket_name, "test.getbyid", "Get By AppId", 20) else {
        shutdown_compositor(&socket_path, child);
        return;
    };

    if wait_for_windows(&socket_path, 1, Duration::from_secs(10)).is_none() {
        eprintln!("skipping: test app window did not appear");
        let _ = app.kill();
        shutdown_compositor(&socket_path, child);
        return;
    }

    // Look up by app_id
    let response = send_command(&socket_path, r#"{"command": "get_window", "app_id": "test.getbyid"}"#)
        .expect("failed to send get_window");

    assert!(response.contains(r#""status":"ok"#), "unexpected: {response}");
    assert!(response.contains(r#""app_id":"test.getbyid""#), "missing app_id: {response}");
    assert!(response.contains("Get By AppId"), "missing title: {response}");

    let _ = app.kill();
    shutdown_compositor(&socket_path, child);
}

#[test]
fn ipc_get_window_by_title() {
    let Some((child, socket_name)) = start_compositor("get_title") else {
        return;
    };
    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    let Some(mut app) = start_test_app(&socket_name, "test.getbytitle", "Unique Title 42", 20) else {
        shutdown_compositor(&socket_path, child);
        return;
    };

    if wait_for_windows(&socket_path, 1, Duration::from_secs(10)).is_none() {
        eprintln!("skipping: test app window did not appear");
        let _ = app.kill();
        shutdown_compositor(&socket_path, child);
        return;
    }

    // Look up by title (case-insensitive substring match)
    let response = send_command(&socket_path, r#"{"command": "get_window", "title": "unique title"}"#)
        .expect("failed to send get_window");

    assert!(response.contains(r#""status":"ok"#), "unexpected: {response}");
    assert!(response.contains("Unique Title 42"), "missing title: {response}");

    let _ = app.kill();
    shutdown_compositor(&socket_path, child);
}

#[test]
fn ipc_get_window_by_stable_window_id() {
    let Some((child, socket_name)) = start_compositor("get_by_window_id") else {
        return;
    };

    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    let Some(mut test_app) = start_test_app(&socket_name, "test.windowid", "Stable Window Id", 20) else {
        shutdown_compositor(&socket_path, child);
        return;
    };
    if wait_for_windows(&socket_path, 1, Duration::from_secs(10)).is_none() {
        eprintln!("skipping: test app window did not appear");
        let _ = test_app.kill();
        shutdown_compositor(&socket_path, child);
        return;
    }

    let list = send_command(&socket_path, r#"{"command": "list_windows"}"#).expect("failed to list windows");
    let list_json: Value = serde_json::from_str(&list).expect("list_windows should return valid JSON");
    let window_id = list_json["windows"][0]["window_id"].as_u64().expect("missing stable window_id");

    let response = send_command(&socket_path, &format!(r#"{{"command":"get_window","window_id":{window_id}}}"#))
        .expect("failed to send get_window by stable id");
    let response_json: Value = serde_json::from_str(&response).expect("get_window should return valid JSON");

    assert!(response.contains(r#""status":"ok"#), "unexpected response: {response}");
    assert_eq!(response_json["window"]["window_id"].as_u64(), Some(window_id), "missing stable window_id: {response}");
    assert_eq!(response_json["window"]["app_id"].as_str(), Some("test.windowid"), "missing app_id: {response}");

    let _ = test_app.kill();
    let _ = test_app.wait();
    shutdown_compositor(&socket_path, child);
}

#[test]
fn ipc_move_window_by_stable_window_id() {
    let Some((child, socket_name)) = start_compositor("move_by_window_id") else {
        return;
    };
    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    let Some(mut app) = start_test_app(&socket_name, "test.move", "Move Window", 20) else {
        shutdown_compositor(&socket_path, child);
        return;
    };
    if wait_for_windows(&socket_path, 1, Duration::from_secs(10)).is_none() {
        eprintln!("skipping: test app window did not appear");
        let _ = app.kill();
        shutdown_compositor(&socket_path, child);
        return;
    }

    let list = send_command(&socket_path, r#"{"command": "list_windows"}"#).expect("failed to list windows");
    let list_json: Value = serde_json::from_str(&list).expect("list_windows should return valid JSON");
    let window_id = list_json["windows"][0]["window_id"].as_u64().expect("missing stable window_id");

    let response =
        send_command(&socket_path, &format!(r#"{{"command":"move_window","window_id":{window_id},"x":120,"y":80}}"#))
            .expect("failed to send move_window");
    assert!(response.contains(r#""status":"ok"#), "unexpected response: {response}");

    let moved = send_command(&socket_path, &format!(r#"{{"command":"get_window","window_id":{window_id}}}"#))
        .expect("failed to fetch moved window");
    let moved_json: Value = serde_json::from_str(&moved).expect("get_window should return valid JSON");
    assert_eq!(moved_json["window"]["content_x"].as_i64(), Some(120));
    assert_eq!(moved_json["window"]["content_y"].as_i64(), Some(80));

    let _ = app.kill();
    let _ = app.wait();
    shutdown_compositor(&socket_path, child);
}

#[test]
fn ipc_resize_window_by_stable_window_id() {
    let Some((child, socket_name)) = start_compositor("resize_by_window_id") else {
        return;
    };
    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    let Some(mut app) = start_test_app(&socket_name, "test.resize", "Resize Window", 20) else {
        shutdown_compositor(&socket_path, child);
        return;
    };
    if wait_for_windows(&socket_path, 1, Duration::from_secs(10)).is_none() {
        eprintln!("skipping: test app window did not appear");
        let _ = app.kill();
        shutdown_compositor(&socket_path, child);
        return;
    }

    let list = send_command(&socket_path, r#"{"command": "list_windows"}"#).expect("failed to list windows");
    let list_json: Value = serde_json::from_str(&list).expect("list_windows should return valid JSON");
    let window_id = list_json["windows"][0]["window_id"].as_u64().expect("missing stable window_id");

    let response = send_command(
        &socket_path,
        &format!(r#"{{"command":"resize_window","window_id":{window_id},"width":640,"height":360}}"#),
    )
    .expect("failed to send resize_window");
    assert!(response.contains(r#""status":"ok"#), "unexpected response: {response}");

    let _ = app.kill();
    let _ = app.wait();
    shutdown_compositor(&socket_path, child);
}

#[test]
fn ipc_focus_window_by_app_id() {
    let Some((child, socket_name)) = start_compositor("focus_appid") else {
        return;
    };
    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    let Some(mut app) = start_test_app(&socket_name, "test.focus", "Focus Test", 20) else {
        shutdown_compositor(&socket_path, child);
        return;
    };

    if wait_for_windows(&socket_path, 1, Duration::from_secs(10)).is_none() {
        eprintln!("skipping: test app window did not appear");
        let _ = app.kill();
        shutdown_compositor(&socket_path, child);
        return;
    }

    let response = send_command(&socket_path, r#"{"command": "focus_window", "app_id": "test.focus"}"#)
        .expect("failed to send focus_window");

    assert!(response.contains(r#""status":"ok"#), "unexpected: {response}");
    assert!(response.contains("test.focus"), "missing app_id in response: {response}");

    let _ = app.kill();
    shutdown_compositor(&socket_path, child);
}

#[test]
fn ipc_close_window_by_app_id() {
    let Some((child, socket_name)) = start_compositor("close_appid") else {
        return;
    };
    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    let Some(mut app) = start_test_app(&socket_name, "test.close", "Close Test", 20) else {
        shutdown_compositor(&socket_path, child);
        return;
    };

    if wait_for_windows(&socket_path, 1, Duration::from_secs(10)).is_none() {
        eprintln!("skipping: test app window did not appear");
        let _ = app.kill();
        shutdown_compositor(&socket_path, child);
        return;
    }

    let response = send_command(&socket_path, r#"{"command": "close_window", "app_id": "test.close"}"#)
        .expect("failed to send close_window");

    assert!(response.contains(r#""status":"ok"#), "unexpected: {response}");
    assert!(response.contains("test.close"), "missing app_id in response: {response}");

    // Poll until the window is gone. Closing is a round-trip: the client must
    // receive the close event, destroy its surface, and exit before the compositor
    // drops the window — that can take well over a fixed delay on a slow,
    // software-rendered CI runner, so wait with a timeout instead of a flat sleep.
    let mut list = String::new();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        list = send_command(&socket_path, r#"{"command": "list_windows"}"#).expect("failed to list windows");
        if list.contains(r#""windows":[]"#) {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    assert!(list.contains(r#""windows":[]"#), "window should be gone: {list}");

    let _ = app.wait();
    shutdown_compositor(&socket_path, child);
}

#[test]
fn ipc_screenshot_with_client() {
    let Some((child, socket_name)) = start_compositor("screenshot_client") else {
        return;
    };
    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    let Some(mut app) = start_test_app(&socket_name, "test.screenshot", "Screenshot Test", 20) else {
        shutdown_compositor(&socket_path, child);
        return;
    };

    if wait_for_windows(&socket_path, 1, Duration::from_secs(10)).is_none() {
        eprintln!("skipping: test app window did not appear");
        let _ = app.kill();
        shutdown_compositor(&socket_path, child);
        return;
    }

    // Give compositor a frame to render the client window
    std::thread::sleep(Duration::from_millis(500));

    let response = send_command(&socket_path, r#"{"command": "screenshot"}"#).expect("failed to send screenshot");

    if response.contains(r#""status":"ok"#) {
        assert!(response.contains(r#""format":"png"#), "missing format: {}", &response[..response.len().min(200)]);
        // With a client window, the screenshot data should be non-trivial
        assert!(response.contains(r#""data":""#), "missing data field in screenshot response");
    } else {
        eprintln!("screenshot with client failed (expected in environments without GPU)");
    }

    let _ = app.kill();
    shutdown_compositor(&socket_path, child);
}

/// Helper: send a fire-and-forget injection command (no response is written
/// for `key_event` / `pointer_*`), so unlike [`send_command`] nothing is read.
fn send_injection(socket_path: &PathBuf, command: &str) -> Result<(), Box<dyn std::error::Error>> {
    let stream = UnixStream::connect(socket_path)?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut writer = &stream;
    writeln!(writer, "{command}")?;
    writer.flush()?;
    Ok(())
}

/// Helper: poll `get_modifiers` until the reported state matches, or time out.
/// Injection is fire-and-forget, so the observable state trails the command.
fn wait_for_modifiers(socket_path: &PathBuf, expected: (bool, bool, bool, bool), timeout: Duration) -> Option<Value> {
    let start = Instant::now();
    let mut last = None;
    while start.elapsed() < timeout {
        if let Ok(response) = send_command(socket_path, r#"{"command": "get_modifiers"}"#)
            && let Ok(value) = serde_json::from_str::<Value>(&response)
        {
            let state = (
                value["ctrl"].as_bool().unwrap_or(false),
                value["alt"].as_bool().unwrap_or(false),
                value["shift"].as_bool().unwrap_or(false),
                value["logo"].as_bool().unwrap_or(false),
            );
            if state == expected {
                return Some(value);
            }
            last = Some(value);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    eprintln!("wait_for_modifiers timed out; last state: {last:?}");
    None
}

#[test]
fn ipc_list_popups_empty() {
    let Some((child, socket_name)) = start_compositor("popups_empty") else {
        return;
    };

    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    let response = send_command(&socket_path, r#"{"command": "list_popups"}"#).expect("failed to send list_popups");
    assert!(response.contains(r#""status":"ok"#), "unexpected response: {response}");
    assert!(response.contains(r#""popups":[]"#), "expected empty popups list: {response}");

    shutdown_compositor(&socket_path, child);
}

#[test]
fn ipc_get_modifiers_reflects_injected_keys() {
    let Some((child, socket_name)) = start_compositor("get_modifiers") else {
        return;
    };

    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    // Idle seat: no modifier is active.
    let idle = wait_for_modifiers(&socket_path, (false, false, false, false), Duration::from_secs(5))
        .expect("modifiers should start released");
    assert_eq!(idle["status"].as_str(), Some("ok"));

    // Hold Ctrl+Alt+Shift (evdev: KEY_LEFTCTRL=29, KEY_LEFTSHIFT=42, KEY_LEFTALT=56).
    for key in [29u32, 56, 42] {
        send_injection(&socket_path, &format!(r#"{{"command":"key_event","key":{key},"state":"press"}}"#))
            .expect("failed to inject key press");
    }
    assert!(
        wait_for_modifiers(&socket_path, (true, true, true, false), Duration::from_secs(5)).is_some(),
        "held Ctrl+Alt+Shift must be observable via get_modifiers"
    );

    // Release them again.
    for key in [29u32, 56, 42] {
        send_injection(&socket_path, &format!(r#"{{"command":"key_event","key":{key},"state":"release"}}"#))
            .expect("failed to inject key release");
    }
    assert!(
        wait_for_modifiers(&socket_path, (false, false, false, false), Duration::from_secs(5)).is_some(),
        "released modifiers must be reported as inactive"
    );

    shutdown_compositor(&socket_path, child);
}

#[test]
fn ipc_get_pointer_position() {
    let Some((child, socket_name)) = start_compositor("pointer_pos") else {
        return;
    };

    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    let response = send_command(&socket_path, r#"{"command": "get_pointer_position"}"#)
        .expect("failed to send get_pointer_position");
    assert!(response.contains(r#""status":"ok"#), "unexpected response: {response}");
    // Response must contain numeric x and y fields.
    let value: serde_json::Value = serde_json::from_str(&response).expect("invalid JSON");
    assert!(value.get("x").and_then(serde_json::Value::as_f64).is_some(), "missing x: {response}");
    assert!(value.get("y").and_then(serde_json::Value::as_f64).is_some(), "missing y: {response}");

    shutdown_compositor(&socket_path, child);
}
