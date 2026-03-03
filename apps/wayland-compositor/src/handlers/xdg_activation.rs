//! `xdg-activation-v1` handler — focus-stealing prevention.
//!
//! Allows applications to request activation (raising) of a window via tokens.
//! Used by `gtk_window_present()` and similar APIs.

use smithay::{
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    wayland::{
        seat::WaylandFocus,
        xdg_activation::{XdgActivationHandler, XdgActivationState, XdgActivationToken, XdgActivationTokenData},
    },
};

use crate::state::State;

impl XdgActivationHandler for State {
    fn activation_state(&mut self) -> &mut XdgActivationState {
        &mut self.xdg_activation_state
    }

    fn token_created(&mut self, _token: XdgActivationToken, _data: XdgActivationTokenData) -> bool {
        // Accept all tokens in the test compositor
        true
    }

    fn request_activation(
        &mut self,
        _token: XdgActivationToken,
        _token_data: XdgActivationTokenData,
        surface: WlSurface,
    ) {
        let window = self.space.elements().find(|w| w.wl_surface().is_some_and(|s| *s == surface)).cloned();

        if let Some(window) = window {
            self.space.raise_element(&window, true);
            let serial = smithay::utils::SERIAL_COUNTER.next_serial();
            let keyboard = self.keyboard();
            keyboard.set_focus(self, Some(crate::focus::KeyboardFocusTarget::Window(window)), serial);
        }
    }
}
