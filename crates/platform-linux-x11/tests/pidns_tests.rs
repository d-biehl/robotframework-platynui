// Must precede the `cfg`: a false crate-level `cfg` drops every attribute after
// it, and the crate left empty on other targets still sees every dependency.
#![allow(unused_crate_dependencies)]
// This test only applies to Linux (X11, PID namespaces).
#![cfg(target_os = "linux")]

//! PID-namespace integration tests for the X11 hit-test's own-window decision.
//!
//! A window says which process it belongs to through `_NET_WM_PID`, a number in
//! the client's PID namespace. The window manager skips a window as the
//! runtime's own only when two witnesses agree: the window reports our PID, and
//! the X server attributes it to the same process as our own connection (see
//! `window_manager.rs`, *Own-window ownership*). These tests build the
//! situations with a real `Xvfb`, each window created where it is created in the
//! deployment it stands for:
//!
//! - **The sidecar** — `Xvfb` and the application in one PID namespace, the
//!   hit-testing runtime in a sibling, the application's window reporting the
//!   runtime's own in-namespace PID. The measured collision: the window must be
//!   resolved.
//! - **A server that cannot see the runtime** (WSLg's shape) — `Xvfb` in its own
//!   namespace, the runtime and its windows in a sibling: the runtime's own
//!   window is skipped, the window behind resolved.
//! - **A runtime in a child namespace** (a container on the host's display) —
//!   the runtime's own window is skipped, while a host application's window
//!   that reuses the runtime's in-namespace PID is resolved.
//! - **One namespace** — a window reporting our PID is skipped and the window
//!   behind it resolved, exactly as before.
//! - **No X-Resource** — `Xvfb` without the extension; the previous comparison
//!   stays and the log warns once.
//!
//! The tests are `#[ignore]`d because unprivileged user namespaces are not
//! available everywhere (`AppArmor` blocks them on stock Ubuntu 24.04). Run them
//! with:
//!
//! ```sh
//! just test-x11-pidns
//! ```
//!
//! A missing prerequisite — `unshare` or unprivileged user namespaces, `Xvfb`,
//! a display that never comes up — **fails** the run with a message naming it.
//! It never skips: an ignored test only runs when someone asks for exactly this
//! coverage, so a skip would read as a pass in the one run meant to exercise it.
//! This follows `crates/java-agent/tests/live_fixture.rs`.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use platynui_core::platform::WindowManager;
use platynui_core::types::Point;
use platynui_platform_linux_x11::X11Connection;
use platynui_platform_linux_x11::window_manager::X11EwmhWindowManager;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, CreateWindowAux, PropMode, Window, WindowClass};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

/// A user namespace that keeps our uid, and a PID namespace of its own.
const UNSHARE_ARGS: [&str; 7] =
    ["--user", "--map-current-user", "--keep-caps", "--pid", "--fork", "--mount-proc", "--kill-child"];

const ROLE_ENV: &str = "PLATYNUI_X11_PIDNS_ROLE";
const DIR_ENV: &str = "PLATYNUI_X11_PIDNS_DIR";
const DISPLAY_ENV: &str = "PLATYNUI_X11_PIDNS_DISPLAY";
/// Set for a probe that lays its own window over the point before hit-testing.
const OWN_WINDOW_ENV: &str = "PLATYNUI_X11_PIDNS_OWN_WINDOW";
/// For such a probe: a window reporting this PID goes behind its own, and the
/// probe lists both in the stacking order itself.
const BEHIND_PID_ENV: &str = "PLATYNUI_X11_PIDNS_BEHIND_PID";

/// The log line the window manager writes once per connection.
const DECIDED_LOG: &str = "X11 own-window identification decided";
const UNVERIFIED_LOG: &str = "X11 own-window exclusion is unverified on this display";

/// A point inside every fixture window.
const POINT: (f64, f64) = (200.0, 200.0);
/// Where a window behind is laid.
const BEHIND: (i16, i16, u16, u16) = (100, 100, 300, 200);
/// Where the runtime's own window is laid, over the window behind.
const OWN: (i16, i16, u16, u16) = (150, 150, 150, 100);

/// How often a probe hit-tests, so the log can show the decision is taken once.
const HITS: usize = 3;

