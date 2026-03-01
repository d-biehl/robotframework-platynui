//! `pointer-constraints-v1` + `relative-pointer-v1` handler.
//!
//! Pointer constraints allow clients to lock or confine the pointer to a
//! specific region (e.g., for gaming or drag operations). Relative pointer
//! provides raw delta events independent of constraints.

use smithay::{
    input::pointer::PointerHandle,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point},
    wayland::pointer_constraints::PointerConstraintsHandler,
};

use crate::state::State;

impl PointerConstraintsHandler for State {
    fn new_constraint(&mut self, _surface: &WlSurface, _pointer: &PointerHandle<Self>) {
        // Accept all constraints in the test compositor
    }

    fn cursor_position_hint(
        &mut self,
        _surface: &WlSurface,
        _pointer: &PointerHandle<Self>,
        _location: Point<f64, Logical>,
    ) {
        // Acknowledge position hints but don't act on them
    }
}
