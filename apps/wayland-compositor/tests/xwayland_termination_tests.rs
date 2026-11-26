#![allow(unused_crate_dependencies)]

//! `XWayland` terminating during the session (spec: *compositor-session-lifecycle*,
//! "`XWayland` terminating during the session is reported, not fatal"): the
//! compositor reports it, removes the server's X11 windows — mapped and minimized
//! — reports `XWayland` as unusable, and keeps serving Wayland clients.
//!
//! The test needs a real `Xwayland` on `PATH` and is therefore `#[ignore]`d. Run
//! it with:
//!
//! ```sh
//! cargo nextest run -p platynui-wayland-compositor --run-ignored ignored-only -E 'binary(xwayland_termination_tests)'
//! ```

// This entire test suite only applies to Linux (Wayland compositor).
#![cfg(target_os = "linux")]

#[path = "shared/client.rs"]
mod client;
#[path = "shared/harness.rs"]
mod harness;
#[path = "shared/x11.rs"]
mod x11;

use client::Client;
use harness::{Compositor, Options};
use serde_json::json;
use x11::X11;

/// The process ID of the `Xwayland` serving `display` (e.g. `:1`), from `/proc`.
fn xwayland_pid(display: &str) -> u32 {
    let mut found = Vec::new();
    for entry in std::fs::read_dir("/proc").expect("read /proc").flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else { continue };
        let Ok(cmdline) = std::fs::read(entry.path().join("cmdline")) else { continue };
        let mut args = cmdline.split(|byte| *byte == 0).map(String::from_utf8_lossy);
        let is_xwayland = args.next().is_some_and(|argv0| argv0.ends_with("Xwayland"));
        if is_xwayland && args.any(|arg| arg == display) {
            found.push(pid);
        }
    }
    assert_eq!(found.len(), 1, "expected exactly one Xwayland serving {display}, found {found:?}");
    found[0]
}

#[test]
#[ignore = "needs Xwayland on PATH; see the module documentation"]
fn wayland_clients_keep_working_after_xwayland_dies() {
    let compositor = Compositor::start("xw-dies", &Options { xwayland: true, ..Options::default() });
    let display = compositor.x11_display();
    let x11 = X11::connect(&display);
    x11.window("pui-x11-mapped", 0, 0, 200, 150, 0x00FF_0000, 0, false);
    x11.window("pui-x11-minimized", 300, 0, 200, 150, 0x0000_00FF, 0, false);
    compositor.wait_for_window_title("pui-x11-mapped");
    compositor.wait_for_window_title("pui-x11-minimized");
    compositor.command(&json!({"command": "minimize_window", "title": "pui-x11-minimized"}));

    let mut client = Client::connect(&compositor.wayland_socket()).expect("wayland client");
    let _toplevel = client.toplevel("pui-wayland", (200, 150), 0x0000_FF00).expect("wayland toplevel");
    compositor.wait_for_window("pui-wayland");
    let before = compositor.command(&json!({"command": "list_windows"}));
    assert!(before["minimized"].to_string().contains("pui-x11-minimized"), "precondition: {before}");

    let pid = xwayland_pid(&display);
    let killed = std::process::Command::new("/bin/kill").args(["-KILL", &pid.to_string()]).status().expect("kill");
    assert!(killed.success(), "could not kill Xwayland {pid}");

    compositor.eventually("XWayland reported as unusable", |c| c.status()["xwayland"] == json!(false));
    let after = compositor.command(&json!({"command": "list_windows"}));
    let listing = after.to_string();
    assert!(!listing.contains("pui-x11-"), "X11 windows survived their server: {after}");
    assert!(listing.contains("pui-wayland"), "the Wayland window is gone: {after}");
    assert!(
        compositor.logs().contains("XWayland terminated during the session"),
        "the compositor's own ERROR is missing\n{}",
        compositor.logs()
    );
    assert_eq!(compositor.status()["status"], json!("ok"), "the compositor stopped answering");
}