/// The PID a window behind the runtime's own reports where it collides with
/// nothing.
const SOMEBODY_ELSE: u32 = 7;

// ── The roles ───────────────────────────────────────────────────────────────

/// The runtime side of a namespace test, when this binary was started as the
/// probe — and nothing otherwise, so the ignored test is harmless in a run that
/// forces ignored tests.
///
/// It publishes its PID, optionally lays its own window (reporting that PID)
/// over the point, waits for the go signal, hit-tests several times and records
/// what it resolved.
#[test]
#[ignore = "the probe of the X11 PID-namespace tests, started by them"]
fn harness_probe() {
    if std::env::var(ROLE_ENV).as_deref() != Ok("probe") {
        return;
    }
    let dir = PathBuf::from(std::env::var(DIR_ENV).expect("the probe is given its directory"));
    let display = std::env::var(DISPLAY_ENV).expect("the probe is given the display");
    std::fs::write(dir.join("probe-pid"), std::process::id().to_string()).expect("the directory is writable");

    // The runtime's own UI, on a connection of its own as a toolkit would open
    // one; it lives until the probe ends.
    let _own_ui = std::env::var_os(OWN_WINDOW_ENV).map(|_| {
        let fixture = Fixture::new(&display);
        let behind =
            std::env::var(BEHIND_PID_ENV).ok().map(|pid| fixture.add_window(BEHIND, Some(pid.parse().expect("a PID"))));
        let own = fixture.add_window(OWN, Some(std::process::id()));
        if let Some(behind) = behind {
            fixture.set_stacking(&[behind, own]);
            write_record(&dir, "behind-window", &behind.to_string());
        }
        write_record(&dir, "own-window", &own.to_string());
        fixture
    });
    wait_for(&dir.join("go"), "the go signal", &[]);

    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .try_init();
    let wm = X11EwmhWindowManager::new(X11Connection::connect(Some(&display)).expect("the probe reaches the display"));
    let mut hit = None;
    for _ in 0..HITS {
        hit = wm.window_at_point(Point::new(POINT.0, POINT.1)).expect("the hit-test does not fail");
    }
    let record = match hit {
        Some(hit) => format!("xid={}\npid={:?}\n", hit.id.raw(), hit.pid),
        None => "xid=none\npid=None\n".to_string(),
    };
    write_record(&dir, "probe-result", &record);
}

/// The application container of the sidecar test, when this binary was started
/// as it: the display server and the application's window in one PID
/// namespace, so the server sees the application as it sees any local client.
#[test]
#[ignore = "the application container of the X11 PID-namespace tests, started by them"]
fn harness_app() {
    if std::env::var(ROLE_ENV).as_deref() != Ok("app") {
        return;
    }
    let dir = PathBuf::from(std::env::var(DIR_ENV).expect("the application is given its directory"));
    let (_xvfb, number) = spawn_xvfb(&dir.join("xvfb.log"), false, &[]);
    write_record(&dir, "display", &number);
    let pid: u32 =
        wait_for(&dir.join("app-window-pid"), "the PID the window reports", &[]).trim().parse().expect("a PID");
    let fixture = Fixture::new(&format!(":{number}"));
    let window = fixture.add_window(BEHIND, Some(pid));
    write_record(&dir, "app-window", &window.to_string());
    // Keep the window and the server until the test ends the namespace.
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline && !dir.join("stop").exists() {
        std::thread::sleep(Duration::from_millis(50));
    }
}

// ── The tests ───────────────────────────────────────────────────────────────

