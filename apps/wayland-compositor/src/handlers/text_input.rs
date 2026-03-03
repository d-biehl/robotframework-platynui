//! `text-input-v3` + `input-method-v2` handler — IME support.
//!
//! Text input enables input method editors (IMEs) for CJK characters,
//! compose sequences, and emoji input. The input method protocol allows
//! IME implementations to communicate with the compositor.

use smithay::{
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Rectangle},
    wayland::input_method::{InputMethodHandler, PopupSurface},
};

use crate::state::State;

impl InputMethodHandler for State {
    fn new_popup(&mut self, _surface: PopupSurface) {
        // No-op: IME popups not yet supported.
    }

    fn dismiss_popup(&mut self, _surface: PopupSurface) {
        // No-op: IME popups not yet supported.
    }

    fn popup_repositioned(&mut self, _surface: PopupSurface) {
        // No-op: IME popups not yet supported.
    }

    fn parent_geometry(&self, parent: &WlSurface) -> Rectangle<i32, Logical> {
        // Look up the window that owns this surface and return its geometry
        // in the compositor space so that IME popups are positioned correctly.
        use smithay::wayland::seat::WaylandFocus;

        for window in self.space.elements() {
            if window.wl_surface().is_some_and(|s| *s == *parent)
                && let Some(loc) = self.space.element_location(window)
            {
                let geo = window.geometry();
                return Rectangle::new(loc, geo.size);
            }
        }

        // Fallback: use the primary output geometry.
        tracing::debug!("text_input: parent surface not found, using output geometry");
        self.space.output_geometry(&self.output).unwrap_or_default()
    }
}
