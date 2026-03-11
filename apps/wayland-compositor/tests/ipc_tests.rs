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
// Suppress unused-crate-dependency warnings — integration test inherits library deps.
#![allow(unused_crate_dependencies)]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

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
    assert!(
        response.contains(r#""status":"ok"#) || response.contains(r#""status":"error"#),
        "unexpected response: {response}"
    );

    if response.contains(r#""status":"ok"#) {
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

    // Give the app time to receive the close request and exit
    std::thread::sleep(Duration::from_millis(500));

    // Verify the window is gone
    let list = send_command(&socket_path, r#"{"command": "list_windows"}"#).expect("failed to list windows");
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
