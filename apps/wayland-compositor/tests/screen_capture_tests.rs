#![allow(unused_crate_dependencies)]

//! Screen capture through `ext-image-capture-source-v1` /
//! `ext-image-copy-capture-v1`.
//!
//! Spec: `compositor-screen-capture`. A raw client maps solid-colored
//! toplevels and captures outputs and toplevels into `wl_shm` buffers it can
//! read back.

#![cfg(target_os = "linux")]

#[path = "shared/client.rs"]
mod client;
#[path = "shared/harness.rs"]
mod harness;

use client::{Client, FrameOutcome, ShmBuffer};
use harness::{Compositor, OUTPUT_SIZE, OUTPUT_SIZE_U32, Options};
use serde_json::{Value, json};
use wayland_client::WEnum;
use wayland_client::protocol::wl_shm;
use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_frame_v1::FailureReason;

const RED: u32 = 0xFFC0_2020;
const BLUE: u32 = 0xFF20_40C0;

fn connect(compositor: &Compositor) -> Client {
    Client::connect(&compositor.wayland_socket())
        .unwrap_or_else(|err| panic!("cannot connect: {err}\n{}", compositor.logs()))
}

fn ok<T>(compositor: &Compositor, result: Result<T, client::BoxError>) -> T {
    result.unwrap_or_else(|err| panic!("{err}\n{}", compositor.logs()))
}

fn window(compositor: &Compositor, app_id: &str) -> Value {
    compositor.windows().into_iter().find(|w| w["app_id"] == app_id).unwrap_or_else(|| panic!("no window {app_id}"))
}

/// Map a solid-colored toplevel and put its frame at (x, y); returns the
/// toplevel and its content origin.
fn placed(
    compositor: &Compositor,
    client: &mut Client,
    app_id: &str,
    color: u32,
    x: i32,
    y: i32,
) -> (client::Toplevel, (i32, i32)) {
    let toplevel = ok(compositor, client.toplevel(app_id, (300, 200), color));
    compositor.wait_for_window(app_id);
    compositor.move_window(app_id, x, y);
    let w = window(compositor, app_id);
    let origin = (
        i32::try_from(w["content_x"].as_i64().expect("content_x")).expect("x"),
        i32::try_from(w["content_y"].as_i64().expect("content_y")).expect("y"),
    );
    (toplevel, origin)
}

/// The RGB part of a pixel.
fn rgb(pixel: u32) -> u32 {
    pixel & 0x00FF_FFFF
}

fn assert_color(buffer: &ShmBuffer, x: i32, y: i32, color: u32, what: &str) {
    let pixel = buffer.pixel(x, y);
    assert_eq!(rgb(pixel), rgb(color), "{what}: pixel ({x}, {y}) is {pixel:#010x}, expected {color:#010x}");
}

fn first_output(compositor: &Compositor, client: &Client) -> wayland_client::protocol::wl_output::WlOutput {
    client.state.outputs.first().cloned().unwrap_or_else(|| panic!("no wl_output\n{}", compositor.logs()))
}

#[test]
fn a_full_output_capture_succeeds() {
    let compositor = Compositor::start("cap-output", &Options::default());
    let mut client = connect(&compositor);
    let (_toplevel, origin) = placed(&compositor, &mut client, "test.red", RED, 100, 100);
    let output = first_output(&compositor, &client);

    let source = ok(&compositor, client.output_source(&output));
    let session = ok(&compositor, client.session(&source, false));
    let info = client.session_info(&session);
    assert_eq!(info.buffer_size, Some(OUTPUT_SIZE_U32), "announced size");
    assert!(info.shm_formats.contains(&wl_shm::Format::Argb8888), "ARGB8888 not announced: {:?}", info.shm_formats);
    assert!(info.shm_formats.contains(&wl_shm::Format::Xrgb8888), "XRGB8888 not announced: {:?}", info.shm_formats);

    let buffer = ok(&compositor, client.buffer(OUTPUT_SIZE, wl_shm::Format::Argb8888, 0));
    let frame = ok(&compositor, client.capture(&session, &buffer));
    assert_eq!(frame.outcome, FrameOutcome::Ready, "{}", compositor.logs());
    assert!(frame.transform && frame.damage && frame.presentation_time, "frame metadata missing: {frame:?}");
    assert_color(&buffer, origin.0 + 150, origin.1 + 100, RED, "toplevel center");
    assert_color(&buffer, origin.0 + 5, origin.1 + 5, RED, "toplevel corner");
}

