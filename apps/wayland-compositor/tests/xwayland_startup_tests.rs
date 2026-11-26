#![allow(unused_crate_dependencies)]

//! Session startup with and without `--xwayland` (spec: *compositor-session-lifecycle*).
//!
//! A compositor started with `--xwayland` holds back its readiness and its child program until
//! `XWayland` is usable. These tests start `platynui-wayland-compositor` headless with a fake
//! `Xwayland` on a private `PATH` and check every way `XWayland` can fail to become usable: the
//! session must end promptly with `1`, name the cause at ERROR level, announce no readiness,
//! start no child, and leave no server process behind. They also anchor the startup path without
//! `XWayland`, which the fix rewrites.
//!
//! The fakes are shell scripts that use only absolute paths, because the private `PATH` holds
//! nothing but the fake (and smithay passes that `PATH` on to it). They need X11 display sockets
//! to be creatable: smithay prepares `/tmp/.X11-unix/X<N>` and `/tmp/.X<N>-lock` before it
//! starts the server, so these tests touch that global state like any X server would.
//!
//! The tests marked `#[ignore]` need a real `Xwayland` on `PATH` (CI installs none). Run them
//! locally with:
//!
//! ```text
//! cargo nextest run -p platynui-wayland-compositor --run-ignored ignored-only -E 'binary(xwayland_startup_tests)'
//! ```

// This entire test suite only applies to Linux (Wayland compositor).
#![cfg(target_os = "linux")]

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use platynui_wayland_compositor as _;

/// Upper bound for one compositor session; a hang fails the test instead of blocking it. It is
/// below the watchdog `--timeout` every test passes, so an unfixed hang shows up as this
/// deadline and not as a watchdog exit.
const SESSION_DEADLINE: Duration = Duration::from_secs(30);

/// The watchdog every session gets unless a test sets its own `--timeout`.
const WATCHDOG_SECS: &str = "60";

/// A startup error must end the session within this time.
const PROMPT: Duration = Duration::from_secs(5);

/// What the private `PATH` offers as `Xwayland`.
enum Xwayland {
    /// No `Xwayland` at all.
    Missing,
    /// An `Xwayland` file without execute permission.
    NotExecutable,
    /// An executable `Xwayland` shell script with this body.
    Fake(String),
    /// The host's `PATH`, with whatever real `Xwayland` it has.
    Real,
}

/// A fake that prints a diagnostic and exits without ever reporting a display.
const DIAGNOSTIC: &str = "fake Xwayland: cannot open the display device";

fn exits_before_ready() -> Xwayland {
    Xwayland::Fake(format!("echo '{DIAGNOSTIC}' >&2\nexit 1\n"))
}

/// A fake that records its process ID in `pid_file` and stays alive without reporting a display.
/// The ID survives the `exec`, so it is the ID of the `sleep` that keeps running.
fn never_ready(pid_file: &Path) -> Xwayland {
    Xwayland::Fake(format!("echo $$ > '{}'\nexec /bin/sleep 30\n", pid_file.display()))
}

/// A fake that reports display `1` on the descriptor smithay passes after `-displayfd` and then
/// exits, so the window manager connection reaches end of file at once. It writes through
/// `/proc/self/fd/<n>` because dash rejects redirections to descriptors above 9.
fn reports_and_exits() -> Xwayland {
    Xwayland::Fake(
        "fd=\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = -displayfd ]; then fd=$2; fi\n  shift\ndone\n\
         printf '1\\n' > /proc/self/fd/$fd\nexit 0\n"
            .to_owned(),
    )
}

/// One compositor run and the files around it.
struct Session {
    name: String,
    dir: tempfile::TempDir,
    runtime: tempfile::TempDir,
    socket_name: String,
    process: Child,
    started: Instant,
}

/// The outcome of a finished run.
struct Outcome {
    status: ExitStatus,
    elapsed: Duration,
    stdout: String,
    log: String,
    marker_exists: bool,
}

