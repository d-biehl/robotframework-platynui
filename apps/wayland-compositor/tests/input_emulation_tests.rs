#![allow(unused_crate_dependencies)]

//! Emulated input from protocol clients: virtual pointer, virtual keyboard,
//! pointer warp and EIS.
//!
//! Spec: `compositor-input-emulation`. Each test starts a headless compositor,
//! drives it through a raw Wayland client (or an EI client) and observes the
//! result through the control socket and the events the client receives.

#![cfg(target_os = "linux")]

#[path = "shared/client.rs"]
mod client;
#[path = "shared/harness.rs"]
mod harness;

use client::{Client, PointerEvent};
use harness::{Compositor, OUTPUT_SIZE, OUTPUT_SIZE_U32, Options};
use serde_json::{Value, json};
use wayland_client::Proxy;
use wayland_client::protocol::wl_pointer;

/// `BTN_LEFT` (linux/input-event-codes.h).
const BTN_LEFT: u32 = 0x110;

const RED: u32 = 0xFFC0_2020;
const BLUE: u32 = 0xFF20_40C0;

fn connect(compositor: &Compositor) -> Client {
    Client::connect(&compositor.wayland_socket())
        .unwrap_or_else(|err| panic!("cannot connect: {err}\n{}", compositor.logs()))
}

/// Run a client step; on error fail with the compositor log.
fn ok<T>(compositor: &Compositor, result: Result<T, client::BoxError>) -> T {
    result.unwrap_or_else(|err| panic!("{err}\n{}", compositor.logs()))
}

fn window(compositor: &Compositor, app_id: &str) -> Value {
    compositor.windows().into_iter().find(|w| w["app_id"] == app_id).unwrap_or_else(|| panic!("no window {app_id}"))
}

fn is_focused(compositor: &Compositor, app_id: &str) -> bool {
    compositor.windows().iter().any(|w| w["app_id"] == app_id && w["focused"] == true)
}

/// A non-negative logical coordinate as a protocol `uint`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_u32(value: f64) -> u32 {
    assert!(value >= 0.0, "negative coordinate {value}");
    value.round() as u32
}

/// Global center of a window's client content.
fn content_center(window: &Value) -> (f64, f64) {
    let f = |k: &str| window[k].as_f64().unwrap_or_else(|| panic!("window lacks {k}: {window}"));
    (f("content_x") + f("content_width") / 2.0, f("content_y") + f("content_height") / 2.0)
}

/// Two non-overlapping toplevels A (left) and B (right), with B focused.
fn two_windows(compositor: &Compositor, client: &mut Client) -> (client::Toplevel, client::Toplevel) {
    let a = ok(compositor, client.toplevel("test.a", (300, 200), RED));
    let b = ok(compositor, client.toplevel("test.b", (300, 200), BLUE));
    compositor.wait_for_window("test.a");
    compositor.wait_for_window("test.b");
    compositor.move_window("test.a", 100, 100);
    compositor.move_window("test.b", 700, 100);
    compositor.command(&json!({"command": "focus_window", "app_id": "test.b"}));
    compositor.eventually("B focused", |c| is_focused(c, "test.b"));
    (a, b)
}

/// The pointer received an enter on `surface`, followed by a press and a
/// release of `button`.
fn clicked_on(
    state: &client::ClientState,
    surface: &wayland_client::protocol::wl_surface::WlSurface,
    button: u32,
) -> bool {
    let id = surface.id();
    let Some(enter) =
        state.pointer_events.iter().rposition(|e| matches!(e, PointerEvent::Enter { surface, .. } if *surface == id))
    else {
        return false;
    };
    let after = &state.pointer_events[enter..];
    let press =
        after.iter().position(|e| matches!(e, PointerEvent::Button { button: b, pressed: true, .. } if *b == button));
    let release =
        after.iter().position(|e| matches!(e, PointerEvent::Button { button: b, pressed: false, .. } if *b == button));
    matches!((press, release), (Some(p), Some(r)) if p < r)
}

// ─── Virtual pointer ────────────────────────────────────────────────────