#[test]
fn a_scaled_output_announces_its_mode_size() {
    let compositor = Compositor::start("cap-scale", &Options { scale: Some(2.0), ..Options::default() });
    let mut client = connect(&compositor);
    let output = first_output(&compositor, &client);
    let status = compositor.status();
    let mode = &status["outputs"][0];
    let expected = (
        u32::try_from(mode["width"].as_i64().expect("width")).expect("w"),
        u32::try_from(mode["height"].as_i64().expect("height")).expect("h"),
    );

    let source = ok(&compositor, client.output_source(&output));
    let session = ok(&compositor, client.session(&source, false));
    assert_eq!(client.session_info(&session).buffer_size, Some(expected), "status: {status}");
}

#[test]
fn a_covered_toplevel_is_captured_without_what_covers_it() {
    let compositor = Compositor::start("cap-toplevel", &Options::default());
    let mut client = connect(&compositor);
    ok(&compositor, client.bind_toplevel_list());
    let (_a, _) = placed(&compositor, &mut client, "test.a", RED, 100, 100);
    let (_b, _) = placed(&compositor, &mut client, "test.b", BLUE, 250, 100);
    compositor.command(&json!({"command": "focus_window", "app_id": "test.b"}));

    let handle = ok(&compositor, client.handle_for("test.a"));
    let source = ok(&compositor, client.toplevel_source(&handle));
    let session = ok(&compositor, client.session(&source, false));
    assert_eq!(client.session_info(&session).buffer_size, Some((300, 200)), "toplevel buffer size");

    let buffer = ok(&compositor, client.buffer((300, 200), wl_shm::Format::Argb8888, 0));
    let frame = ok(&compositor, client.capture(&session, &buffer));
    assert_eq!(frame.outcome, FrameOutcome::Ready, "{}", compositor.logs());
    for (x, y) in [(2, 2), (150, 100), (200, 100), (297, 197), (297, 2)] {
        assert_color(&buffer, x, y, RED, "captured toplevel");
    }
}

#[test]
fn a_wrong_sized_buffer_is_rejected_and_the_session_recovers() {
    let compositor = Compositor::start("cap-wrong", &Options::default());
    let mut client = connect(&compositor);
    let output = first_output(&compositor, &client);
    let source = ok(&compositor, client.output_source(&output));
    let session = ok(&compositor, client.session(&source, false));

    let small = ok(&compositor, client.buffer((640, 480), wl_shm::Format::Argb8888, 0));
    let frame = ok(&compositor, client.capture(&session, &small));
    assert_eq!(frame.outcome, FrameOutcome::Failed(WEnum::Value(FailureReason::BufferConstraints)));

    let right = ok(&compositor, client.buffer(OUTPUT_SIZE, wl_shm::Format::Argb8888, 0));
    let frame = ok(&compositor, client.capture(&session, &right));
    assert_eq!(frame.outcome, FrameOutcome::Ready, "{}", compositor.logs());
}

#[test]
fn the_cursor_appears_only_when_asked_for() {
    let compositor = Compositor::start("cap-cursor", &Options::default());
    let mut client = connect(&compositor);
    let (_toplevel, origin) = placed(&compositor, &mut client, "test.red", RED, 100, 100);
    let pointer = (origin.0 + 100, origin.1 + 80);
    compositor.pointer_move_to(f64::from(pointer.0), f64::from(pointer.1));
    let output = first_output(&compositor, &client);

    // The cursor image is drawn near the pointer (hotspot at or close to its
    // top-left); look at a 24×24 box from there.
    let differs_near_pointer = |buffer: &ShmBuffer| {
        (0..24).any(|dy| (0..24).any(|dx| rgb(buffer.pixel(pointer.0 + dx, pointer.1 + dy)) != rgb(RED)))
    };

    let source = ok(&compositor, client.output_source(&output));
    let with_cursor = ok(&compositor, client.session(&source, true));
    let buffer = ok(&compositor, client.buffer(OUTPUT_SIZE, wl_shm::Format::Argb8888, 0));
    assert_eq!(ok(&compositor, client.capture(&with_cursor, &buffer)).outcome, FrameOutcome::Ready);
    assert!(differs_near_pointer(&buffer), "no cursor near the pointer in the paint_cursors frame");

    let without_cursor = ok(&compositor, client.session(&source, false));
    let buffer = ok(&compositor, client.buffer(OUTPUT_SIZE, wl_shm::Format::Argb8888, 0));
    assert_eq!(ok(&compositor, client.capture(&without_cursor, &buffer)).outcome, FrameOutcome::Ready);
    assert!(!differs_near_pointer(&buffer), "a cursor was drawn without paint_cursors");
}