/// Spec: *An application whose process identifier equals the runtime's is still
/// resolved* — measured before as *No element*.
#[test]
#[ignore = "needs unprivileged user namespaces and Xvfb; run with `just test-x11-pidns`"]
fn an_application_reusing_the_runtimes_pid_is_resolved() {
    require_pid_namespaces();
    require_xvfb();
    let dir = HarnessDir::new("sidecar");
    let app_log = dir.path().join("app.log");
    let _app = spawn_role("app", &dir, &[], &app_log);
    let number = wait_for(&dir.path().join("display"), "the application's display", &[&app_log]).trim().to_string();
    let _display_files = DisplayFiles(number.clone());
    let display = format!(":{number}");

    let probe_log = dir.path().join("probe.log");
    let _probe = spawn_role("probe", &dir, &[(DISPLAY_ENV, &display)], &probe_log);
    let probe_pid = read_pid_record(&dir, "probe-pid", &probe_log);

    // The collision: the application's window reports the runtime's own
    // in-namespace PID. It must take effect, or the test proves nothing.
    write_record(&dir.0, "app-window-pid", &probe_pid.to_string());
    let window: Window = wait_for(&dir.path().join("app-window"), "the application's window", &[&app_log])
        .trim()
        .parse()
        .expect("an XID");
    let fixture = Fixture::new(&display);
    fixture.set_stacking(&[window]);
    assert_eq!(fixture.read_pid(window), Some(probe_pid), "the collision was not set up");

    let (result, log) = run_probe(&dir, &probe_log);
    assert!(
        result.contains(&format!("xid={window}")),
        "the application's window must be resolved although it reports the runtime's PID:\n{result}\n{log}"
    );
    assert!(result.contains(&format!("pid=Some({probe_pid})")), "{result}");
    // What the server said about the runtime, from a sibling namespace: `0`.
    assert_eq!(log.matches(DECIDED_LOG).count(), 1, "the decision is logged once per connection:\n{log}");
    assert!(log.contains("server_view=Reported(0)"), "{log}");
    assert!(log.contains("mode=Foreign"), "{log}");
}

/// Spec: *The runtime's own window is skipped where the window system numbers it
/// differently* — the server sees neither the runtime nor its windows (WSLg).
#[test]
#[ignore = "needs unprivileged user namespaces and Xvfb; run with `just test-x11-pidns`"]
fn the_runtimes_own_window_is_skipped_where_the_server_cannot_see_it() {
    require_pid_namespaces();
    require_xvfb();
    let dir = HarnessDir::new("unseen");
    let xvfb = Xvfb::start(&dir, true, &[]);
    let probe_log = dir.path().join("probe.log");
    let behind_pid = SOMEBODY_ELSE.to_string();
    let _probe = spawn_role(
        "probe",
        &dir,
        &[(DISPLAY_ENV, &xvfb.display), (OWN_WINDOW_ENV, "1"), (BEHIND_PID_ENV, &behind_pid)],
        &probe_log,
    );
    let behind = wait_for(&dir.path().join("behind-window"), "the window behind", &[&probe_log]).trim().to_string();
    wait_for(&dir.path().join("own-window"), "the runtime's own window", &[&probe_log]);

    let (result, log) = run_probe(&dir, &probe_log);
    assert!(result.contains(&format!("xid={behind}")), "the window behind our own must be resolved:\n{result}\n{log}");
    assert!(log.contains("server_view=Reported(0)"), "{log}");
    assert!(log.contains("mode=Foreign"), "{log}");
}

/// Spec: *The runtime's own window is skipped where the window system numbers it
/// differently* — the runtime in a child namespace of the server's, with a host
/// application reusing its in-namespace PID.
#[test]
#[ignore = "needs unprivileged user namespaces and Xvfb; run with `just test-x11-pidns`"]
fn the_runtimes_own_window_is_skipped_from_a_child_namespace_while_a_colliding_window_is_resolved() {
    require_pid_namespaces();
    require_xvfb();
    let dir = HarnessDir::new("child");
    let xvfb = Xvfb::start(&dir, false, &[]);
    let probe_log = dir.path().join("probe.log");
    let _probe = spawn_role("probe", &dir, &[(DISPLAY_ENV, &xvfb.display), (OWN_WINDOW_ENV, "1")], &probe_log);
    let probe_pid = read_pid_record(&dir, "probe-pid", &probe_log);
    let own: Window = wait_for(&dir.path().join("own-window"), "the runtime's own window", &[&probe_log])
        .trim()
        .parse()
        .expect("an XID");

    // A host application behind the runtime's window, reporting the runtime's
    // in-namespace PID.
    let fixture = Fixture::new(&xvfb.display);
    let behind = fixture.add_window(BEHIND, Some(probe_pid));
    fixture.set_stacking(&[behind, own]);
    assert_eq!(fixture.read_pid(behind), Some(probe_pid), "the collision was not set up");

    let (result, log) = run_probe(&dir, &probe_log);
    assert!(
        result.contains(&format!("xid={behind}")),
        "our own window must be skipped and the colliding window behind it resolved:\n{result}\n{log}"
    );
    assert!(result.contains(&format!("pid=Some({probe_pid})")), "{result}");
    assert!(
        !log.contains(&format!("server_view=Reported({probe_pid})")),
        "the server numbers the probe differently:\n{log}"
    );
    assert!(log.contains("mode=Foreign"), "{log}");
}