#[test]
fn a_virtual_pointer_click_activates_the_window_under_it() {
    let compositor = Compositor::start("vp-click", &Options::default());
    let mut client = connect(&compositor);
    let (a, _b) = two_windows(&compositor, &mut client);
    let (x, y) = content_center(&window(&compositor, "test.a"));

    let pointer = ok(&compositor, client.virtual_pointer(None));
    pointer.motion_absolute(0, to_u32(x), to_u32(y), OUTPUT_SIZE_U32.0, OUTPUT_SIZE_U32.1);
    pointer.frame();
    pointer.button(1, BTN_LEFT, wl_pointer::ButtonState::Pressed);
    pointer.frame();
    pointer.button(2, BTN_LEFT, wl_pointer::ButtonState::Released);
    pointer.frame();
    ok(&compositor, client.roundtrip());

    compositor.eventually("A focused after the virtual-pointer click", |c| is_focused(c, "test.a"));
    ok(&compositor, client.dispatch_until("press and release on A", |s| clicked_on(s, &a.surface, BTN_LEFT)));
}

#[test]
fn relative_motion_past_the_right_edge_stops_at_the_edge() {
    let compositor = Compositor::start("vp-edge", &Options::default());
    let mut client = connect(&compositor);
    compositor.pointer_move_to(1270.0, 100.0);

    let pointer = ok(&compositor, client.virtual_pointer(None));
    pointer.motion(0, 50.0, 0.0);
    pointer.frame();
    ok(&compositor, client.roundtrip());

    let (x, y) = compositor.pointer_position();
    assert!(x <= f64::from(OUTPUT_SIZE.0 - 1), "pointer left the output: x = {x}");
    assert!(x >= 1270.0, "pointer did not move right: x = {x}");
    assert!((y - 100.0).abs() < f64::EPSILON, "y changed: {y}");
}

#[test]
fn an_output_bound_virtual_pointer_reaches_that_outputs_center() {
    let compositor = Compositor::start("vp-output", &Options { outputs: Some(2), ..Options::default() });
    let mut client = connect(&compositor);
    let second = ok(&compositor, client.output_at((OUTPUT_SIZE.0, 0)));

    let pointer = ok(&compositor, client.virtual_pointer(Some(&second)));
    pointer.motion_absolute(0, 500, 500, 1000, 1000);
    pointer.frame();
    ok(&compositor, client.roundtrip());

    let (x, y) = compositor.pointer_position();
    assert!((x - 1920.0).abs() < 1.0 && (y - 360.0).abs() < 1.0, "pointer at ({x}, {y}), expected (1920, 360)");
}

#[test]
fn zero_extent_is_ignored() {
    let compositor = Compositor::start("vp-zero", &Options::default());
    let mut client = connect(&compositor);
    compositor.pointer_move_to(10.0, 10.0);

    let pointer = ok(&compositor, client.virtual_pointer(None));
    pointer.motion_absolute(0, 500, 500, 0, 1000);
    pointer.frame();
    ok(&compositor, client.roundtrip());

    let (x, y) = compositor.pointer_position();
    assert!((x - 10.0).abs() < f64::EPSILON && (y - 10.0).abs() < f64::EPSILON, "pointer moved to ({x}, {y})");
    assert!(client.protocol_error().is_none(), "client was disconnected: {:?}", client.protocol_error());
    ok(&compositor, client.roundtrip());
}

// ─── Virtual keyboard ───────────────────────────────────────────────────

/// `KEY_SEMICOLON`: `;` on the US layout, `ö` on the German one.
const KEY_SEMICOLON: u32 = 39;

/// `ControlMask` in the modifier masks of the standard keymaps.
const CONTROL_MASK: u32 = 1 << 2;

/// A German XKB keymap, serialized as text.
fn german_keymap() -> String {
    use smithay::input::keyboard::xkb;
    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    let keymap = xkb::Keymap::new_from_names(&context, "", "", "de", "", None, xkb::KEYMAP_COMPILE_NO_FLAGS)
        .expect("compile the German keymap");
    keymap.get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1)
}

/// Whether a keymap puts `ö` on [`KEY_SEMICOLON`] — true for German, false
/// for US. Asked through xkbcommon, since the text form spells keysyms in
/// whatever way the installed libxkbcommon prefers.
fn is_german(keymap: &str) -> bool {
    use smithay::input::keyboard::xkb;
    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    let Some(keymap) = xkb::Keymap::new_from_string(
        &context,
        keymap.to_string(),
        xkb::KEYMAP_FORMAT_TEXT_V1,
        xkb::KEYMAP_COMPILE_NO_FLAGS,
    ) else {
        return false;
    };
    // xkb keycodes are evdev codes + 8.
    xkb::State::new(&keymap).key_get_one_sym(xkb::Keycode::new(KEY_SEMICOLON + 8)).raw() == 0x00f6
}

