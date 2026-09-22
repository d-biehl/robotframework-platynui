// This test only applies to Linux (Wayland compositor, PID namespaces).
#![cfg(target_os = "linux")]
#![allow(unused_crate_dependencies)]

//! PID-namespace integration test — the compositor serves a client whose
//! process it cannot see.
//!
//! The compositor runs inside its own user+PID namespace while the test process
//! stays outside it and connects as an ordinary Wayland client through the
//! shared `XDG_RUNTIME_DIR`. The kernel then reports process id `0` to the
//! compositor's `SO_PEERCRED` read — the same answer measured for a sidecar
//! deployment, where the runtime lives in a sibling namespace.
//!
//! The test is `#[ignore]`d because unprivileged user namespaces are not
//! available everywhere (`AppArmor` blocks them on stock Ubuntu 24.04). Run it
//! with:
//!
//! ```sh
//! just test-compositor-pidns
//! ```
//!
//! A missing prerequisite **fails** the run rather than skipping the coverage:
//! an ignored test only runs when someone asks for exactly this coverage, so a
//! skip would read as a pass in the one run meant to exercise it. This follows
//! `crates/java-agent/tests/live_fixture.rs`, not the graceful skip of
//! `ipc_tests.rs`.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

#[path = "shared/popup_client.rs"]
mod popup_client;

/// `app_id` of the fixture's toplevel.
const APP_ID: &str = "test.pidns";

/// The compositor's message for a client whose process it could not identify.
const UNIDENTIFIED_CLIENT_LOG: &str = "could not identify the client's process";

/// The `unshare` arguments that put a child in its own user and PID namespace.
///
/// `--map-current-user` keeps the uid, which the Wayland socket's ownership
/// needs; `--mount-proc` gives the namespace a matching `/proc`.
const UNSHARE_ARGS: [&str; 5] = ["--user", "--map-current-user", "--pid", "--fork", "--mount-proc"];

/// Fail unless this machine can put a process in its own PID namespace.
fn require_pid_namespaces() {
    let probe = Command::new("unshare").args(UNSHARE_ARGS).arg("/bin/true").output();
    match probe {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            panic!("prerequisite missing: `unshare` is not installed (util-linux)");
        }
        Err(err) => panic!("prerequisite missing: cannot run `unshare`: {err}"),
        Ok(output) if !output.status.success() => panic!(
            "prerequisite missing: unprivileged user namespaces are unavailable \
             (`unshare {}` exited with {}: {}). Allow them with \
             `sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0`, and check that \
             `user.max_user_namespaces` is not 0.",
            UNSHARE_ARGS.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim(),
        ),
        Ok(_) => {}
    }
}

/// The compositor under test, running in its own PID namespace.
///
/// Dropping this kills the `unshare` parent; `--kill-child` takes the
/// compositor down with it, so a failing test leaves nothing behind.
struct Compositor {
    child: Child,
    log: tempfile::NamedTempFile,
}

impl Compositor {
    /// Start the compositor inside a fresh user+PID namespace.
    fn start(socket_name: &str) -> Self {
        let log = tempfile::NamedTempFile::new()
            .unwrap_or_else(|err| panic!("prerequisite missing: cannot create a log file: {err}"));
        let binary = env!("CARGO_BIN_EXE_platynui-wayland-compositor");
        let stdout = log.reopen().expect("log file must be reopenable");
        let stderr = log.reopen().expect("log file must be reopenable");

        let child = Command::new("unshare")
            .args(UNSHARE_ARGS)
            .arg("--kill-child")
            .arg(binary)
            .args(["--backend", "headless", "--socket-name", socket_name, "--timeout", "30"])
            .args(["--log-level", "debug"])
            .env("LIBGL_ALWAYS_SOFTWARE", "1")
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .unwrap_or_else(|err| panic!("prerequisite missing: cannot start the compositor: {err}"));

        Self { child, log }
    }

    /// Wait for the compositor to exit and report its status.
    fn wait_for_exit(&mut self) -> std::process::ExitStatus {
        self.child.wait().expect("waiting for the compositor must succeed")
    }