/// Spec: *A point over the host process's own window is skipped*.
#[test]
#[ignore = "needs Xvfb; run with `just test-x11-pidns`"]
fn the_runtimes_own_window_is_skipped_where_the_server_numbers_it_alike() {
    require_xvfb();
    let dir = HarnessDir::new("verified");
    let xvfb = Xvfb::start(&dir, false, &[]);
    let (behind, log) = hit_test_over_own_window(&xvfb.display);
    assert_eq!(log.matches(DECIDED_LOG).count(), 1, "the decision is logged once per connection:\n{log}");
    assert!(log.contains("mode=Verified"), "{log}");
    assert_eq!(behind.0, behind.1, "the window behind our own must be resolved");
}

/// Spec: *The window system cannot be asked, so the previous behaviour is kept
/// and reported*.
#[test]
#[ignore = "needs Xvfb; run with `just test-x11-pidns`"]
fn a_server_without_x_resource_keeps_the_previous_comparison_and_warns_once() {
    require_xvfb();
    let dir = HarnessDir::new("unknown");
    let xvfb = Xvfb::start(&dir, false, &["-extension", "X-Resource"]);
    let (behind, log) = hit_test_over_own_window(&xvfb.display);
    assert_eq!(log.matches(UNVERIFIED_LOG).count(), 1, "the warning is emitted once per connection:\n{log}");
    assert_eq!(log.matches(DECIDED_LOG).count(), 0, "{log}");
    assert_eq!(behind.0, behind.1, "the window reporting our PID is still skipped");
}

/// Lay a window reporting our own PID over a window reporting another, hit-test
/// the overlap several times in this process, and answer (the window resolved,
/// the window behind) together with the captured log.
fn hit_test_over_own_window(display: &str) -> ((Option<u64>, Option<u64>), String) {
    let fixture = Fixture::new(display);
    let behind = fixture.add_window(BEHIND, Some(SOMEBODY_ELSE));
    let own = fixture.add_window(OWN, Some(std::process::id()));
    fixture.set_stacking(&[behind, own]);

    let captured = Captured::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(captured.clone())
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .finish();
    let resolved = tracing::subscriber::with_default(subscriber, || {
        let wm = X11EwmhWindowManager::new(X11Connection::connect(Some(display)).expect("reach the display"));
        let mut resolved = None;
        for _ in 0..HITS {
            resolved = wm.window_at_point(Point::new(POINT.0, POINT.1)).expect("the hit-test does not fail");
        }
        resolved
    });
    ((resolved.map(|hit| hit.id.raw()), Some(u64::from(behind))), captured.text())
}

/// Start this binary in a PID namespace of its own, in `role`.
fn spawn_role(role: &str, dir: &HarnessDir, env: &[(&str, &str)], log: &Path) -> KillOnDrop {
    let test = format!("harness_{role}");
    let mut command = Command::new("unshare");
    command
        .args(UNSHARE_ARGS)
        .arg(std::env::current_exe().expect("the test binary has a path"))
        .args(["--exact", &test, "--include-ignored", "--nocapture"])
        .env(ROLE_ENV, role)
        .env(DIR_ENV, dir.path())
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(log).expect("the directory is writable"));
    for (key, value) in env {
        command.env(key, value);
    }
    KillOnDrop(command.spawn().unwrap_or_else(|err| panic!("prerequisite missing: cannot run `unshare`: {err}")))
}