/// Map a toplevel and wait until it holds the keyboard focus.
fn focused_toplevel(compositor: &Compositor, client: &mut Client, app_id: &str) -> client::Toplevel {
    let toplevel = ok(compositor, client.toplevel(app_id, (300, 200), RED));
    compositor.wait_for_window(app_id);
    compositor.command(&json!({"command": "focus_window", "app_id": app_id}));
    let id = toplevel.surface.id();
    ok(
        compositor,
        client.dispatch_until("keyboard focus", |s| {
            s.keyboard_events.iter().any(|e| matches!(e, client::KeyboardEvent::Enter { surface } if *surface == id))
        }),
    );
    toplevel
}

/// The index of the `n`-th (0-based) press of `key` the client received.
fn nth_press(state: &client::ClientState, key: u32, n: usize) -> Option<usize> {
    state
        .keyboard_events
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(e, client::KeyboardEvent::Key { key: k, pressed: true } if *k == key))
        .nth(n)
        .map(|(i, _)| i)
}

/// The keymap the client had active at event index `index`.
fn keymap_before(state: &client::ClientState, index: usize) -> String {
    state.keyboard_events[..index]
        .iter()
        .rev()
        .find_map(|e| match e {
            client::KeyboardEvent::Keymap(k) => Some(k.clone()),
            _ => None,
        })
        .expect("a keymap before the key")
}

#[test]
fn a_custom_virtual_keyboard_keymap_is_honored() {
    let compositor = Compositor::start("vk-keymap", &Options::default());
    assert!(!is_german(&compositor.keymap()), "the compositor keymap must not be German for this test");
    let mut client = connect(&compositor);
    let _toplevel = focused_toplevel(&compositor, &mut client, "test.kb");

    let keyboard = ok(&compositor, client.virtual_keyboard(&german_keymap()));
    keyboard.key(0, KEY_SEMICOLON, 1);
    keyboard.key(1, KEY_SEMICOLON, 0);
    ok(&compositor, client.roundtrip());
    ok(&compositor, client.dispatch_until("the virtual key", |s| nth_press(s, KEY_SEMICOLON, 0).is_some()));

    compositor.key(KEY_SEMICOLON, true);
    compositor.key(KEY_SEMICOLON, false);
    ok(&compositor, client.dispatch_until("the injected key", |s| nth_press(s, KEY_SEMICOLON, 1).is_some()));

    let state = &client.state;
    let virtual_key = nth_press(state, KEY_SEMICOLON, 0).expect("virtual key");
    let injected_key = nth_press(state, KEY_SEMICOLON, 1).expect("injected key");
    assert!(
        is_german(&keymap_before(state, virtual_key)),
        "the virtual key arrived without the virtual keyboard's keymap"
    );
    assert!(
        !is_german(&keymap_before(state, injected_key)),
        "the injected key arrived with the virtual keyboard's keymap"
    );
}

/// A virtual keyboard is a seat device: while it holds Ctrl the seat reports
/// Ctrl (`compositor-modifier-state`), and once it is gone the compositor has
/// cleared what it left set.
#[test]
fn a_destroyed_virtual_keyboard_clears_its_modifiers() {
    let compositor = Compositor::start("vk-mods", &Options::default());
    let mut client = connect(&compositor);
    let _toplevel = focused_toplevel(&compositor, &mut client, "test.kb");

    let keyboard = ok(&compositor, client.virtual_keyboard(&compositor.keymap()));
    keyboard.modifiers(CONTROL_MASK, 0, 0, 0);
    ok(&compositor, client.roundtrip());
    ok(
        &compositor,
        client.dispatch_until("Ctrl from the virtual keyboard", |s| {
            s.keyboard_events.iter().any(
                |e| matches!(e, client::KeyboardEvent::Modifiers { depressed, .. } if depressed & CONTROL_MASK != 0),
            )
        }),
    );
    compositor.eventually("the seat reports Ctrl from the virtual keyboard", |c| c.modifiers()["ctrl"] == true);

    keyboard.destroy();
    ok(&compositor, client.roundtrip());
    compositor.eventually("Ctrl released after the virtual keyboard is gone", |c| c.modifiers()["ctrl"] == false);
}

