//! `xdg-decoration-unstable-v1` handler — server/client-side decoration negotiation.
//!
//! Prefers server-side decorations (SSD) so the compositor renders title bars
//! with close/maximize/minimize buttons.  If the client insists on CSD, we accept it.
//!
//! **Timing:** The `new_decoration` callback fires *after* `new_toplevel` has
//! already mapped the window and sent the initial configure.  That initial
//! configure does not include a decoration mode because the decoration object
//! did not exist yet.  We therefore send a second configure here and adjust
//! the window's position to leave room for the title bar.

use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode;
use smithay::reexports::wayland_server::Resource;
use smithay::wayland::seat::WaylandFocus;
use smithay::wayland::shell::xdg::{ToplevelSurface, decoration::XdgDecorationHandler};

use crate::state::State;

impl XdgDecorationHandler for State {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        tracing::debug!(
            surface = ?toplevel.wl_surface().id(),
            "new decoration — requesting server-side decorations",
        );

        // Request server-side decorations so the compositor renders title bars.
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(Mode::ServerSide);
        });

        // Move the window down to leave room for the title bar.  At
        // `new_toplevel` time the decoration mode was not yet negotiated,
        // so `map_window` placed the window without an SSD offset.
        adjust_window_position_for_ssd(&mut self.space, &toplevel, true);

        // Send a configure so the client learns the compositor prefers SSD.
        // The initial configure from `map_window` was sent before the
        // decoration object existed, so it did not include the mode.
        toplevel.send_configure();
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, mode: Mode) {
        tracing::debug!(
            surface = ?toplevel.wl_surface().id(),
            ?mode,
            "client requested decoration mode",
        );

        // Read the previous mode so we can adjust the window position on
        // SSD ↔ CSD transitions.
        let was_ssd = toplevel.with_pending_state(|state| {
            let was = state.decoration_mode == Some(Mode::ServerSide);
            // Honor the client's preference — CSD apps keep their own decorations.
            state.decoration_mode = Some(mode);
            was
        });
        let will_be_ssd = mode == Mode::ServerSide;

        match (was_ssd, will_be_ssd) {
            (true, false) => adjust_window_position_for_ssd(&mut self.space, &toplevel, false),
            (false, true) => adjust_window_position_for_ssd(&mut self.space, &toplevel, true),
            _ => {}
        }

        // Notify the client of the negotiated mode.
        toplevel.send_configure();
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        tracing::debug!(
            surface = ?toplevel.wl_surface().id(),
            "decoration mode unset — falling back to server-side",
        );

        let was_ssd = toplevel.with_pending_state(|state| {
            let was = state.decoration_mode == Some(Mode::ServerSide);
            state.decoration_mode = Some(Mode::ServerSide);
            was
        });

        if !was_ssd {
            adjust_window_position_for_ssd(&mut self.space, &toplevel, true);
        }

        toplevel.send_configure();
    }
}

/// Adjust a window's position in the space to account for the SSD title bar.
///
/// When `add_offset` is true, the window is moved **down** by
/// [`TITLEBAR_HEIGHT`](crate::decorations::TITLEBAR_HEIGHT) so the title bar
/// (rendered above the client content) stays on-screen.  When false, the
/// offset is removed (transition back to CSD).
fn adjust_window_position_for_ssd(
    space: &mut smithay::desktop::Space<smithay::desktop::Window>,
    toplevel: &ToplevelSurface,
    add_offset: bool,
) {
    use crate::decorations::TITLEBAR_HEIGHT;

    let wl_surface = toplevel.wl_surface().clone();
    let Some(window) = space.elements().find(|w| w.wl_surface().is_some_and(|s| *s == wl_surface)).cloned() else {
        return;
    };
    let Some(loc) = space.element_location(&window) else {
        return;
    };
    let delta = if add_offset { TITLEBAR_HEIGHT } else { -TITLEBAR_HEIGHT };
    space.map_element(window, (loc.x, loc.y + delta), false);
}
