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
// Shared with `pidns_tests.rs`; see the module's own documentation.

#[path = "shared/popup_client.rs"]
mod popup_client;

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

// ─── Minimized and maximized window state ───────────────────────────────

/// Helper: send `command` until the JSON response satisfies `predicate`, or timeout.
/// Window-state changes land asynchronously (configure/ack/commit round-trip with the
/// client), so a single read may still see the previous state.
fn wait_for_response(
    socket_path: &PathBuf,
    command: &str,
    timeout: Duration,
    predicate: impl Fn(&Value) -> bool,
) -> Option<Value> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Ok(response) = send_command(socket_path, command)
            && let Ok(value) = serde_json::from_str::<Value>(&response)
            && predicate(&value)
        {
            return Some(value);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    None
}

/// Helper: start a compositor with one egui client and return its stable `window_id`
/// after maximizing and then minimizing it. `None` when the environment cannot run
/// the scenario (the caller then skips, like the other client tests).
fn start_with_window_minimized_from_maximized(test_name: &str, app_id: &str) -> Option<(Child, PathBuf, Child, u64)> {
    let (child, socket_name) = start_compositor(test_name)?;
    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return None;
    };
    let Some(mut app) = start_test_app(&socket_name, app_id, "Window State", 30) else {
        shutdown_compositor(&socket_path, child);
        return None;
    };
    let Some(list) = wait_for_windows(&socket_path, 1, Duration::from_secs(10)) else {
        eprintln!("skipping: test app window did not appear");
        let _ = app.kill();
        shutdown_compositor(&socket_path, child);
        return None;
    };
    let list_json: Value = serde_json::from_str(&list).expect("list_windows should return valid JSON");
    let window_id = list_json["windows"][0]["window_id"].as_u64().expect("missing stable window_id");

    let get_window = format!(r#"{{"command":"get_window","window_id":{window_id}}}"#);
    let maximize = send_command(&socket_path, &format!(r#"{{"command":"maximize_window","window_id":{window_id}}}"#))
        .expect("failed to send maximize_window");
    assert!(maximize.contains(r#""status":"ok"#), "maximize failed: {maximize}");
    let maximized = wait_for_response(&socket_path, &get_window, Duration::from_secs(10), |value| {
        value["window"]["maximized"].as_bool() == Some(true)
    });
    assert!(maximized.is_some(), "window never reported maximized");

    let minimize = send_command(&socket_path, &format!(r#"{{"command":"minimize_window","window_id":{window_id}}}"#))
        .expect("failed to send minimize_window");
    assert!(minimize.contains(r#""status":"ok"#), "minimize failed: {minimize}");

    Some((child, socket_path, app, window_id))
}

#[test]
fn ipc_minimized_window_reports_size_and_state() {
    let Some((child, socket_path, mut app, window_id)) =
        start_with_window_minimized_from_maximized("minimized_state", "test.minimized.state")
    else {
        return;
    };

    let list = wait_for_response(&socket_path, r#"{"command": "list_windows"}"#, Duration::from_secs(10), |value| {
        value["minimized"].as_array().is_some_and(|entries| !entries.is_empty())
    })
    .expect("window never appeared in the minimized list");
    let entry = &list["minimized"][0];
    assert_eq!(entry["window_id"].as_u64(), Some(window_id), "unexpected minimized entry: {list}");
    assert_eq!(entry["minimized"].as_bool(), Some(true), "minimized flag missing: {list}");
    assert_eq!(entry["maximized"].as_bool(), Some(true), "maximized state lost: {list}");
    assert!(entry["content_width"].as_i64().is_some_and(|w| w > 0), "missing content width: {list}");
    assert!(entry["content_height"].as_i64().is_some_and(|h| h > 0), "missing content height: {list}");

    let response = send_command(&socket_path, &format!(r#"{{"command":"get_window","window_id":{window_id}}}"#))
        .expect("failed to send get_window");
    let response_json: Value = serde_json::from_str(&response).expect("get_window should return valid JSON");
    assert!(response.contains(r#""status":"ok"#), "minimized window not found by window_id: {response}");
    assert_eq!(response_json["window"]["window_id"].as_u64(), Some(window_id), "unexpected window: {response}");
    assert_eq!(response_json["window"]["minimized"].as_bool(), Some(true), "not reported minimized: {response}");

    let _ = app.kill();
    let _ = app.wait();
    shutdown_compositor(&socket_path, child);
}

#[test]
fn ipc_focus_window_brings_back_minimized_window_maximized() {
    let Some((child, socket_path, mut app, window_id)) =
        start_with_window_minimized_from_maximized("focus_minimized", "test.focus.minimized")
    else {
        return;
    };
    let get_window = format!(r#"{{"command":"get_window","window_id":{window_id}}}"#);
    let minimized = wait_for_response(&socket_path, &get_window, Duration::from_secs(10), |value| {
        value["window"]["minimized"].as_bool() == Some(true)
    });
    assert!(minimized.is_some(), "window never reported minimized");

    let response = send_command(&socket_path, &format!(r#"{{"command":"focus_window","window_id":{window_id}}}"#))
        .expect("failed to send focus_window");
    assert!(response.contains(r#""status":"ok"#), "focus_window did not find the minimized window: {response}");

    let restored = wait_for_response(&socket_path, &get_window, Duration::from_secs(10), |value| {
        value["window"]["minimized"].as_bool() == Some(false) && value["window"]["focused"].as_bool() == Some(true)
    })
    .expect("focused window never came back from minimized");
    assert_eq!(restored["window"]["maximized"].as_bool(), Some(true), "window lost its maximized state: {restored}");

    let _ = app.kill();
    let _ = app.wait();
    shutdown_compositor(&socket_path, child);
}

// ─── window_at_point and the caller's own windows ────────────────────────
//
// No PID namespace is needed here: the default lane already *is* the topology
// the exclusion is about — this test process owns a Wayland window and is the
// control-socket caller, so the compositor resolves both sides to one process.

/// `app_id` of the window this test process owns.
const AT_POINT_OWN_APP_ID: &str = "test.atpoint.own";
/// `app_id` of the window owned by a separate process.
const AT_POINT_OTHER_APP_ID: &str = "test.atpoint.other";

/// Environment variables through which the parent hands the child its request.
const CHILD_SOCKET_ENV: &str = "PLATYNUI_TEST_AT_POINT_SOCKET";
const CHILD_POINT_ENV: &str = "PLATYNUI_TEST_AT_POINT_POINT";
const CHILD_OUTPUT_ENV: &str = "PLATYNUI_TEST_AT_POINT_OUTPUT";

/// Content rectangle (`x`, `y`, `width`, `height`) of a window entry.
fn content_rect(window: &Value) -> (i64, i64, i64, i64) {
    let field = |name: &str| window[name].as_i64().unwrap_or_else(|| panic!("missing {name}: {window}"));
    (field("content_x"), field("content_y"), field("content_width"), field("content_height"))
}

/// Look up a window entry by `app_id` in a fresh `list_windows` response.
fn window_by_app_id(socket_path: &PathBuf, app_id: &str) -> Option<Value> {
    let response = send_command(socket_path, r#"{"command": "list_windows"}"#).ok()?;
    let value: Value = serde_json::from_str(&response).ok()?;
    value["windows"].as_array()?.iter().find(|window| window["app_id"].as_str() == Some(app_id)).cloned()
}

/// Send a command that must succeed, and return the parsed response.
fn command_ok(socket_path: &PathBuf, request: &str) -> Value {
    let response =
        send_command(socket_path, request).unwrap_or_else(|err| panic!("`{request}` could not be sent: {err}"));
    let value: Value = serde_json::from_str(&response).unwrap_or_else(|err| panic!("`{request}` → {response}: {err}"));
    assert_eq!(value["status"].as_str(), Some("ok"), "`{request}` failed: {response}");
    value
}

fn move_window_to(socket_path: &PathBuf, window: &Value, x: i64, y: i64) {
    let window_id = window["window_id"].as_u64().expect("missing window_id");
    command_ok(socket_path, &format!(r#"{{"command":"move_window","window_id":{window_id},"x":{x},"y":{y}}}"#));
}

fn resize_window_to(socket_path: &PathBuf, window: &Value, width: i64, height: i64) {
    let window_id = window["window_id"].as_u64().expect("missing window_id");
    command_ok(
        socket_path,
        &format!(r#"{{"command":"resize_window","window_id":{window_id},"width":{width},"height":{height}}}"#),
    );
}

/// Raise a window to the front (and focus it), like an activation request.
fn raise_window(socket_path: &PathBuf, window: &Value) {
    let window_id = window["window_id"].as_u64().expect("missing window_id");
    command_ok(socket_path, &format!(r#"{{"command":"focus_window","window_id":{window_id}}}"#));
}

fn at_point(socket_path: &PathBuf, (x, y): (i64, i64)) -> Value {
    command_ok(socket_path, &format!(r#"{{"command":"window_at_point","x":{x},"y":{y}}}"#))
}

/// Ask `window_at_point` from a *different* process, by re-executing this test
/// binary as a child that runs the helper test below. All that matters is that
/// the request arrives on a connection whose peer is not this process.
fn at_point_from_another_process(socket_path: &PathBuf, (x, y): (i64, i64)) -> Value {
    let output = tempfile::NamedTempFile::new().expect("cannot create the child's response file");
    let binary = std::env::current_exe().expect("the test binary must have a path");
    let status = Command::new(binary)
        .args(["--exact", "window_at_point_from_another_process", "--include-ignored", "--nocapture"])
        .env(CHILD_SOCKET_ENV, socket_path)
        .env(CHILD_POINT_ENV, format!("{x},{y}"))
        .env(CHILD_OUTPUT_ENV, output.path())
        .status()
        .expect("cannot re-execute the test binary as a child");
    assert!(status.success(), "the child process failed: {status}");

    let response = std::fs::read_to_string(output.path()).expect("the child wrote no response");
    serde_json::from_str(&response).unwrap_or_else(|err| panic!("the child's response is not JSON: {response}: {err}"))
}

/// Helper process for [`ipc_window_at_point_skips_the_callers_own_window`]:
/// asks `window_at_point` over a window this process does not own.
///
/// `#[ignore]`d, because it is not a test of its own — the parent re-executes
/// this binary with `--include-ignored --exact`. A run that forces ignored tests
/// finds no request in the environment and returns.
#[test]
#[ignore = "helper process, driven by ipc_window_at_point_skips_the_callers_own_window"]
fn window_at_point_from_another_process() {
    let (Ok(socket), Ok(point), Ok(output)) =
        (std::env::var(CHILD_SOCKET_ENV), std::env::var(CHILD_POINT_ENV), std::env::var(CHILD_OUTPUT_ENV))
    else {
        eprintln!("helper process: no request in the environment — it is driven by its parent test");
        return;
    };

    let (x, y) = point.split_once(',').expect("the point must be given as `x,y`");
    let response = send_command(&PathBuf::from(socket), &format!(r#"{{"command":"window_at_point","x":{x},"y":{y}}}"#))
        .expect("the helper could not query window_at_point");
    std::fs::write(output, response).expect("the helper could not write its response");
}

#[test]
fn ipc_window_at_point_skips_the_callers_own_window() {
    let Some((child, socket_name)) = start_compositor("at_point_own") else {
        return;
    };
    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    // This process's own window, mapped from this process's own connection.
    let runtime_dir = PathBuf::from(std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string()));
    let fixture = match popup_client::open_toplevel_with_popup(&runtime_dir, &socket_name, AT_POINT_OWN_APP_ID) {
        Ok(fixture) => fixture,
        Err(err) => {
            eprintln!("skipping: popup client could not connect/map: {err}");
            shutdown_compositor(&socket_path, child);
            return;
        }
    };

    // A window of another process, over the same area.
    let Some(mut app) = start_test_app(&socket_name, AT_POINT_OTHER_APP_ID, "At Point Window", 30) else {
        drop(fixture);
        shutdown_compositor(&socket_path, child);
        return;
    };
    if wait_for_windows(&socket_path, 2, Duration::from_secs(15)).is_none() {
        eprintln!("skipping: the two windows did not both appear in the compositor");
        let _ = app.kill();
        drop(fixture);
        shutdown_compositor(&socket_path, child);
        return;
    }

    // Lay the two windows out so they overlap and each keeps a private area,
    // with this process's own window in front of the other one.
    let own = window_by_app_id(&socket_path, AT_POINT_OWN_APP_ID).expect("own window must be listed");
    let other = window_by_app_id(&socket_path, AT_POINT_OTHER_APP_ID).expect("the other window must be listed");
    resize_window_to(&socket_path, &other, 400, 300);
    move_window_to(&socket_path, &other, 300, 200);
    move_window_to(&socket_path, &own, 100, 100);
    raise_window(&socket_path, &own);
    std::thread::sleep(Duration::from_millis(300));

    let own = window_by_app_id(&socket_path, AT_POINT_OWN_APP_ID).expect("own window must still be listed");
    let other = window_by_app_id(&socket_path, AT_POINT_OTHER_APP_ID).expect("the other window must still be listed");
    let (ox, oy, ow, oh) = content_rect(&own);
    let (tx, ty, tw, th) = content_rect(&other);
    assert!(
        ox < tx && oy < ty && tx < ox + ow && ty < oy + oh && ox + ow < tx + tw && oy + oh < ty + th,
        "the layout must overlap and leave each window a private area: own={own} other={other}"
    );
    let overlap = ((tx + ox + ow) / 2, (ty + oy + oh) / 2);
    let own_only = (i64::midpoint(ox, tx), i64::midpoint(oy, ty));
    let other_only = ((ox + ow + tx + tw) / 2, (oy + oh + ty + th) / 2);

    // Over the overlap our own window is frontmost, so the answer is the window
    // behind it — never the window of the process that asked.
    let overlap_hit = at_point(&socket_path, overlap);
    assert_eq!(
        overlap_hit["window"]["app_id"].as_str(),
        Some(AT_POINT_OTHER_APP_ID),
        "the caller's own window must be skipped for the one behind it: {overlap_hit}"
    );

    // The reported `id` is still the window's index in the listing: the stack
    // walk derives that index itself, where the old single-hit lookup searched
    // for the window it had been handed.
    let listing = command_ok(&socket_path, r#"{"command": "list_windows"}"#);
    let expected_index = listing["windows"]
        .as_array()
        .expect("list_windows must carry a windows array")
        .iter()
        .position(|window| window["app_id"].as_str() == Some(AT_POINT_OTHER_APP_ID))
        .expect("the other process's window must be listed");
    assert_eq!(
        overlap_hit["window"]["id"].as_u64(),
        Some(u64::try_from(expected_index).expect("an index fits u64")),
        "window_at_point must report the window's index in the listing: {overlap_hit}"
    );

    // Where only our own window is, there is nothing left to report.
    let response = at_point(&socket_path, own_only);
    assert!(
        response["window"].is_null(),
        "only the caller's own window is at {own_only:?}, so nothing may be reported: {response}"
    );

    // Nothing is excluded for the wrong reason.
    let response = at_point(&socket_path, other_only);
    assert_eq!(
        response["window"]["app_id"].as_str(),
        Some(AT_POINT_OTHER_APP_ID),
        "a point over the other process's window alone must report it: {response}"
    );

    // The caller is the reference, not the owner of the frontmost window: the
    // same point answers differently for a process that owns nothing there.
    let response = at_point_from_another_process(&socket_path, overlap);
    assert_eq!(
        response["window"]["app_id"].as_str(),
        Some(AT_POINT_OWN_APP_ID),
        "another process must be given the frontmost window at {overlap:?}: {response}"
    );

    // The exclusion is the point lookup's alone — the listings keep reporting
    // the caller's own window, with the process id the compositor established.
    let listed = window_by_app_id(&socket_path, AT_POINT_OWN_APP_ID).expect("own window must still be listed");
    assert_eq!(listed["pid"].as_u64(), Some(u64::from(std::process::id())), "own window's pid: {listed}");
    let by_app_id =
        command_ok(&socket_path, &format!(r#"{{"command":"get_window","app_id":"{AT_POINT_OWN_APP_ID}"}}"#));
    assert_eq!(by_app_id["window"]["app_id"].as_str(), Some(AT_POINT_OWN_APP_ID), "get_window: {by_app_id}");
    let popups = command_ok(&socket_path, r#"{"command": "list_popups"}"#);
    assert_eq!(
        popups["popups"][0]["pid"].as_u64(),
        Some(u64::from(std::process::id())),
        "the caller's own popup must stay listed: {popups}"
    );

    drop(fixture);
    let _ = app.kill();
    let _ = app.wait();
    shutdown_compositor(&socket_path, child);
}

/// The two error answers the `window_at_point` documentation promises: a missing
/// coordinate is the command's own error, a coordinate that is not a number
/// fails request parsing like any other malformed request.
#[test]
fn ipc_window_at_point_rejects_bad_coordinates() {
    let Some((child, socket_name)) = start_compositor("at_point_errors") else {
        return;
    };
    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        return;
    };

    let missing = send_command(&socket_path, r#"{"command":"window_at_point","x":10.0}"#)
        .expect("failed to send window_at_point without y");
    assert!(missing.contains("window_at_point requires x and y"), "unexpected response: {missing}");

    let not_a_number = send_command(&socket_path, r#"{"command":"window_at_point","x":"abc","y":10.0}"#)
        .expect("failed to send window_at_point with a non-numeric x");
    assert!(not_a_number.contains("invalid JSON"), "unexpected response: {not_a_number}");

    shutdown_compositor(&socket_path, child);
}

/// The compositor states once per client, at a diagnostic level, which process it
/// established for that client — and never again while serving requests, which is
/// what keeps the log readable when every window operation lists windows.
///
/// The compositor logs into a file rather than a pipe: at `--log-level debug` a
/// pipe could fill and block it before the test reads anything.
#[test]
fn ipc_client_process_is_logged_once_at_debug() {
    let log = tempfile::NamedTempFile::new().expect("cannot create a log file");
    let socket_name = format!("platynui-test-clientlog-{}", std::process::id());
    let binary = env!("CARGO_BIN_EXE_platynui-wayland-compositor");
    let Ok(child) = Command::new(binary)
        .args(["--backend", test_backend(), "--socket-name", &socket_name, "--timeout", "30"])
        .args(["--log-level", "debug"])
        .env("LIBGL_ALWAYS_SOFTWARE", "1")
        .stdout(std::process::Stdio::from(log.reopen().expect("log file must be reopenable")))
        .stderr(std::process::Stdio::from(log.reopen().expect("log file must be reopenable")))
        .spawn()
    else {
        eprintln!("skipping: cannot start compositor");
        return;
    };
    let mut child = child;

    let Some(socket_path) = wait_for_socket(&socket_name, Duration::from_secs(10)) else {
        eprintln!("skipping: control socket did not appear");
        let _ = child.kill();
        let _ = child.wait();
        return;
    };

    let runtime_dir = PathBuf::from(std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string()));
    let fixture = match popup_client::open_toplevel_with_popup(&runtime_dir, &socket_name, "test.clientlog") {
        Ok(fixture) => fixture,
        Err(err) => {
            eprintln!("skipping: popup client could not connect/map: {err}");
            let _ = send_command(&socket_path, r#"{"command": "shutdown"}"#);
            let _ = child.wait();
            return;
        }
    };

    // Ask several times: identity is established when the connection is accepted,
    // so serving requests must not add lines.
    if wait_for_windows(&socket_path, 1, Duration::from_secs(10)).is_none() {
        eprintln!("skipping: fixture toplevel did not appear");
        drop(fixture);
        let _ = send_command(&socket_path, r#"{"command": "shutdown"}"#);
        let _ = child.wait();
        return;
    }
    for _ in 0..3 {
        let _ = send_command(&socket_path, r#"{"command": "list_windows"}"#);
    }

    drop(fixture);
    let _ = send_command(&socket_path, r#"{"command": "shutdown"}"#);
    let _ = child.wait();

    let captured = std::fs::read_to_string(log.path()).expect("the compositor's log must be readable");
    let identified: Vec<&str> =
        captured.lines().filter(|line| line.contains("Wayland client process identified")).collect();
    assert_eq!(identified.len(), 1, "expected exactly one line for the one client\n--- compositor log ---\n{captured}");
    assert!(
        identified[0].contains(&format!("pid={}", std::process::id())),
        "the line must name this process: {}",
        identified[0]
    );
    assert!(identified[0].contains("DEBUG"), "the identified case belongs at a diagnostic level: {}", identified[0]);
    assert!(
        !captured.contains("could not identify the client's process"),
        "a client in the compositor's own namespace must not warn\n--- compositor log ---\n{captured}"
    );
}