// ─── EIS ────────────────────────────────────────────────────────────────

#[path = "shared/ei_client.rs"]
mod ei_client;

use ei_client::EiClient;
use reis::ei::handshake::ContextType;
use reis::enumflags2::BitFlags;
use reis::event::DeviceCapability;

/// `KEY_A` (linux/input-event-codes.h).
const KEY_A: u32 = 30;

fn with_eis() -> Options {
    Options { eis: true, ..Options::default() }
}

fn ei_sender(compositor: &Compositor, wanted: BitFlags<DeviceCapability>) -> EiClient {
    EiClient::sender(&compositor.eis_socket(), wanted)
        .unwrap_or_else(|err| panic!("EI sender setup failed: {err}\n{}", compositor.logs()))
}

#[test]
fn the_child_process_finds_the_eis_socket() {
    let marker = tempfile::Builder::new().prefix("pui-eis-env-").tempdir().expect("temp dir");
    let file = marker.path().join("libei_socket");
    let script = format!("printf '%s' \"$LIBEI_SOCKET\" > '{}'", file.display());
    let compositor = Compositor::start(
        "eis-child",
        &Options { print_env: true, child: vec!["/bin/sh".into(), "-c".into(), script], ..with_eis() },
    );

    compositor
        .eventually("the child wrote LIBEI_SOCKET", |_| std::fs::read_to_string(&file).is_ok_and(|s| !s.is_empty()));
    let path = std::fs::read_to_string(&file).expect("marker");
    assert!(path.ends_with("eis-platynui"), "LIBEI_SOCKET = {path:?}");
    assert!(
        compositor.stdout().contains(&format!("LIBEI_SOCKET={path}")),
        "readiness output lacks LIBEI_SOCKET:\n{}",
        compositor.logs()
    );
    EiClient::connect(std::path::Path::new(&path), "platynui-compositor-tests", ContextType::Sender)
        .unwrap_or_else(|err| panic!("cannot connect to {path}: {err}\n{}", compositor.logs()));
}

#[test]
fn eis_can_be_switched_off() {
    let compositor = Compositor::start("eis-off", &Options { print_env: true, ..Options::default() });
    compositor.eventually("readiness output", |c| c.stdout().contains("WAYLAND_DISPLAY="));
    assert!(!compositor.eis_socket().exists(), "an EIS socket exists although --no-eis was given");
    assert!(
        !compositor.stdout().contains("LIBEI_SOCKET"),
        "readiness output names LIBEI_SOCKET:\n{}",
        compositor.stdout()
    );
}

#[test]
fn a_receiver_context_is_refused() {
    let compositor = Compositor::start("eis-recv", &with_eis());
    let mut client = EiClient::connect(&compositor.eis_socket(), "platynui-compositor-tests", ContextType::Receiver)
        .unwrap_or_else(|err| panic!("handshake failed: {err}\n{}", compositor.logs()));
    client.settle(std::time::Duration::from_secs(3)).expect("dispatch");
    assert!(client.disconnected, "a receiver context was not disconnected\n{}", compositor.logs());
    compositor.status();
}

#[test]
fn binding_every_capability_yields_every_device_except_text() {
    let compositor = Compositor::start("eis-caps", &Options { outputs: Some(2), ..with_eis() });
    let mut client = ei_sender(&compositor, DeviceCapability::PointerAbsolute.into());
    client.settle(std::time::Duration::from_millis(500)).expect("dispatch");

    let expected = DeviceCapability::Pointer
        | DeviceCapability::PointerAbsolute
        | DeviceCapability::Button
        | DeviceCapability::Scroll
        | DeviceCapability::Keyboard
        | DeviceCapability::Touch;
    assert_eq!(client.capabilities(), expected, "device capabilities");

    let absolute = client.device_with(DeviceCapability::PointerAbsolute).expect("absolute pointer");
    let mut regions: Vec<_> = absolute.regions().iter().map(|r| (r.x, r.y, r.width, r.height)).collect();
    regions.sort_unstable();
    let (w, h) = OUTPUT_SIZE_U32;
    assert_eq!(regions, vec![(0, 0, w, h), (w, 0, w, h)], "absolute-pointer regions");
}

