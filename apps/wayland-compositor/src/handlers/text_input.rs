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
        // Track IME popup surface
    }

    fn dismiss_popup(&mut self, _surface: PopupSurface) {
        // Dismiss IME popup
    }

    fn popup_repositioned(&mut self, _surface: PopupSurface) {
        // Handle popup reposition
    }

    fn parent_geometry(&self, _parent: &WlSurface) -> Rectangle<i32, Logical> {
        // Return a default geometry for the parent surface.
        // A real compositor would look up the actual surface geometry.
        Rectangle::default()
    }
}