/// Give the probe the go signal and answer its hit-test record and its log.
fn run_probe(dir: &HarnessDir, probe_log: &Path) -> (String, String) {
    write_record(&dir.0, "go", "go");
    let result = wait_for(&dir.path().join("probe-result"), "the probe's hit-test", &[probe_log]);
    let log = std::fs::read_to_string(probe_log).unwrap_or_default();
    println!("hit-test result:\n{result}probe log:\n{log}");
    (result, log)
}

fn read_pid_record(dir: &HarnessDir, name: &str, log: &Path) -> u32 {
    wait_for(&dir.path().join(name), name, &[log]).trim().parse().expect("a PID record holds a number")
}

/// Write `content` under `name` in `dir` in one step, so a reader never sees it
/// half written.
fn write_record(dir: &Path, name: &str, content: &str) {
    let partial = dir.join(format!("{name}.partial"));
    std::fs::write(&partial, content).expect("the directory is writable");
    std::fs::rename(&partial, dir.join(name)).expect("the directory is writable");
}

/// The lock and socket files of a display whose server died with its namespace.
struct DisplayFiles(String);

impl Drop for DisplayFiles {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(format!("/tmp/.X{}-lock", self.0));
        let _ = std::fs::remove_file(format!("/tmp/.X11-unix/X{}", self.0));
    }
}

// ── Prerequisites ───────────────────────────────────────────────────────────

/// Fail unless this machine can put a process in its own PID namespace.
fn require_pid_namespaces() {
    match Command::new("unshare").args(UNSHARE_ARGS).arg("/bin/true").output() {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            panic!("prerequisite missing: `unshare` is not installed (util-linux)")
        }
        Err(err) => panic!("prerequisite missing: cannot run `unshare`: {err}"),
        Ok(output) if !output.status.success() => panic!(
            "prerequisite missing: unprivileged user namespaces are unavailable (`unshare {}` exited with {}: {}). \
             Allow them with `sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0`, and check that \
             `user.max_user_namespaces` is not 0.",
            UNSHARE_ARGS.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim(),
        ),
        Ok(_) => {}
    }
}

/// Fail unless `Xvfb` can be started.
fn require_xvfb() {
    if let Err(err) = Command::new("Xvfb").arg("-help").stdout(Stdio::null()).stderr(Stdio::null()).status() {
        panic!("prerequisite missing: `Xvfb` cannot be started ({err}); install the X virtual framebuffer");
    }
}

// ── The display ─────────────────────────────────────────────────────────────

/// An `Xvfb` on a display number it picks itself, optionally in a PID
/// namespace of its own. Dropping it stops the server and removes its files.
struct Xvfb {
    child: Child,
    display: String,
    number: String,
}

impl Xvfb {
    fn start(dir: &HarnessDir, own_pid_namespace: bool, extra: &[&str]) -> Self {
        let log = dir.path().join(if own_pid_namespace { "xvfb-ns.log" } else { "xvfb.log" });
        let (child, number) = spawn_xvfb(&log, own_pid_namespace, extra);
        Self { child, display: format!(":{number}"), number }
    }
}

/// Start `Xvfb` on a display number it picks itself, and answer the process and
/// that number once the server accepts clients.
fn spawn_xvfb(log: &Path, own_pid_namespace: bool, extra: &[&str]) -> (Child, String) {
    let mut command = if own_pid_namespace {
        let mut command = Command::new("unshare");
        command.args(UNSHARE_ARGS).arg("Xvfb");
        command
    } else {
        Command::new("Xvfb")
    };
    let mut child = command
        .args(["-displayfd", "1", "-nolisten", "tcp", "-screen", "0", "800x600x24"])
        .args(extra)
        .stdout(Stdio::piped())
        .stderr(std::fs::File::create(log).expect("the directory is writable"))
        .spawn()
        .unwrap_or_else(|err| panic!("prerequisite missing: cannot start `Xvfb`: {err}"));

    // `-displayfd` writes the display number once the server accepts clients.
    let stdout = child.stdout.take().expect("stdout is piped");
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = BufReader::new(stdout).read_line(&mut line);
        let _ = sender.send(line);
    });
    match receiver.recv_timeout(Duration::from_secs(20)) {
        Ok(line) if !line.trim().is_empty() => (child, line.trim().to_string()),
        _ => {
            let _ = child.kill();
            let _ = child.wait();
            panic!("prerequisite missing: `Xvfb` never came up\n{}", std::fs::read_to_string(log).unwrap_or_default());
        }
    }
}

