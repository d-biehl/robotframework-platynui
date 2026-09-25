// Must precede the `cfg`: a false crate-level `cfg` drops every attribute after
// it, and the crate left empty on other targets still sees every dependency.
#![allow(unused_crate_dependencies)]
// This test only applies to Linux (Wayland, PID namespaces).
#![cfg(target_os = "linux")]

//! PID-namespace integration tests for Wayland compositor identification.
//!
//! The sidecar deployment: the `PlatynUI` compositor and its application run in
//! one PID namespace, the runtime in a sibling one that reaches the Wayland
//! socket, the control socket and the accessibility bus by path. From there the
//! compositor's PID reads as 0, so identification cannot rest on the peer
//! process. These tests run the CLI in exactly that shape through
//! `scripts/wayland-sidecar-harness.sh`:
//!
//! - **The sidecar** — the compositor is identified, and a top-level window's
//!   bounds are the compositor's own geometry for it, including its position,
//!   with no substitution warning in the log.
//! - **Identified but unusable** — the session environment marks a `PlatynUI`
//!   session while the control socket points at a path nothing serves: every
//!   gated operation fails naming that path, never as an unrecognised
//!   compositor, and a top-level keeps its toolkit geometry with exactly one
//!   warning naming the window and the path.
//!
//! The tests are `#[ignore]`d because unprivileged user namespaces are not
//! available everywhere (`AppArmor` blocks them on stock Ubuntu 24.04). Run them
//! with:
//!
//! ```sh
//! just test-wayland-pidns
//! ```
//!
//! A missing prerequisite — `unshare` or unprivileged user namespaces, a binary
//! that is not built, a compositor or bus that never comes up — **fails** the
//! run with a message naming it. It never skips: an ignored test only runs when
//! someone asks for exactly this coverage, so a skip would read as a pass in the
//! one run meant to exercise it. This follows
//! `crates/java-agent/tests/live_fixture.rs`.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// The title the harness gives the egui test app's window.
const APP_TITLE: &str = "Sidecar App";

/// The provider's warning for a top-level whose bounds are the toolkit's own
/// geometry because the window manager could not answer.
const SUBSTITUTION_WARNING: &str = "window manager cannot answer for this top-level window";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("repository root")
}

/// A built binary under `target/debug`, or a failure naming how to build it.
fn binary(name: &str, build: &str) -> PathBuf {
    let path = repo().join("target/debug").join(name);
    assert!(path.is_file(), "prerequisite missing: {} is not built; run `{build}`", path.display());
    path
}

fn cli() -> PathBuf {
    binary("platynui-cli-rs", "cargo build -p platynui-cli --bin platynui-cli-rs")
}

fn ctl() -> PathBuf {
    binary("platynui-wayland-compositor-ctl", "cargo build -p platynui-wayland-compositor-ctl")
}

/// One command run in the sibling namespace.
struct Step {
    code: i32,
    stdout: String,
    stderr: String,
}

impl std::fmt::Display for Step {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "exit {}\n--- stdout\n{}--- stderr\n{}", self.code, self.stdout, self.stderr)
    }
}

