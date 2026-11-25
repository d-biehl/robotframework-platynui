//! Toplevel-with-popup fixture used by the compositor's integration tests.
//!
//! The egui test app cannot create `xdg_popups` (its menus render in-window),
//! so the popup tests speak the Wayland protocol directly through the shared
//! raw client ([`client`]): map a toplevel, then an `xdg_popup` anchored inside
//! it, and keep the connection alive while the test queries the control socket.
//!
//! Shared by `ipc_tests.rs` and `pidns_tests.rs` through `#[path]`; each of them
//! uses a different part of it, hence the blanket `dead_code` allowance.
#![allow(dead_code)]

use std::path::Path;

#[path = "client.rs"]
mod client;

pub const TOPLEVEL_SIZE: (i32, i32) = (300, 200);
/// Global offset of the popup's top-left inside the toplevel.
pub const POPUP_OFFSET: (i32, i32) = (31, 41);
pub const POPUP_SIZE: (i32, i32) = (100, 80);

/// A connected client holding a mapped toplevel and one mapped popup.
/// Dropping it closes the connection (and thereby dismisses everything).
pub struct Fixture {
    client: client::Client,
    toplevel: client::Toplevel,
    popup: client::Popup,
}

impl Fixture {
    /// Destroy the popup and flush, so the compositor drops it from its
    /// popup tree while toplevel and connection stay alive.
    pub fn dismiss_popup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.popup.role.destroy();
        self.popup.xdg_surface.destroy();
        self.popup.surface.destroy();
        self.client.roundtrip()
    }
}

/// Connect to the compositor socket, map a [`TOPLEVEL_SIZE`] toplevel and
/// a [`POPUP_SIZE`] popup at [`POPUP_OFFSET`] inside it.
pub fn open_toplevel_with_popup(
    runtime_dir: &Path,
    socket_name: &str,
    app_id: &str,
) -> Result<Fixture, Box<dyn std::error::Error>> {
    let mut client = client::Client::connect(&runtime_dir.join(socket_name))?;
    let toplevel = client.toplevel(app_id, TOPLEVEL_SIZE, 0xFF80_8080)?;
    let popup = client.popup(&toplevel, POPUP_OFFSET, POPUP_SIZE, 0xFF40_4040)?;
    Ok(Fixture { client, toplevel, popup })
}