impl Outcome {
    /// Everything a failed assertion should show.
    fn describe(&self) -> String {
        format!(
            "exit status: {} after {:?}\nmarker file exists: {}\nstdout:\n{}\ncompositor log:\n{}",
            self.status, self.elapsed, self.marker_exists, self.stdout, self.log
        )
    }

    fn ready_lines(&self) -> usize {
        self.log.lines().filter(|line| line.trim() == "READY").count()
    }

    fn error_lines(&self) -> Vec<&str> {
        self.log.lines().filter(|line| line.contains("ERROR")).collect()
    }

    /// Asserts the common outcome of a startup error: exit `1`, promptly, no readiness, no
    /// child, and an ERROR line containing every one of `causes`.
    fn assert_startup_error(&self, causes: &[&str]) {
        assert_eq!(self.status.code(), Some(1), "{}", self.describe());
        assert!(self.elapsed < PROMPT, "the startup error was not prompt\n{}", self.describe());
        assert_eq!(self.ready_lines(), 0, "readiness was announced\n{}", self.describe());
        assert!(!self.marker_exists, "the child program was started\n{}", self.describe());
        let errors = self.error_lines();
        assert!(
            errors.iter().any(|line| causes.iter().all(|cause| line.contains(cause))),
            "no ERROR line contains {causes:?}\n{}",
            self.describe()
        );
    }
}

/// The child program every test that expects no child passes: it creates the marker file.
fn marker_child(session_dir: &Path) -> Vec<String> {
    vec!["/bin/sh".into(), "-c".into(), format!(": > '{}'", session_dir.join("marker").display())]
}

/// smithay creates the X11 sockets before it starts any server; without them every fake would
/// report "Could not find a free socket" instead of its own cause.
fn require_x11_socket_dir() {
    let dir = Path::new("/tmp/.X11-unix");
    let writable = std::fs::metadata(dir).is_ok_and(|meta| meta.is_dir() && meta.permissions().mode() & 0o002 != 0);
    assert!(writable, "precondition: {} must exist and be world-writable for X11 display sockets", dir.display());
}

impl Session {
    /// Starts the compositor headless with `args` before `--` and `child` after it.
    fn start(name: &str, xwayland: &Xwayland, args: &[&str], child: &[String]) -> Self {
        Self::start_with(name, xwayland, args, child, |_, _| ())
    }

