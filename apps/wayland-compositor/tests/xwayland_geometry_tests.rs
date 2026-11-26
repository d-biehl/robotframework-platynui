#![allow(unused_crate_dependencies)]

//! X11 window geometry under Xwayland: windows with client-side shadows
//! (`_GTK_FRAME_EXTENTS`, as GTK sets them) keep the position and size the X
//! server knows, and the compositor shows every X11 window where the X server
//! believes it is — so menus (override-redirect windows) open where their
//! toolkit computed them.
//!
//! These tests need a real `Xwayland` on `PATH` and are therefore `#[ignore]`d.
//! Run them with:
//!
//! ```sh
//! cargo nextest run -p platynui-wayland-compositor --run-ignored ignored-only -E 'binary(xwayland_geometry_tests)'
//! ```
//!
//! A missing `Xwayland` fails the run (the compositor never reports a display).

#![cfg(target_os = "linux")]

#[path = "shared/client.rs"]
mod client;
#[path = "shared/harness.rs"]
mod harness;
#[path = "shared/x11.rs"]
mod x11;

use std::time::{Duration, Instant};

use client::{Client, FrameOutcome, ShmBuffer};
use harness::{Compositor, OUTPUT_SIZE, Options};
use wayland_client::protocol::wl_shm;
use x11::X11;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConfigureWindowAux, ConnectionExt as _};

const RED: u32 = 0x00FF_0000;
const BLUE: u32 = 0x0000_00FF;
const SHADOW: u32 = 20;

fn capture(compositor: &Compositor, client: &mut Client) -> ShmBuffer {
    let output = client.state.outputs.first().cloned().expect("wl_output");
    let source = client.output_source(&output).expect("source");
    let session = client.session(&source, false).expect("session");
    let buffer = client.buffer(OUTPUT_SIZE, wl_shm::Format::Argb8888, 0).expect("buffer");
    let frame = client.capture(&session, &buffer).expect("capture");
    assert_eq!(frame.outcome, FrameOutcome::Ready, "{}", compositor.logs());
    buffer
}

fn rgb(buffer: &ShmBuffer, x: i32, y: i32) -> u32 {
    buffer.pixel(x, y) & 0x00FF_FFFF
}

/// Wait until the compositor shows `color` at (x, y).
fn wait_for_color(compositor: &Compositor, client: &mut Client, x: i32, y: i32, color: u32) -> ShmBuffer {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let buffer = capture(compositor, client);
        if rgb(&buffer, x, y) == color {
            return buffer;
        }
        assert!(
            Instant::now() < deadline,
            "({x}, {y}) shows {:#08x}, expected {color:#08x}\n{}",
            rgb(&buffer, x, y),
            compositor.logs()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn start(name: &str) -> (Compositor, X11, Client) {
    let compositor = Compositor::start(name, &Options { xwayland: true, ..Options::default() });
    let x11 = X11::connect(&compositor.x11_display());
    let client = Client::connect(&compositor.wayland_socket()).expect("wayland client");
    (compositor, x11, client)
}

#[test]
#[ignore = "needs Xwayland on PATH; see the module documentation"]
fn a_shadowed_x11_window_keeps_its_x11_size_and_shows_at_its_x11_position() {
    let (compositor, x11, mut client) = start("xw-shadow");
    let win = x11.window("pui-shadowed", 0, 0, 300, 200, RED, SHADOW, false);
    compositor.wait_for_window_title("pui-shadowed");

    let (x, y, w, h) = x11.wait_rect(win, "the mapped window", |(_, _, w, _)| w > 0);
    assert_eq!((w, h), (300, 200), "the compositor resized the window");

    // The whole X11 window, shadow included, is drawn at its X11 rectangle.
    // (The shadow may reach past the top-left screen edge, so check the
    // bottom-right corner.)
    let (right, bottom) = (x + i32::try_from(w).expect("w"), y + i32::try_from(h).expect("h"));
    let buffer = wait_for_color(&compositor, &mut client, right - 3, bottom - 3, RED);
    assert_ne!(rgb(&buffer, right + 2, bottom + 2), RED, "the window is drawn right/below its X11 rectangle");
    assert_eq!(rgb(&buffer, x + 30, y + 30), RED, "the window's visible area is not at its X11 position");
}

#[test]
#[ignore = "needs Xwayland on PATH; see the module documentation"]
fn a_shadowed_x11_window_keeps_its_position_when_it_resizes_itself() {
    let (compositor, x11, _client) = start("xw-resize");
    let win = x11.window("pui-resize", 0, 0, 300, 200, RED, SHADOW, false);
    compositor.wait_for_window_title("pui-resize");
    let (x, y, _, _) = x11.wait_rect(win, "the mapped window", |(_, _, w, _)| w > 0);

    x11.conn.configure_window(win, &ConfigureWindowAux::new().width(320).height(220)).expect("configure");
    x11.conn.flush().expect("flush");
    let (nx, ny, _, _) = x11.wait_rect(win, "the resize", |(_, _, w, h)| (w, h) == (320, 220));
    assert_eq!((nx, ny), (x, y), "the window moved when it resized itself");
}

#[test]
#[ignore = "needs Xwayland on PATH; see the module documentation"]
fn an_override_redirect_window_shows_where_it_placed_itself() {
    let (compositor, x11, mut client) = start("xw-menu");
    let _menu = x11.window("pui-menu", 400, 300, 100, 80, BLUE, 10, true);

    let buffer = wait_for_color(&compositor, &mut client, 402, 302, BLUE);
    assert_ne!(rgb(&buffer, 397, 297), BLUE, "the menu is drawn left/above where it placed itself");
    assert_eq!(rgb(&buffer, 497, 377), BLUE, "the menu's far corner is not where it placed itself");
}