impl Drop for Xvfb {
    fn drop(&mut self) {
        // SIGKILL reaches `unshare` too, whose `--kill-child` then ends the server.
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(format!("/tmp/.X{}-lock", self.number));
        let _ = std::fs::remove_file(format!("/tmp/.X11-unix/X{}", self.number));
    }
}

/// Windows on the display, set up the way a window manager and its clients
/// would leave them: mapped, with `_NET_WM_PID`, and listed bottom-to-top in the
/// root's `_NET_CLIENT_LIST_STACKING`.
struct Fixture {
    conn: RustConnection,
    root: Window,
}

impl Fixture {
    fn new(display: &str) -> Self {
        let (conn, screen) = x11rb::connect(Some(display)).expect("the fixture reaches the display");
        let root = conn.setup().roots[screen].root;
        Self { conn, root }
    }

    fn atom(&self, name: &str) -> u32 {
        self.conn.intern_atom(false, name.as_bytes()).expect("intern").reply().expect("intern reply").atom
    }

    fn add_window(&self, (x, y, width, height): (i16, i16, u16, u16), pid: Option<u32>) -> Window {
        let window = self.conn.generate_id().expect("an XID");
        self.conn
            .create_window(
                x11rb::COPY_DEPTH_FROM_PARENT,
                window,
                self.root,
                x,
                y,
                width,
                height,
                0,
                WindowClass::INPUT_OUTPUT,
                0,
                &CreateWindowAux::new(),
            )
            .expect("create window");
        if let Some(pid) = pid {
            self.conn
                .change_property32(PropMode::REPLACE, window, self.atom("_NET_WM_PID"), AtomEnum::CARDINAL, &[pid])
                .expect("set _NET_WM_PID");
        }
        self.conn.map_window(window).expect("map window");
        self.sync();
        window
    }

    fn set_stacking(&self, bottom_to_top: &[Window]) {
        for name in ["_NET_CLIENT_LIST_STACKING", "_NET_CLIENT_LIST"] {
            self.conn
                .change_property32(PropMode::REPLACE, self.root, self.atom(name), AtomEnum::WINDOW, bottom_to_top)
                .expect("set the client list");
        }
        self.sync();
    }

    fn read_pid(&self, window: Window) -> Option<u32> {
        let reply = self
            .conn
            .get_property(false, window, self.atom("_NET_WM_PID"), AtomEnum::CARDINAL, 0, 1)
            .ok()?
            .reply()
            .ok()?;
        reply.value32().and_then(|mut values| values.next())
    }

    /// Round-trip to the server, so everything sent so far has been applied.
    fn sync(&self) {
        self.conn.get_input_focus().expect("sync").reply().expect("sync reply");
    }
}

// ── Small helpers ───────────────────────────────────────────────────────────

/// The tests' directory, removed when they end.
struct HarnessDir(PathBuf);

impl HarnessDir {
    fn new(tag: &str) -> Self {
        let base = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|dir| dir.is_dir())
            .unwrap_or_else(std::env::temp_dir);
        let dir = base.join(format!("x11-pidns-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)
            .unwrap_or_else(|err| panic!("prerequisite missing: cannot create {}: {err}", dir.display()));
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for HarnessDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A process killed when the test ends, however it ends.
struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Wait for `path` to appear and return its content, failing with the given
/// logs if it never does.
fn wait_for(path: &Path, what: &str, logs: &[&Path]) -> String {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Ok(content) = std::fs::read_to_string(path)
            && !content.is_empty()
        {
            return content;
        }
        if Instant::now() > deadline {
            let logs: String = logs
                .iter()
                .map(|log| format!("--- {} ---\n{}\n", log.display(), std::fs::read_to_string(log).unwrap_or_default()))
                .collect();
            panic!("{what} never arrived at {}\n{logs}", path.display());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A tracing writer that collects everything into one buffer.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Captured {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().expect("capture lock")).into_owned()
    }
}

impl Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("capture lock").extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Captured {
    type Writer = Captured;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}
