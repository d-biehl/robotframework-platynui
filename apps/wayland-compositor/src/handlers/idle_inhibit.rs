//! `zwp-idle-inhibit-v1` handler — idle inhibition.
//!
//! Video players and presentation applications use this protocol to prevent
//! the screensaver or DPMS from activating while content is being displayed.
//!
//! The compositor tracks active inhibitors in a `HashSet<WlSurface>` on
//! [`State`].  When at least one inhibitor is active, the idle notifier is
//! told to suppress idle transitions via
//! [`IdleNotifierState::set_is_inhibited`].

use smithay::wayland::idle_inhibit::IdleInhibitHandler;

use crate::state::State;

impl IdleInhibitHandler for State {
    fn inhibit(&mut self, surface: smithay::reexports::wayland_server::protocol::wl_surface::WlSurface) {
        tracing::debug!(?surface, "idle inhibit requested");
        self.idle_inhibit_surfaces.insert(surface);
        self.idle_notify_state.set_is_inhibited(true);
    }

    fn uninhibit(&mut self, surface: smithay::reexports::wayland_server::protocol::wl_surface::WlSurface) {
        tracing::debug!(?surface, "idle inhibit released");
        self.idle_inhibit_surfaces.remove(&surface);
        let still_inhibited = !self.idle_inhibit_surfaces.is_empty();
        self.idle_notify_state.set_is_inhibited(still_inhibited);
    }
}