#[test]
fn the_ei_keymap_matches_the_compositor_keymap() {
    let compositor = Compositor::start("eis-keymap", &Options { keyboard_layout: Some("de".into()), ..with_eis() });
    let client = ei_sender(&compositor, DeviceCapability::Keyboard.into());
    let ei_keymap = client.keymap().unwrap_or_else(|| panic!("the EI keyboard has no keymap\n{}", compositor.logs()));
    assert!(is_german(&compositor.keymap()), "the compositor keymap is not German");
    assert!(is_german(&ei_keymap), "the EI keyboard's keymap is not the compositor's German layout");
}

#[test]
fn a_key_typed_through_eis_reaches_the_focused_client() {
    let compositor = Compositor::start("eis-key", &with_eis());
    let mut client = connect(&compositor);
    let _toplevel = focused_toplevel(&compositor, &mut client, "test.kb");
    let mut ei = ei_sender(&compositor, DeviceCapability::Keyboard.into());

    ei.key(KEY_A, true).expect("press");
    ei.key(KEY_A, false).expect("release");
    ok(
        &compositor,
        client.dispatch_until("press and release of KEY_A", |s| {
            let press = nth_press(s, KEY_A, 0);
            press.is_some_and(|p| {
                s.keyboard_events[p..]
                    .iter()
                    .any(|e| matches!(e, client::KeyboardEvent::Key { key, pressed: false } if *key == KEY_A))
            })
        }),
    );
    let press = nth_press(&client.state, KEY_A, 0).expect("press");
    assert_eq!(
        keymap_before(&client.state, press),
        compositor.keymap(),
        "the key arrived with a keymap other than the compositor's"
    );
}

#[test]
fn an_eis_click_activates_the_window_under_it() {
    let compositor = Compositor::start("eis-click", &with_eis());
    let mut client = connect(&compositor);
    let (_a, _b) = two_windows(&compositor, &mut client);
    let (x, y) = content_center(&window(&compositor, "test.a"));
    let mut ei = ei_sender(&compositor, DeviceCapability::PointerAbsolute | DeviceCapability::Button);

    ei.move_to(x, y).expect("move");
    ei.button(BTN_LEFT, true).expect("press");
    ei.button(BTN_LEFT, false).expect("release");
    compositor.eventually("A focused after the EIS click", |c| is_focused(c, "test.a"));
}

#[test]
fn eis_absolute_motion_lands_on_the_requested_global_position() {
    let compositor = Compositor::start("eis-abs", &Options { outputs: Some(2), ..with_eis() });
    let mut ei = ei_sender(&compositor, DeviceCapability::PointerAbsolute.into());

    ei.move_to(1500.0, 200.0).expect("move");
    compositor.eventually("pointer at (1500, 200)", |c| {
        let (x, y) = c.pointer_position();
        (x - 1500.0).abs() < 1.0 && (y - 200.0).abs() < 1.0
    });
}

// ─── Pointer warp ───────────────────────────────────────────────────────

/// A toplevel at a known global position with the pointer over it; returns
/// the toplevel, its content origin and the enter serial.
fn hovered_toplevel(compositor: &Compositor, client: &mut Client, app_id: &str) -> (client::Toplevel, (f64, f64), u32) {
    let toplevel = ok(compositor, client.toplevel(app_id, (200, 200), RED));
    compositor.wait_for_window(app_id);
    compositor.move_window(app_id, 100, 100);
    let w = window(compositor, app_id);
    let origin = (w["content_x"].as_f64().expect("content_x"), w["content_y"].as_f64().expect("content_y"));
    compositor.pointer_move_to(origin.0 + 50.0, origin.1 + 50.0);
    let surface = toplevel.surface.clone();
    ok(compositor, client.dispatch_until("pointer enter", |s| s.pointer_is_over(&surface)));
    let serial = client.state.last_enter_serial(&toplevel.surface).expect("enter serial");
    (toplevel, origin, serial)
}

