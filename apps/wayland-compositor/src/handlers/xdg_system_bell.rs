//! `xdg-system-bell-v1` handler — system bell notification.
//!
//! Terminal emulators and many GTK applications trigger this event.
//! We simply log the bell; no audio output is produced.

use smithay::wayland::xdg_system_bell::XdgSystemBellHandler;

use crate::state::State;

impl XdgSystemBellHandler for State {
    fn ring(&mut self, surface: Option<smithay::reexports::wayland_server::protocol::wl_surface::WlSurface>) {
        tracing::debug!(?surface, "system bell");
    }
}