#[test]
fn a_cursor_session_reports_the_cursor_image_and_hotspot() {
    let compositor = Compositor::start("cap-cursorsess", &Options::default());
    let mut client = connect(&compositor);
    compositor.pointer_move_to(200.0, 200.0);
    let output = first_output(&compositor, &client);

    let source = ok(&compositor, client.output_source(&output));
    let cursor = ok(&compositor, client.cursor_session(&source));
    let session = ok(&compositor, client.cursor_capture_session(&cursor));
    ok(
        &compositor,
        client.dispatch_until("cursor enter and hotspot", |s| {
            s.cursor_sessions.get(&wayland_client::Proxy::id(&cursor)).is_some_and(|c| c.entered && c.hotspot.is_some())
        }),
    );

    let (w, h) = client.session_info(&session).buffer_size.expect("cursor size");
    let buffer = ok(
        &compositor,
        client.buffer((i32::try_from(w).expect("w"), i32::try_from(h).expect("h")), wl_shm::Format::Argb8888, 0),
    );
    let frame = ok(&compositor, client.capture(&session, &buffer));
    assert_eq!(frame.outcome, FrameOutcome::Ready, "{}", compositor.logs());
    assert!(client.cursor_session_info(&cursor).hotspot.is_some(), "no hotspot");
}

#[test]
fn a_second_pending_frame_is_a_protocol_error() {
    use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_session_v1;

    let compositor = Compositor::start("cap-dupframe", &Options::default());
    let mut client = connect(&compositor);
    let output = first_output(&compositor, &client);
    let source = ok(&compositor, client.output_source(&output));
    let session = ok(&compositor, client.session(&source, false));

    let _first = session.create_frame(&client.qh, ());
    let _second = session.create_frame(&client.qh, ());
    assert!(client.roundtrip().is_err(), "a second pending frame was accepted");
    let error = client.protocol_error().expect("a protocol error");
    assert_eq!(error.object_interface, "ext_image_copy_capture_session_v1", "{error:?}");
    assert_eq!(error.code, ext_image_copy_capture_session_v1::Error::DuplicateFrame as u32, "{error:?}");
    compositor.status();
}

#[test]
fn closing_the_captured_toplevel_stops_its_session() {
    let compositor = Compositor::start("cap-stopped", &Options::default());
    let mut client = connect(&compositor);
    ok(&compositor, client.bind_toplevel_list());
    let (a, _) = placed(&compositor, &mut client, "test.a", RED, 100, 100);
    let handle = ok(&compositor, client.handle_for("test.a"));
    let source = ok(&compositor, client.toplevel_source(&handle));
    let session = ok(&compositor, client.session(&source, false));

    a.role.destroy();
    a.xdg_surface.destroy();
    a.surface.destroy();
    let id = wayland_client::Proxy::id(&session);
    ok(&compositor, client.dispatch_until("session stopped", |s| s.sessions.get(&id).is_some_and(|i| i.stopped)));

    let buffer = ok(&compositor, client.buffer((300, 200), wl_shm::Format::Argb8888, 0));
    let frame = ok(&compositor, client.capture(&session, &buffer));
    // The reason is the protocol implementation's choice (smithay answers
    // `buffer_constraints` once a session has stopped).
    assert!(matches!(frame.outcome, FrameOutcome::Failed(_)), "a frame on a stopped session did not fail: {frame:?}");
    assert!(client.protocol_error().is_none(), "client was disconnected: {:?}", client.protocol_error());
}
