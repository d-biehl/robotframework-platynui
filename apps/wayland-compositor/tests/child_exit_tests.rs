#![allow(unused_crate_dependencies)]

//! `--exit-with-child` integration tests — the compositor reports its child's result as its exit code.
//!
//! A CI session runs the test command as the compositor's child, so the compositor's own
//! exit code is the only result the caller sees: a failing child must fail the compositor,
//! and a child whose result is unknown must never read as success. These tests start
//! `platynui-wayland-compositor` headless with a child and check the compositor's exit status.

// This entire test suite only applies to Linux (Wayland compositor).
#![cfg(target_os = "linux")]

use std::fs::File;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use platynui_wayland_compositor as _;

/// Upper bound for one compositor session; a hang fails the test instead of blocking it.
const SESSION_DEADLINE: Duration = Duration::from_secs(30);

/// Run the compositor headless with `--timeout <timeout_secs> --exit-with-child -- <child>` and
/// return its exit status plus its log output (for the assertion message — a failure that only
/// happens on CI is otherwise undiagnosable).
fn run_compositor_with_child(test_name: &str, timeout_secs: u64, child: &[&str]) -> (ExitStatus, String) {
    let socket_name = format!("platynui-test-child-exit-{test_name}-{}", std::process::id());
    let log_path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("child-exit-{test_name}.log"));
    let log = File::create(&log_path).expect("failed to create the compositor log file");

    let mut compositor = Command::new(env!("CARGO_BIN_EXE_platynui-wayland-compositor"))
        .args(["--backend", "headless", "--socket-name", &socket_name, "--no-control-socket", "--no-eis"])
        .args(["--log-level", "info", "--timeout", &timeout_secs.to_string(), "--exit-with-child", "--"])
        .args(child)
        .env("LIBGL_ALWAYS_SOFTWARE", "1")
        .stdout(Stdio::from(log.try_clone().expect("failed to clone the log file handle")))
        .stderr(Stdio::from(log))
        .spawn()
        .expect("failed to start the compositor");

    let start = Instant::now();
    let status = loop {
        if let Some(status) = compositor.try_wait().expect("failed to wait for the compositor") {
            break status;
        }
        if start.elapsed() > SESSION_DEADLINE {
            let _ = compositor.kill();
            let _ = compositor.wait();
            let output = std::fs::read_to_string(&log_path).unwrap_or_default();
            panic!("compositor did not exit within {SESSION_DEADLINE:?}\ncompositor output:\n{output}");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    (status, std::fs::read_to_string(&log_path).unwrap_or_default())
}

#[test]
fn a_successful_child_exits_the_compositor_successfully() {
    let (status, output) = run_compositor_with_child("success", 60, &["sh", "-c", "exit 0"]);
    assert_eq!(status.code(), Some(0), "compositor exit status: {status}\ncompositor output:\n{output}");
}

#[test]
fn a_failing_child_exits_the_compositor_with_its_exit_code() {
    let (status, output) = run_compositor_with_child("failure", 60, &["sh", "-c", "exit 3"]);
    assert_eq!(status.code(), Some(3), "compositor exit status: {status}\ncompositor output:\n{output}");
}

#[test]
fn a_child_killed_by_a_signal_exits_the_compositor_with_128_plus_the_signal() {
    let (status, output) = run_compositor_with_child("signal", 60, &["sh", "-c", "kill -TERM $$"]);
    assert_eq!(status.code(), Some(128 + 15), "compositor exit status: {status}\ncompositor output:\n{output}");
}

#[test]
fn a_child_that_cannot_be_started_exits_the_compositor_with_127() {
    let (status, output) = run_compositor_with_child("not-found", 60, &["/nonexistent/platynui-test-child"]);
    assert_eq!(status.code(), Some(127), "compositor exit status: {status}\ncompositor output:\n{output}");
}

#[test]
fn a_session_ended_by_the_timeout_before_the_child_exits_fails() {
    let (status, output) = run_compositor_with_child("timeout", 1, &["sleep", "10"]);
    assert_eq!(status.code(), Some(1), "compositor exit status: {status}\ncompositor output:\n{output}");
}
