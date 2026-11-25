#![allow(unused_crate_dependencies)]

//! Which protocol globals the restrictive policy hides.
//!
//! Spec: `compositor-input-emulation` — "The restrictive policy hides the
//! virtual-input globals"; `compositor-screen-capture` — "The restrictive
//! policy hides screen capture". A raw client lists the registry of a
//! compositor started with and without `--restrict-protocols`.

#![cfg(target_os = "linux")]

#[path = "shared/client.rs"]
mod client;
#[path = "shared/harness.rs"]
mod harness;

use client::Client;
use harness::{Compositor, Options};

const VIRTUAL_INPUT_GLOBALS: [&str; 3] =
    ["zwp_virtual_keyboard_manager_v1", "zwlr_virtual_pointer_manager_v1", "wp_pointer_warp_v1"];

const SCREEN_CAPTURE_GLOBALS: [&str; 3] = [
    "ext_output_image_capture_source_manager_v1",
    "ext_foreign_toplevel_image_capture_source_manager_v1",
    "ext_image_copy_capture_manager_v1",
];

/// The interface names a fresh client sees.
fn registry(compositor: &Compositor) -> Vec<String> {
    Client::connect(&compositor.wayland_socket())
        .unwrap_or_else(|err| panic!("cannot connect: {err}\n{}", compositor.logs()))
        .interfaces()
}

fn restricted() -> Options {
    Options { restrict_protocols: Some("org.example.trusted".into()), ..Options::default() }
}

#[test]
fn restricted_clients_see_no_virtual_input_globals() {
    let compositor = Compositor::start("pol-vi-r", &restricted());
    let interfaces = registry(&compositor);
    for global in VIRTUAL_INPUT_GLOBALS {
        assert!(
            !interfaces.iter().any(|i| i == global),
            "{global} is advertised under --restrict-protocols: {interfaces:?}"
        );
    }
}

#[test]
fn unrestricted_clients_see_the_virtual_input_globals() {
    let compositor = Compositor::start("pol-vi-u", &Options::default());
    let interfaces = registry(&compositor);
    for global in VIRTUAL_INPUT_GLOBALS {
        assert!(interfaces.iter().any(|i| i == global), "{global} is not advertised: {interfaces:?}");
    }
}

#[test]
fn restricted_clients_cannot_find_screen_capture() {
    let compositor = Compositor::start("pol-sc-r", &restricted());
    let interfaces = registry(&compositor);
    for global in SCREEN_CAPTURE_GLOBALS {
        assert!(
            !interfaces.iter().any(|i| i == global),
            "{global} is advertised under --restrict-protocols: {interfaces:?}"
        );
    }
}

#[test]
fn unrestricted_clients_find_screen_capture() {
    let compositor = Compositor::start("pol-sc-u", &Options::default());
    let interfaces = registry(&compositor);
    for global in SCREEN_CAPTURE_GLOBALS {
        assert!(interfaces.iter().any(|i| i == global), "{global} is not advertised: {interfaces:?}");
    }
}
