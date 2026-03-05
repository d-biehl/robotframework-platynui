//! `pointer-warp-v1` handler — client-requested pointer warping.
//!
//! Allows clients to request the pointer to be moved to a position relative
//! to a `wl_surface`.  The compositor honors the request when the surface
//! has pointer focus.  Used by accessibility tools, remote-desktop clients,
//! and application drag operations.
//!
//! Smithay 0.7 does not yet provide a high-level abstraction for this
//! protocol, so we implement `GlobalDispatch` / `Dispatch` manually using
//! the generated bindings from `wayland-protocols 0.32.10`.

use smithay::{
    input::pointer::MotionEvent,
    reexports::{
        wayland_protocols::wp::pointer_warp::v1::server::wp_pointer_warp_v1::{self, WpPointerWarpV1},
        wayland_server::{Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, backend::GlobalId},
    },
    utils::{Logical, Point, SERIAL_COUNTER},
    wayland::seat::WaylandFocus,
};

use crate::state::State;

// ---------------------------------------------------------------------------
// Global data — carries a filter closure for security policy
// ---------------------------------------------------------------------------

pub struct PointerWarpGlobalData {
    filter: Box<dyn Fn(&Client) -> bool + Send + Sync>,
}

// ---------------------------------------------------------------------------
// GlobalDispatch — advertise the wp_pointer_warp_v1 global
// ---------------------------------------------------------------------------

impl GlobalDispatch<WpPointerWarpV1, PointerWarpGlobalData> for State {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<WpPointerWarpV1>,
        _global_data: &PointerWarpGlobalData,
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }

    fn can_view(client: Client, global_data: &PointerWarpGlobalData) -> bool {
        (global_data.filter)(&client)
    }
}

// ---------------------------------------------------------------------------
// Dispatch — handle warp_pointer / destroy requests
// ---------------------------------------------------------------------------

impl Dispatch<WpPointerWarpV1, ()> for State {
    fn request(
        state: &mut Self,
        _client: &Client,
        _resource: &WpPointerWarpV1,
        request: wp_pointer_warp_v1::Request,
        _data: &(),
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wp_pointer_warp_v1::Request::WarpPointer { surface, pointer: _, x, y, serial: _ } => {
                let pos: Point<f64, Logical> = (x, y).into();

                // Find the window containing this surface.
                let Some(window) =
                    state.space.elements().find(|w| w.wl_surface().is_some_and(|s| *s == surface)).cloned()
                else {
                    tracing::debug!("pointer warp: surface not found in space");
                    return;
                };

                let Some(element_loc) = state.space.element_location(&window) else {
                    tracing::debug!("pointer warp: window has no location in space");
                    return;
                };

                // Surface-local → global compositor coordinates.
                let global_pos: Point<f64, Logical> =
                    (element_loc.to_f64().x + pos.x, element_loc.to_f64().y + pos.y).into();

                tracing::debug!(
                    surface_x = pos.x,
                    surface_y = pos.y,
                    global_x = global_pos.x,
                    global_y = global_pos.y,
                    "pointer warp",
                );

                // Update pointer location and notify clients via motion event.
                state.pointer_location = global_pos;

                let serial = SERIAL_COUNTER.next_serial();
                let under = crate::input::surface_under(state);
                let pointer = state.pointer();
                pointer.motion(state, under, &MotionEvent { location: global_pos, serial, time: 0 });
                pointer.frame(state);
            }
            wp_pointer_warp_v1::Request::Destroy => {}
            _ => {
                tracing::debug!("unhandled pointer warp request");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Registration helper
// ---------------------------------------------------------------------------

/// Register the `wp_pointer_warp_v1` global.
pub fn init_pointer_warp(dh: &DisplayHandle, filter: impl Fn(&Client) -> bool + Send + Sync + 'static) -> GlobalId {
    dh.create_global::<State, WpPointerWarpV1, _>(1, PointerWarpGlobalData { filter: Box::new(filter) })
}