    /// Everything the compositor has logged so far.
    fn log(&self) -> String {
        let mut text = String::new();
        if let Ok(mut file) = std::fs::File::open(self.log.path()) {
            let _ = file.read_to_string(&mut text);
        }
        text
    }
}

impl Drop for Compositor {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Wait for the control socket to appear, failing with the compositor's log.
fn wait_for_control_socket(socket_name: &str, compositor: &Compositor) -> PathBuf {
    let runtime_dir = runtime_dir();
    let socket_path = runtime_dir.join(format!("{socket_name}.control"));
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if socket_path.exists() {
            std::thread::sleep(Duration::from_millis(100));
            return socket_path;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!(
        "prerequisite missing: the compositor's control socket never appeared at {}\n\
         --- compositor log ---\n{}",
        socket_path.display(),
        compositor.log()
    );
}

fn runtime_dir() -> PathBuf {
    PathBuf::from(std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string()))
}

/// Send one command and read one response.
fn try_command(socket_path: &Path, request: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let stream = UnixStream::connect(socket_path)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let mut writer = &stream;
    writeln!(writer, "{request}")?;
    writer.flush()?;

    let mut response = String::new();
    BufReader::new(&stream).read_line(&mut response)?;
    if response.trim().is_empty() {
        return Err("the compositor closed the connection without answering".into());
    }
    Ok(serde_json::from_str(&response)?)
}

/// Send one command, failing loudly with the compositor's log when it does not
/// answer — a compositor that died on an unidentifiable client is exactly what
/// this test is here to catch, so a missing answer is never retried.
fn command(socket_path: &Path, request: &str, compositor: &Compositor) -> Value {
    try_command(socket_path, request).unwrap_or_else(|err| {
        panic!("the compositor did not answer `{request}`: {err}\n--- compositor log ---\n{}", compositor.log())
    })
}

#[test]
#[ignore = "needs unprivileged user namespaces — run with `just test-compositor-pidns`"]
fn compositor_serves_a_client_in_a_foreign_pid_namespace() {
    require_pid_namespaces();

    let socket_name = format!("platynui-pidns-{}", std::process::id());
    let mut compositor = Compositor::start(&socket_name);
    let socket_path = wait_for_control_socket(&socket_name, &compositor);

    // The fixture connects from *outside* the compositor's PID namespace, which
    // is what makes its process unidentifiable to the compositor.
    let _fixture = popup_client::open_toplevel_with_popup(&runtime_dir(), &socket_name, APP_ID).unwrap_or_else(|err| {
        panic!(
            "prerequisite missing: the fixture client could not connect or map: {err}\n\
             --- compositor log ---\n{}",
            compositor.log()
        )
    });

    // Poll until the toplevel is mapped. Every poll reports the window, so on a
    // compositor that cannot represent an unidentifiable client this is where it
    // dies — `command` fails the test instead of retrying.
    let deadline = Instant::now() + Duration::from_secs(15);
    let windows = loop {
        let response = command(&socket_path, r#"{"command": "list_windows"}"#, &compositor);
        if response["windows"].as_array().is_some_and(|windows| !windows.is_empty()) {
            break response;
        }
        assert!(
            Instant::now() < deadline,
            "the fixture's toplevel never appeared in the compositor\n--- compositor log ---\n{}",
            compositor.log()
        );
        std::thread::sleep(Duration::from_millis(200));
    };

    let window = &windows["windows"][0];
    assert_eq!(window["app_id"].as_str(), Some(APP_ID), "unexpected window: {window}");

    let by_app_id = command(&socket_path, &format!(r#"{{"command":"get_window","app_id":"{APP_ID}"}}"#), &compositor);
    assert_eq!(by_app_id["status"].as_str(), Some("ok"), "get_window failed: {by_app_id}");
    assert_eq!(by_app_id["window"]["app_id"].as_str(), Some(APP_ID), "wrong window: {by_app_id}");

    // The point lookup excludes a window only on a positive match of two
    // identities the compositor established. Here it can resolve neither the
    // fixture's client nor the caller — both live outside its PID namespace — so
    // it excludes nothing and answers with the fixture's own window. That is the
    // measured limit of the mechanism, not a defect in it: a control connection
    // from a sibling namespace is just as unresolvable.
    let (x, y) = window_centre(window);
    let at_point = command(&socket_path, &format!(r#"{{"command":"window_at_point","x":{x},"y":{y}}}"#), &compositor);
    assert_eq!(at_point["status"].as_str(), Some("ok"), "window_at_point failed: {at_point}");
    assert_eq!(at_point["window"]["app_id"].as_str(), Some(APP_ID), "wrong window at point: {at_point}");

    let popups = wait_for_popup(&socket_path, &compositor);
    assert_eq!(popups["popups"][0]["parent_window_id"], window["window_id"], "wrong popup parent: {popups}");

    // Every reported process id is `null`. `0` is never reported, because a
    // consumer filtering by PID would take it for a real process. A real PID
    // would mean the namespace topology never took effect — asserted, not
    // skipped, so the test cannot pass vacuously.
    for (label, response) in [
        ("list_windows", &windows),
        ("get_window", &by_app_id),
        ("window_at_point", &at_point),
        ("list_popups", &popups),
    ] {
        let reported = pid_values(response);
        assert!(!reported.is_empty(), "{label} reported no pid field at all: {response}");
        for pid in reported {
            assert!(pid.is_null(), "{label} must report an unidentified client as null, got {pid}: {response}");
        }
    }

    // The session survives the queries and ends on request.
    let status = command(&socket_path, r#"{"command": "status"}"#, &compositor);
    assert_eq!(status["status"].as_str(), Some("ok"), "status failed after the window queries: {status}");

    let shutdown = command(&socket_path, r#"{"command": "shutdown"}"#, &compositor);
    assert_eq!(shutdown["status"].as_str(), Some("ok"), "shutdown failed: {shutdown}");
    let exit = compositor.wait_for_exit();
    let log = compositor.log();
    assert!(exit.success(), "the compositor did not exit cleanly: {exit}\n--- compositor log ---\n{log}");

    // One line per client — one Wayland client here — and it has to be the line
    // an ordinary session would show: at `warn`, which is the compositor's
    // default log level, and naming the PID namespace as the reason. Without the
    // reason these assertions would hold just as well if the credential read had
    // failed wholesale, which is the one other way every pid comes back `null`.
    let unidentified: Vec<&str> = log.lines().filter(|line| line.contains(UNIDENTIFIED_CLIENT_LOG)).collect();
    assert_eq!(
        unidentified.len(),
        1,
        "expected exactly one `{UNIDENTIFIED_CLIENT_LOG}` line\n--- compositor log ---\n{log}"
    );
    assert!(
        unidentified[0].contains("WARN"),
        "the line must be logged at warn, the compositor's default level: {}",
        unidentified[0]
    );
    assert!(
        unidentified[0].contains("SO_PEERCRED reported 0"),
        "the reason must be the foreign PID namespace, not a failed read: {}",
        unidentified[0]
    );
}

/// Every `pid` field anywhere in a response, however deeply nested.
fn pid_values(response: &Value) -> Vec<Value> {
    let mut found = Vec::new();
    collect_pids(response, &mut found);
    found
}

fn collect_pids(value: &Value, found: &mut Vec<Value>) {
    match value {
        Value::Object(fields) => {
            for (key, child) in fields {
                if key == "pid" {
                    found.push(child.clone());
                }
                collect_pids(child, found);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_pids(item, found);
            }
        }
        _ => {}
    }
}

/// The centre of a window's content area.
fn window_centre(window: &Value) -> (i64, i64) {
    let field = |name: &str| window[name].as_i64().unwrap_or_else(|| panic!("missing {name}: {window}"));
    (field("content_x") + field("content_width") / 2, field("content_y") + field("content_height") / 2)
}

/// Poll `list_popups` until the fixture's popup is reported.
fn wait_for_popup(socket_path: &Path, compositor: &Compositor) -> Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let response = command(socket_path, r#"{"command": "list_popups"}"#, compositor);
        if response["popups"].as_array().is_some_and(|popups| popups.len() == 1) {
            return response;
        }
        assert!(
            Instant::now() < deadline,
            "the fixture's popup was never listed: {response}\n--- compositor log ---\n{}",
            compositor.log()
        );
        std::thread::sleep(Duration::from_millis(200));
    }
}
