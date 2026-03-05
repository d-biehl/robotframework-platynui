//! `pointer-constraints-v1` + `relative-pointer-v1` handler.
//!
//! Pointer constraints allow clients to lock or confine the pointer to a
//! specific region (e.g., for gaming or drag operations). Relative pointer
//! provides raw delta events independent of constraints.

use smithay::{
    input::pointer::PointerHandle,
    reexports::wayland_server::{Resource, protocol::wl_surface::WlSurface},
    utils::{Logical, Point},
    wayland::pointer_constraints::{PointerConstraintsHandler, with_pointer_constraint},
};

use crate::state::State;

impl PointerConstraintsHandler for State {
    fn new_constraint(&mut self, surface: &WlSurface, pointer: &PointerHandle<Self>) {
        // Immediately activate all constraints in the test compositor so
        // clients receive the `locked`/`confined` event.
        with_pointer_constraint(surface, pointer, |constraint| {
            if let Some(constraint) = constraint {
                tracing::debug!(
                    surface = ?surface.id(),
                    active = constraint.is_active(),
                    "activating pointer constraint",
                );
                constraint.activate();
            }
        });
    }

    fn cursor_position_hint(
        &mut self,
        surface: &WlSurface,
        _pointer: &PointerHandle<Self>,
        location: Point<f64, Logical>,
    ) {
        tracing::trace!(
            surface = ?surface.id(),
            ?location,
            "pointer constraint cursor position hint",
        );
    }
}