#[test]
fn a_warp_inside_the_focused_surface_moves_the_pointer() {
    let compositor = Compositor::start("warp-ok", &Options::default());
    let mut client = connect(&compositor);
    let (toplevel, origin, serial) = hovered_toplevel(&compositor, &mut client, "test.warp");
    let warp = ok(&compositor, client.pointer_warp());
    let pointer = client.state.pointer.clone().expect("wl_pointer");

    warp.warp_pointer(&toplevel.surface, &pointer, 20.0, 30.0, serial);
    ok(&compositor, client.roundtrip());

    let (x, y) = compositor.pointer_position();
    assert!(
        (x - (origin.0 + 20.0)).abs() < 1.0 && (y - (origin.1 + 30.0)).abs() < 1.0,
        "pointer at ({x}, {y}), expected ({}, {})",
        origin.0 + 20.0,
        origin.1 + 30.0
    );
    ok(
        &compositor,
        client.dispatch_until("motion at (20, 30)", |s| {
            s.pointer_events
                .iter()
                .any(|e| matches!(e, PointerEvent::Motion { x, y } if (x - 20.0).abs() < 1.0 && (y - 30.0).abs() < 1.0))
        }),
    );
}

fn pointer_at(compositor: &Compositor, expected: (f64, f64)) -> bool {
    let (x, y) = compositor.pointer_position();
    (x - expected.0).abs() < 1.0 && (y - expected.1).abs() < 1.0
}

#[test]
fn a_warp_with_a_stale_serial_is_ignored() {
    let compositor = Compositor::start("warp-stale", &Options::default());
    let mut client = connect(&compositor);
    let (toplevel, origin, first_serial) = hovered_toplevel(&compositor, &mut client, "test.warp");

    // Leave the surface and enter it again: the first enter serial is stale.
    compositor.pointer_move_to(origin.0 + 500.0, origin.1 + 300.0);
    let surface = toplevel.surface.clone();
    ok(&compositor, client.dispatch_until("pointer leave", |s| !s.pointer_is_over(&surface)));
    compositor.pointer_move_to(origin.0 + 50.0, origin.1 + 50.0);
    ok(&compositor, client.dispatch_until("pointer enter again", |s| s.pointer_is_over(&surface)));
    assert_ne!(
        client.state.last_enter_serial(&toplevel.surface),
        Some(first_serial),
        "the enter serial did not change"
    );

    let warp = ok(&compositor, client.pointer_warp());
    let pointer = client.state.pointer.clone().expect("wl_pointer");
    warp.warp_pointer(&toplevel.surface, &pointer, 20.0, 30.0, first_serial);
    ok(&compositor, client.roundtrip());

    assert!(pointer_at(&compositor, (origin.0 + 50.0, origin.1 + 50.0)), "a stale-serial warp moved the pointer");
    assert!(client.protocol_error().is_none(), "client was disconnected: {:?}", client.protocol_error());
}

#[test]
fn a_warp_outside_the_surface_or_to_an_unfocused_surface_is_ignored() {
    let compositor = Compositor::start("warp-ignored", &Options::default());
    let mut client = connect(&compositor);
    let toplevel = ok(&compositor, client.toplevel("test.warp", (200, 200), RED));
    compositor.wait_for_window("test.warp");
    compositor.move_window("test.warp", 100, 100);
    let w = window(&compositor, "test.warp");
    let origin = (w["content_x"].as_f64().expect("content_x"), w["content_y"].as_f64().expect("content_y"));
    let outside = (origin.0 + 600.0, origin.1 + 300.0);
    compositor.pointer_move_to(outside.0, outside.1);
    let warp = ok(&compositor, client.pointer_warp());
    let pointer = client.state.pointer.clone().expect("wl_pointer");

    // Unfocused: the pointer is not over the surface.
    warp.warp_pointer(&toplevel.surface, &pointer, 20.0, 30.0, 0);
    ok(&compositor, client.roundtrip());
    assert!(pointer_at(&compositor, outside), "a warp to an unfocused surface moved the pointer");

    // Focused, but outside the 200×200 surface.
    compositor.pointer_move_to(origin.0 + 50.0, origin.1 + 50.0);
    let surface = toplevel.surface.clone();
    ok(&compositor, client.dispatch_until("pointer enter", |s| s.pointer_is_over(&surface)));
    let serial = client.state.last_enter_serial(&toplevel.surface).expect("enter serial");
    warp.warp_pointer(&toplevel.surface, &pointer, 500.0, 500.0, serial);
    ok(&compositor, client.roundtrip());
    assert!(
        pointer_at(&compositor, (origin.0 + 50.0, origin.1 + 50.0)),
        "a warp outside the surface moved the pointer"
    );
    assert!(client.protocol_error().is_none(), "client was disconnected: {:?}", client.protocol_error());
}