    /// Like [`Session::start`], with `prepare` adjusting the command and seeing the session dir
    /// before the compositor starts.
    fn start_with(
        name: &str,
        xwayland: &Xwayland,
        args: &[&str],
        child: &[String],
        prepare: impl FnOnce(&mut Command, &Path),
    ) -> Self {
        require_x11_socket_dir();
        let dir = tempfile::Builder::new().prefix("pui-xw-").tempdir().expect("create the session dir");
        let runtime = tempfile::Builder::new().prefix("pui-rt-").tempdir().expect("create a runtime dir");
        let socket_name = format!("platynui-test-xwayland-{name}-{}", std::process::id());

        let mut command = Command::new(env!("CARGO_BIN_EXE_platynui-wayland-compositor"));
        command
            .args(["--backend", "headless", "--socket-name", &socket_name, "--log-level", "info"])
            .env("XDG_RUNTIME_DIR", runtime.path())
            .env("LIBGL_ALWAYS_SOFTWARE", "1")
            .env_remove("DISPLAY")
            .env_remove("PLATYNUI_CONTROL_SOCKET")
            .env_remove("LIBEI_SOCKET");
        if !args.contains(&"--timeout") {
            command.args(["--timeout", WATCHDOG_SECS]);
        }
        command.args(args);
        if !child.is_empty() {
            command.arg("--").args(child);
        }

        let bin = dir.path().join("bin");
        std::fs::create_dir(&bin).expect("create the fake PATH dir");
        match xwayland {
            Xwayland::Real => {}
            Xwayland::Missing => {
                command.env("PATH", &bin);
            }
            Xwayland::NotExecutable => {
                std::fs::write(bin.join("Xwayland"), "#!/bin/sh\nexit 0\n").expect("write the fake");
                std::fs::set_permissions(bin.join("Xwayland"), std::fs::Permissions::from_mode(0o644))
                    .expect("make the fake non-executable");
                command.env("PATH", &bin);
            }
            Xwayland::Fake(body) => {
                std::fs::write(bin.join("Xwayland"), format!("#!/bin/sh\n{body}")).expect("write the fake");
                std::fs::set_permissions(bin.join("Xwayland"), std::fs::Permissions::from_mode(0o755))
                    .expect("make the fake executable");
                command.env("PATH", &bin);
            }
        }

        prepare(&mut command, dir.path());
        let stdout = File::create(dir.path().join("stdout")).expect("create the stdout file");
        let log = File::create(dir.path().join("log")).expect("create the log file");
        let process = command
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(log))
            .spawn()
            .expect("failed to start the compositor");
        Self { name: name.to_owned(), dir, runtime, socket_name, process, started: Instant::now() }
    }

    fn path(&self, file: &str) -> PathBuf {
        self.dir.path().join(file)
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.path("log")).unwrap_or_default()
    }

    /// Waits for the compositor to exit, killing it at [`SESSION_DEADLINE`].
    fn finish(mut self) -> Outcome {
        let status = loop {
            if let Some(status) = self.process.try_wait().expect("failed to wait for the compositor") {
                break status;
            }
            if self.started.elapsed() > SESSION_DEADLINE {
                let _ = self.process.kill();
                let _ = self.process.wait();
                panic!(
                    "[{}] compositor did not exit within {SESSION_DEADLINE:?}\ncompositor log:\n{}",
                    self.name,
                    self.log()
                );
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        Outcome {
            status,
            elapsed: self.started.elapsed(),
            stdout: std::fs::read_to_string(self.path("stdout")).unwrap_or_default(),
            log: self.log(),
            marker_exists: self.path("marker").exists(),
        }
    }

    /// Sends one control-socket request, retrying until the socket answers or `within` passes.
    fn control(&self, request: &str, within: Duration) -> serde_json::Value {
        let path = self.runtime.path().join(format!("{}.control", self.socket_name));
        let deadline = Instant::now() + within;
        loop {
            match Self::try_control(&path, request) {
                Ok(reply) => return reply,
                Err(err) if Instant::now() > deadline => {
                    panic!("[{}] control request {request} failed: {err}\ncompositor log:\n{}", self.name, self.log())
                }
                Err(_) => std::thread::sleep(Duration::from_millis(50)),
            }
        }
    }

    fn try_control(path: &Path, request: &str) -> std::io::Result<serde_json::Value> {
        let mut stream = UnixStream::connect(path)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        writeln!(stream, "{request}")?;
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line)?;
        serde_json::from_str(&line).map_err(std::io::Error::other)
    }

    /// Waits until the log contains `needle`, or fails after `within`.
    fn wait_for_log(&self, needle: &str, within: Duration) {
        let deadline = Instant::now() + within;
        while !self.log().contains(needle) {
            assert!(Instant::now() < deadline, "[{}] the log never contained {needle:?}\n{}", self.name, self.log());
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

/// Whether the process with the ID recorded in `pid_file` still runs. A zombie does not run: its
/// parent was the compositor, which has exited, and only `init` still has to reap it.
fn recorded_process_runs(pid_file: &Path) -> Option<u32> {
    let pid: u32 = std::fs::read_to_string(pid_file).ok()?.trim().parse().ok()?;
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // The state follows the command name in parentheses, which may itself contain spaces.
    let state = stat.rsplit_once(')')?.1.split_whitespace().next()?;
    (state != "Z" && state != "X").then_some(pid)
}

/// Asserts that the fake recorded in `pid_file` no longer runs. A survivor is killed so a failed
/// run leaves no `sleep` behind.
fn assert_no_server_left(pid_file: &Path, outcome: &Outcome) {
    // The kernel finishes a killed process asynchronously; give it a moment.
    let deadline = Instant::now() + Duration::from_secs(2);
    while recorded_process_runs(pid_file).is_some() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    if let Some(pid) = recorded_process_runs(pid_file) {
        let _ = Command::new("/bin/kill").args(["-KILL", &pid.to_string()]).status();
        panic!("the fake Xwayland (pid {pid}) still runs after the compositor exited\n{}", outcome.describe());
    }
}

fn stdout_has(outcome: &Outcome, key: &str) -> bool {
    outcome.stdout.lines().any(|line| line.starts_with(&format!("{key}=")))
}

// ── Anchors for the startup path without XWayland ───────────────────────────

#[test]
fn readiness_without_xwayland_lists_the_sessions_sockets() {
    let session =
        Session::start("anchor-env", &Xwayland::Real, &["--print-env", "--exit-with-child"], &["/bin/true".into()]);
    let outcome = session.finish();
    assert_eq!(outcome.status.code(), Some(0), "{}", outcome.describe());
    for key in ["WAYLAND_DISPLAY", "PLATYNUI_CONTROL_SOCKET", "LIBEI_SOCKET"] {
        assert!(stdout_has(&outcome, key), "--print-env lacks {key}\n{}", outcome.describe());
    }
    assert!(!stdout_has(&outcome, "DISPLAY"), "--print-env lists DISPLAY without XWayland\n{}", outcome.describe());
    assert_eq!(outcome.ready_lines(), 1, "READY must appear exactly once\n{}", outcome.describe());
}

#[test]
fn the_child_sees_the_ready_sessions_environment() {
    let check = r#"[ -n "$WAYLAND_DISPLAY" ] && [ -n "$PLATYNUI_CONTROL_SOCKET" ] && [ -n "$LIBEI_SOCKET" ] && [ -z "$DISPLAY" ]"#;
    let session = Session::start_with(
        "anchor-child-env",
        &Xwayland::Real,
        &["--exit-with-child"],
        &["/bin/sh".into(), "-c".into(), check.into()],
        |command, _| {
            command.env("DISPLAY", ":99");
        },
    );
    let outcome = session.finish();
    assert_eq!(outcome.status.code(), Some(0), "the child saw the wrong environment\n{}", outcome.describe());
}

// ── A requested XWayland that cannot start is a startup error ───────────────

#[test]
fn xwayland_not_found_fails_the_start() {
    let session = Session::start_with(
        "not-found",
        &Xwayland::Missing,
        &["--xwayland", "--exit-with-child"],
        &[],
        |command, dir| {
            command.arg("--").args(marker_child(dir));
        },
    );
    session.finish().assert_startup_error(&["Xwayland", "not found"]);
}

#[test]
fn xwayland_not_found_fails_the_start_without_exit_with_child() {
    let session = Session::start("not-found-no-child", &Xwayland::Missing, &["--xwayland"], &[]);
    let outcome = session.finish();
    assert_eq!(outcome.status.code(), Some(1), "{}", outcome.describe());
    assert!(outcome.elapsed < PROMPT, "the startup error was not prompt\n{}", outcome.describe());
    assert_eq!(outcome.ready_lines(), 0, "readiness was announced\n{}", outcome.describe());
}

#[test]
fn a_non_executable_xwayland_fails_the_start() {
    let session = Session::start_with(
        "not-executable",
        &Xwayland::NotExecutable,
        &["--xwayland", "--exit-with-child"],
        &[],
        |command, dir| {
            command.arg("--").args(marker_child(dir));
        },
    );
    session.finish().assert_startup_error(&["Xwayland", "not executable"]);
}

#[test]
fn xwayland_exiting_before_it_is_ready_fails_the_start_promptly() {
    let session = Session::start_with(
        "exits-early",
        &exits_before_ready(),
        &["--xwayland", "--xwayland-timeout", "30", "--exit-with-child"],
        &[],
        |command, dir| {
            command.arg("--").args(marker_child(dir));
        },
    );
    session.finish().assert_startup_error(&["exited before it was ready", DIAGNOSTIC]);
}

#[test]
fn xwayland_never_ready_fails_the_start_after_the_timeout() {
    let pid_dir = tempfile::tempdir().expect("pid dir");
    let pid_file = pid_dir.path().join("xwayland.pid");
    let session = Session::start_with(
        "never-ready",
        &never_ready(&pid_file),
        &["--xwayland", "--xwayland-timeout", "1", "--exit-with-child"],
        &[],
        |command, dir| {
            command.arg("--").args(marker_child(dir));
        },
    );
    let outcome = session.finish();
    outcome.assert_startup_error(&["did not become ready within 1 s"]);
    assert_no_server_left(&pid_file, &outcome);
}

#[test]
fn a_window_manager_that_cannot_start_fails_the_start() {
    let session = Session::start_with(
        "wm-fails",
        &reports_and_exits(),
        &["--xwayland", "--print-env", "--exit-with-child"],
        &[],
        |command, dir| {
            command.arg("--").args(marker_child(dir));
        },
    );
    let outcome = session.finish();
    outcome.assert_startup_error(&["X11 window manager failed to start"]);
    assert!(!stdout_has(&outcome, "DISPLAY"), "DISPLAY was printed for an unusable server\n{}", outcome.describe());
}

#[test]
fn a_session_ended_while_xwayland_is_starting_fails() {
    let pid_dir = tempfile::tempdir().expect("pid dir");
    let pid_file = pid_dir.path().join("xwayland.pid");
    let session = Session::start(
        "ends-while-starting",
        &never_ready(&pid_file),
        &["--xwayland", "--xwayland-timeout", "30", "--timeout", "1"],
        &[],
    );
    let outcome = session.finish();
    assert_eq!(outcome.status.code(), Some(1), "{}", outcome.describe());
    assert!(
        outcome.elapsed >= Duration::from_secs(1) && outcome.elapsed < PROMPT,
        "the session did not end with its 1 s watchdog\n{}",
        outcome.describe()
    );
    assert_eq!(outcome.ready_lines(), 0, "readiness was announced\n{}", outcome.describe());
    assert_no_server_left(&pid_file, &outcome);
}

#[test]
fn a_server_that_never_reports_readiness_is_terminated() {
    let pid_dir = tempfile::tempdir().expect("pid dir");
    let pid_file = pid_dir.path().join("xwayland.pid");
    let session =
        Session::start("terminated", &never_ready(&pid_file), &["--xwayland", "--xwayland-timeout", "1"], &[]);
    let outcome = session.finish();
    assert_eq!(outcome.status.code(), Some(1), "{}", outcome.describe());
    assert_no_server_left(&pid_file, &outcome);
}

#[test]
fn an_invalid_startup_timeout_is_rejected() {
    let session =
        Session::start("invalid-timeout", &Xwayland::Missing, &["--xwayland", "--xwayland-timeout", "0"], &[]);
    let outcome = session.finish();
    assert!(!outcome.status.success(), "{}", outcome.describe());
    assert!(
        outcome.log.contains("--xwayland-timeout"),
        "the usage message does not name the option\n{}",
        outcome.describe()
    );
    // Not clap's "unexpected argument": the option must exist and reject the value.
    assert!(outcome.log.contains("invalid value"), "the value was not rejected as invalid\n{}", outcome.describe());
    assert!(!outcome.log.contains("compositor ready"), "a session was started\n{}", outcome.describe());
}

#[test]
fn status_reports_xwayland_unusable_while_it_is_starting() {
    let pid_dir = tempfile::tempdir().expect("pid dir");
    let pid_file = pid_dir.path().join("xwayland.pid");
    let mut session =
        Session::start("status-starting", &never_ready(&pid_file), &["--xwayland", "--xwayland-timeout", "10"], &[]);
    let status = session.control(r#"{"command":"status"}"#, Duration::from_secs(5));
    let xwayland = status.get("xwayland").cloned();
    // End the session while XWayland is still starting; the fix kills the fake with it.
    let _ = Command::new("/bin/kill").args(["-TERM", &session.process.id().to_string()]).status();
    let _ = session.process.wait();
    if let Some(pid) = recorded_process_runs(&pid_file) {
        let _ = Command::new("/bin/kill").args(["-KILL", &pid.to_string()]).status();
    }
    assert_eq!(xwayland, Some(serde_json::Value::Bool(false)), "status: {status}\ncompositor log:\n{}", session.log());
}

// ── Real XWayland (run locally with --run-ignored) ──────────────────────────

#[test]
#[ignore = "needs a real Xwayland binary on PATH"]
fn a_working_xwayland_session_succeeds() {
    let session = Session::start(
        "real-success",
        &Xwayland::Real,
        &["--xwayland", "--print-env", "--exit-with-child"],
        &["/bin/sh".into(), "-c".into(), r#"test -n "$DISPLAY""#.into()],
    );
    let outcome = session.finish();
    assert_eq!(outcome.status.code(), Some(0), "{}", outcome.describe());
    assert!(stdout_has(&outcome, "DISPLAY"), "no DISPLAY line\n{}", outcome.describe());
}

#[test]
#[ignore = "needs a real Xwayland binary on PATH"]
fn readiness_with_xwayland_waits_for_the_window_manager() {
    let session = Session::start(
        "real-order",
        &Xwayland::Real,
        &["--xwayland", "--print-env", "--exit-with-child"],
        &["/bin/true".into()],
    );
    let outcome = session.finish();
    assert_eq!(outcome.status.code(), Some(0), "{}", outcome.describe());
    let position = |needle: &str| {
        outcome.log.find(needle).unwrap_or_else(|| panic!("the log lacks {needle:?}\n{}", outcome.describe()))
    };
    let wm = position("X11 window manager started");
    assert!(wm < position("compositor ready"), "readiness came before the window manager\n{}", outcome.describe());
    assert!(
        wm < position("spawning child program"),
        "the child started before the window manager\n{}",
        outcome.describe()
    );
}

#[test]
#[ignore = "needs a real Xwayland binary on PATH"]
fn status_reports_a_usable_xwayland() {
    let session = Session::start(
        "real-status",
        &Xwayland::Real,
        &["--xwayland", "--exit-with-child"],
        &["/bin/sleep".into(), "30".into()],
    );
    session.wait_for_log("compositor ready", Duration::from_secs(20));
    let status = session.control(r#"{"command":"status"}"#, Duration::from_secs(5));

    let ctl =
        Path::new(env!("CARGO_BIN_EXE_platynui-wayland-compositor")).with_file_name("platynui-wayland-compositor-ctl");
    assert!(ctl.exists(), "build the ctl tool first: cargo build -p platynui-wayland-compositor-ctl");
    let control_socket = session.runtime.path().join(format!("{}.control", session.socket_name));
    let ctl_output = Command::new(&ctl)
        .args(["--socket", &control_socket.display().to_string(), "status"])
        .output()
        .expect("run the ctl tool");
    let _ = session.control(r#"{"command":"shutdown"}"#, Duration::from_secs(5));
    let outcome = session.finish();

    assert_eq!(
        status.get("xwayland"),
        Some(&serde_json::Value::Bool(true)),
        "status: {status}\n{}",
        outcome.describe()
    );
    let ctl_text = String::from_utf8_lossy(&ctl_output.stdout);
    let line = ctl_text.lines().find(|line| line.trim_start().starts_with("XWayland:")).unwrap_or_default();
    assert!(line.split_whitespace().any(|word| word == "yes"), "ctl output:\n{ctl_text}");
}