/// Start the compositor and the test app in one PID namespace and run `steps`
/// — `(name, shell command)` pairs — one after another in a sibling one, with
/// `env` added to the sibling's environment. Each command sees `$CLI`, `$CTL`
/// and `$OUT`, a directory it may write files to.
fn run_in_sidecar(env: &[(&str, &str)], steps: &[(&str, &str)]) -> HashMap<String, Step> {
    let out = std::env::temp_dir().join(format!("platynui-wayland-pidns-{}-{}", std::process::id(), steps.len()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("output directory");

    let mut script = String::new();
    for (name, command) in steps {
        let _ = writeln!(script, r#"{command} >"$OUT/{name}.out" 2>"$OUT/{name}.err"; echo $? >"$OUT/{name}.code""#);
    }

    let mut harness = Command::new(repo().join("scripts/wayland-sidecar-harness.sh"));
    harness.arg("--with-app");
    let variables = [
        ("CLI", cli().display().to_string()),
        ("CTL", ctl().display().to_string()),
        ("OUT", out.display().to_string()),
    ];
    for (key, value) in variables.iter().map(|(k, v)| (*k, v.as_str())).chain(env.iter().copied()) {
        harness.arg("--env").arg(format!("{key}={value}"));
    }
    harness.args(["--", "bash", "-c", &script]);

    let output = harness.output().expect("run scripts/wayland-sidecar-harness.sh");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the sidecar harness failed ({}); a missing prerequisite is named here:\n{stderr}",
        output.status
    );

    let read = |name: &str, ext: &str| std::fs::read_to_string(out.join(format!("{name}.{ext}"))).unwrap_or_default();
    let steps = steps
        .iter()
        .map(|(name, _)| {
            let code = read(name, "code").trim().parse().unwrap_or(-1);
            (name.to_string(), Step { code, stdout: read(name, "out"), stderr: read(name, "err") })
        })
        .collect();
    let _ = std::fs::remove_dir_all(&out);
    steps
}

/// The `@Bounds` of the app's top-level window from a `query --format json`.
fn frame_bounds(step: &Step) -> (f64, f64, f64, f64) {
    let items: Value = serde_json::from_str(&step.stdout).unwrap_or_else(|err| panic!("query output: {err}\n{step}"));
    let frame = items
        .as_array()
        .and_then(|items| items.iter().find(|item| item["owner_name"] == APP_TITLE))
        .unwrap_or_else(|| panic!("no @Bounds for `{APP_TITLE}`\n{step}"));
    let value = &frame["value"];
    let field = |name: &str| value[name].as_f64().unwrap_or_else(|| panic!("@Bounds.{name}\n{step}"));
    (field("x"), field("y"), field("width"), field("height"))
}

/// The compositor's own content geometry for the app's window, from
/// `platynui-wayland-compositor-ctl -j list-windows`.
fn compositor_geometry(step: &Step) -> (f64, f64, f64, f64) {
    let status: Value = serde_json::from_str(&step.stdout).unwrap_or_else(|err| panic!("list-windows: {err}\n{step}"));
    let window = status["windows"]
        .as_array()
        .and_then(|windows| windows.iter().find(|window| window["title"] == APP_TITLE))
        .unwrap_or_else(|| panic!("the compositor lists no `{APP_TITLE}`\n{step}"));
    let field = |name: &str| window[name].as_f64().unwrap_or_else(|| panic!("{name}\n{step}"));
    (field("content_x"), field("content_y"), field("content_width"), field("content_height"))
}

const QUERY_BOUNDS: &str = r#""$CLI" query --format json "//control:Frame/@Bounds""#;

/// Spec: *A runtime in a sibling PID namespace identifies the compositor*, and
/// *A usable window manager reports the same bounds as before, without a
/// warning*. Measured before the fix: `Wayland Desktop (Unknown)`, and the
/// Frame at `{0,0,600,500}` against the compositor's `{10,40,600,500}`.
#[test]
#[ignore = "needs unprivileged user namespaces; run with `just test-wayland-pidns`"]
fn a_sidecar_identifies_the_compositor_and_reads_window_positions() {
    let steps = run_in_sidecar(
        &[],
        &[
            ("info", r#""$CLI" --log-level debug info"#),
            ("bounds", QUERY_BOUNDS),
            ("windows", r#""$CTL" -s "$PLATYNUI_CONTROL_SOCKET" -j list-windows"#),
        ],
    );

    let info = &steps["info"];
    assert_eq!(info.code, 0, "{info}");
    assert!(info.stdout.contains("Wayland Desktop (PlatynUI)"), "the compositor is identified\n{info}");
    let records = info.stderr.lines().filter(|line| line.contains("Wayland compositor identification")).count();
    assert_eq!(records, 1, "exactly one identification record\n{info}");
    assert!(info.stderr.contains("basis=handshake"), "the handshake decided\n{info}");

    let bounds = &steps["bounds"];
    assert_eq!(bounds.code, 0, "{bounds}");
    assert_eq!(
        frame_bounds(bounds),
        compositor_geometry(&steps["windows"]),
        "a top-level's bounds are the compositor's geometry, position included\n{bounds}"
    );
    assert!(!bounds.stderr.contains(SUBSTITUTION_WARNING), "no substitution warning\n{bounds}");
}

/// Spec: *An identified session with an unusable control channel fails by
/// name*, and *A top-level window whose window manager is unusable keeps its
/// fallback bounds and is reported*. Measured before the fix: the highlight
/// reported success, the other operations blamed an unknown compositor.
#[test]
#[ignore = "needs unprivileged user namespaces; run with `just test-wayland-pidns`"]
fn an_identified_session_with_an_unusable_control_socket_fails_by_name() {
    let dead = std::env::temp_dir().join(format!("platynui-pidns-nothing-listens-{}.control", std::process::id()));
    let _ = std::fs::remove_file(&dead);
    let dead = dead.display().to_string();

    let steps = run_in_sidecar(
        &[("XDG_CURRENT_DESKTOP", "platynui"), ("PLATYNUI_CONTROL_SOCKET", &dead)],
        &[
            ("info", r#""$CLI" --log-level debug info"#),
            ("window", r#""$CLI" window --activate "//control:Frame""#),
            ("screenshot", r#""$CLI" screenshot "$OUT/shot.png""#),
            ("highlight", r#""$CLI" highlight --rect 10,10,50,50 --duration-ms 100"#),
            ("bounds", QUERY_BOUNDS),
            ("bounds_again", QUERY_BOUNDS),
        ],
    );

    let info = &steps["info"];
    assert!(info.stdout.contains("Wayland Desktop (PlatynUI)"), "the environment identifies the session\n{info}");
    assert!(info.stderr.contains("basis=environment"), "{info}");
    assert!(info.stderr.contains(&dead), "the identification record names the control socket\n{info}");

    // `window` reports per-window results and exits 0 either way; the others
    // fail the command.
    let window = &steps["window"];
    assert!(window.stdout.contains("Failed:") && !window.stdout.contains("applied"), "`window` must fail\n{window}");
    for name in ["screenshot", "highlight"] {
        assert_ne!(steps[name].code, 0, "`{name}` must fail\n{}", steps[name]);
    }
    for name in ["window", "screenshot", "highlight"] {
        let step = &steps[name];
        let output = format!("{}{}", step.stdout, step.stderr);
        assert!(output.contains(&dead), "`{name}` names the control socket\n{step}");
        for claim in ["(Unknown)", "compositor Unknown", "undetected", "no backend implemented", "Highlighted", "Saved"]
        {
            assert!(!output.contains(claim), "`{name}` must not say `{claim}`\n{step}");
        }
    }

    for name in ["bounds", "bounds_again"] {
        let step = &steps[name];
        assert_eq!(step.code, 0, "the read still succeeds\n{step}");
        assert_eq!(frame_bounds(step), (0.0, 0.0, 600.0, 500.0), "with the toolkit's rectangle\n{step}");
    }
    // One CLI invocation is one runtime; each reports its own substitutions once.
    let bounds = &steps["bounds"];
    let warnings: Vec<_> = bounds.stderr.lines().filter(|line| line.contains(SUBSTITUTION_WARNING)).collect();
    assert_eq!(warnings.len(), 1, "exactly one substitution warning\n{bounds}");
    assert!(
        warnings[0].contains(APP_TITLE) && warnings[0].contains(&dead),
        "it names the window and the socket\n{bounds}"
    );
}