#[test]
fn a_warp_inside_a_popup_lands_at_the_popups_global_position() {
    let compositor = Compositor::start("warp-popup", &Options::default());
    let mut client = connect(&compositor);
    let toplevel = ok(&compositor, client.toplevel("test.warp", (300, 200), RED));
    compositor.wait_for_window("test.warp");
    compositor.move_window("test.warp", 100, 100);
    let popup = ok(&compositor, client.popup(&toplevel, (40, 50), (120, 80), BLUE));
    let popups = compositor.command(&json!({"command": "list_popups"}));
    let rect = &popups["popups"][0];
    let popup_origin = (rect["x"].as_f64().expect("popup x"), rect["y"].as_f64().expect("popup y"));

    compositor.pointer_move_to(popup_origin.0 + 60.0, popup_origin.1 + 40.0);
    let surface = popup.surface.clone();
    ok(&compositor, client.dispatch_until("pointer enter on the popup", |s| s.pointer_is_over(&surface)));
    let serial = client.state.last_enter_serial(&popup.surface).expect("enter serial");

    let warp = ok(&compositor, client.pointer_warp());
    let pointer = client.state.pointer.clone().expect("wl_pointer");
    warp.warp_pointer(&popup.surface, &pointer, 10.0, 15.0, serial);
    ok(&compositor, client.roundtrip());

    let expected = (popup_origin.0 + 10.0, popup_origin.1 + 15.0);
    assert!(pointer_at(&compositor, expected), "pointer at {:?}, expected {expected:?}", compositor.pointer_position());
}

// ─── EIS: several clients, release on disconnect ───────────────────────

/// `KEY_LEFTSHIFT` / `KEY_LEFTCTRL` (linux/input-event-codes.h).
const KEY_LEFTSHIFT: u32 = 42;
const KEY_LEFTCTRL: u32 = 29;

#[test]
fn a_second_ei_client_does_not_displace_the_first() {
    let compositor = Compositor::start("eis-two", &with_eis());
    let mut a = ei_sender(&compositor, DeviceCapability::PointerAbsolute.into());
    let _b = ei_sender(&compositor, DeviceCapability::PointerAbsolute.into());
    a.settle(std::time::Duration::from_millis(300)).expect("dispatch");
    assert!(!a.disconnected, "the first EI client was disconnected by the second\n{}", compositor.logs());

    a.move_to(100.0, 100.0).expect("move");
    compositor.eventually("pointer at (100, 100)", |c| pointer_at(c, (100.0, 100.0)));
    a.settle(std::time::Duration::from_millis(100)).expect("dispatch");
    assert!(!a.disconnected, "the first EI client lost its connection");
}

#[test]
fn a_crashed_ei_client_leaves_no_key_pressed() {
    let compositor = Compositor::start("eis-crash", &with_eis());
    let mut client = connect(&compositor);
    let _toplevel = focused_toplevel(&compositor, &mut client, "test.kb");
    let mut ei = ei_sender(&compositor, DeviceCapability::Keyboard.into());

    ei.key(KEY_LEFTSHIFT, true).expect("press");
    compositor.eventually("Shift held through EIS", |c| c.modifiers()["shift"] == true);
    drop(ei); // disconnect without releasing

    compositor.eventually("Shift released after the EI client is gone", |c| c.modifiers()["shift"] == false);
    ok(
        &compositor,
        client.dispatch_until("the release of Shift", |s| {
            s.keyboard_events
                .iter()
                .any(|e| matches!(e, client::KeyboardEvent::Key { key, pressed: false } if *key == KEY_LEFTSHIFT))
        }),
    );
}

#[test]
fn another_devices_held_key_survives() {
    let compositor = Compositor::start("eis-survive", &with_eis());
    let mut client = connect(&compositor);
    let _toplevel = focused_toplevel(&compositor, &mut client, "test.kb");
    let mut ei = ei_sender(&compositor, DeviceCapability::Keyboard.into());

    compositor.key(KEY_LEFTSHIFT, true);
    ei.key(KEY_LEFTCTRL, true).expect("press");
    compositor.eventually("Shift and Ctrl held", |c| c.modifiers()["shift"] == true && c.modifiers()["ctrl"] == true);
    drop(ei);

    compositor.eventually("Ctrl released, Shift kept", |c| {
        let m = c.modifiers();
        m["ctrl"] == false && m["shift"] == true
    });
}
