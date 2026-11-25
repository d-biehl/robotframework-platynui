//! Pointer warp protocol handler (`wp-pointer-warp-v1`).
//!
//! Lets a client move the pointer to a position inside one of its own
//! surfaces (accessibility tools, remote desktops, applications that confine
//! the pointer during drags). Smithay decodes the request and converts the
//! position to logical coordinates; this module decides whether to honor it:
//!
//! - the surface has pointer focus (also while an implicit grab holds it),
//! - the serial is the one of the last pointer enter the seat sent, and
//! - the position lies inside the surface.
//!
//! Anything else is ignored without a protocol error. An honored warp moves
//! the pointer like any absolute motion ([`crate::input::move_pointer_to`]);
//! the global position is the surface's origin plus the requested offset, so
//! popups and subsurfaces work like toplevels.
//!
//! Smithay's `PointerWarpManager` has no global filter, so the global is
//! created here ([`init_pointer_warp`]) with the restrictive-policy filter;
//! its resources use smithay's `GlobalData`, so the requests are still
//! handled by smithay.

use smithay::{
    backend::{input::InputTime, renderer::utils::with_renderer_surface_state},
    input::pointer::PointerHandle,
    reexports::{
        wayland_protocols::wp::pointer_warp::v1::server::wp_pointer_warp_v1::WpPointerWarpV1,
        wayland_server::{
            Client, DataInit, DisplayHandle, New,
            backend::GlobalId,
            protocol::{wl_pointer::WlPointer, wl_surface::WlSurface},
        },
    },
    utils::{Logical, Point, Serial},
    wayland::{GlobalData, GlobalDispatch2, pointer_warp::PointerWarpHandler, seat::WaylandFocus},
};

use crate::state::State;

/// Data of the `wp_pointer_warp_v1` global: who may see it.
pub struct PointerWarpGlobal {
    filter: Box<dyn Fn(&Client) -> bool + Send + Sync>,
}

impl GlobalDispatch2<WpPointerWarpV1, State> for PointerWarpGlobal {
    fn bind(
        &self,
        _state: &mut State,
        _dh: &DisplayHandle,
        _client: &Client,
        resource: New<WpPointerWarpV1>,
        data_init: &mut DataInit<'_, State>,
    ) {
        // smithay's `GlobalData` routes the requests to `PointerWarpHandler`.
        data_init.init(resource, GlobalData);
    }

    fn can_view(&self, client: &Client) -> bool {
        (self.filter)(client)
    }
}

/// Register the `wp_pointer_warp_v1` global for the clients `filter` accepts.
pub fn init_pointer_warp(dh: &DisplayHandle, filter: impl Fn(&Client) -> bool + Send + Sync + 'static) -> GlobalId {
    dh.create_global::<State, WpPointerWarpV1, _>(1, PointerWarpGlobal { filter: Box::new(filter) })
}

impl PointerWarpHandler for State {
    fn warp_pointer(&mut self, surface: WlSurface, _pointer: WlPointer, pos: Point<f64, Logical>, serial: Serial) {
        let pointer = self.pointer();
        if pointer.last_enter() != Some(serial) {
            tracing::debug!("pointer warp ignored: stale enter serial");
            return;
        }
        let Some(origin) = focused_surface_origin(self, &pointer, &surface) else {
            tracing::debug!("pointer warp ignored: surface does not have pointer focus");
            return;
        };
        let inside = with_renderer_surface_state(&surface, |s| s.surface_size())
            .flatten()
            .is_some_and(|size| pos.x >= 0.0 && pos.y >= 0.0 && pos.x < f64::from(size.w) && pos.y < f64::from(size.h));
        if !inside {
            tracing::debug!(x = pos.x, y = pos.y, "pointer warp ignored: position outside the surface");
            return;
        }
        let target = origin + pos;
        tracing::debug!(x = target.x, y = target.y, "pointer warp");
        crate::input::move_pointer_to(self, target, InputTime::now());
    }
}

/// The global origin of `surface` if it holds the pointer focus — directly,
/// or as the focus an implicit grab started on.
fn focused_surface_origin(
    state: &State,
    pointer: &PointerHandle<State>,
    surface: &WlSurface,
) -> Option<Point<f64, Logical>> {
    let is_surface = |focus: &crate::focus::PointerFocusTarget| focus.wl_surface().is_some_and(|s| *s == *surface);
    if pointer.current_focus().is_some_and(|focus| is_surface(&focus))
        && let Some((focus, origin)) = crate::input::surface_under(state)
        && is_surface(&focus)
    {
        return Some(origin);
    }
    pointer
        .grab_start_data()
        .and_then(|data| data.focus)
        .and_then(|(focus, origin)| is_surface(&focus).then_some(origin))
}
