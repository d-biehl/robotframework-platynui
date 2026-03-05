//! `xdg-toplevel-drag-v1` handler — move a window during a drag operation.
//!
//! Allows clients to attach an `xdg_toplevel` to a drag-and-drop operation,
//! making the window follow the pointer.  Used by browsers for tab detach
//! (Firefox, Chromium) and by drag-from-window scenarios.
//!
//! This is a stub implementation: the manager global is advertised so clients
//! don't log protocol-not-found warnings, but the actual window-during-drag
//! behaviour is not yet implemented.  The `attach` request is accepted and
//! logged but has no compositor-side effect.
//!
//! Smithay 0.7 does not provide a high-level abstraction for this protocol.
//! We implement `GlobalDispatch` / `Dispatch` manually using the generated
//! bindings from `wayland-protocols 0.32`.

use smithay::reexports::{
    wayland_protocols::xdg::toplevel_drag::v1::server::{
        xdg_toplevel_drag_manager_v1::{self, XdgToplevelDragManagerV1},
        xdg_toplevel_drag_v1::{self, XdgToplevelDragV1},
    },
    wayland_server::{Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, backend::GlobalId},
};

use crate::state::State;

// ---------------------------------------------------------------------------
// GlobalDispatch — advertise the xdg_toplevel_drag_manager_v1 global
// ---------------------------------------------------------------------------

impl GlobalDispatch<XdgToplevelDragManagerV1, ()> for State {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<XdgToplevelDragManagerV1>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

// ---------------------------------------------------------------------------
// Dispatch — handle manager requests (destroy + get_xdg_toplevel_drag)
// ---------------------------------------------------------------------------

impl Dispatch<XdgToplevelDragManagerV1, ()> for State {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &XdgToplevelDragManagerV1,
        request: xdg_toplevel_drag_manager_v1::Request,
        _data: &(),
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            xdg_toplevel_drag_manager_v1::Request::GetXdgToplevelDrag { id, data_source: _ } => {
                tracing::debug!("toplevel drag created (stub)");
                data_init.init(id, ());
            }
            xdg_toplevel_drag_manager_v1::Request::Destroy => {}
            _ => {
                tracing::debug!("unhandled toplevel drag manager request");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Dispatch — handle per-drag requests (attach + destroy)
// ---------------------------------------------------------------------------

impl Dispatch<XdgToplevelDragV1, ()> for State {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &XdgToplevelDragV1,
        request: xdg_toplevel_drag_v1::Request,
        _data: &(),
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            xdg_toplevel_drag_v1::Request::Attach { toplevel: _, x_offset, y_offset } => {
                tracing::debug!(x_offset, y_offset, "toplevel drag attach (stub, not yet implemented)");
            }
            xdg_toplevel_drag_v1::Request::Destroy => {}
            _ => {
                tracing::debug!("unhandled toplevel drag request");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Registration helper
// ---------------------------------------------------------------------------

/// Register the `xdg_toplevel_drag_manager_v1` global.
pub fn init_toplevel_drag(dh: &DisplayHandle) -> GlobalId {
    dh.create_global::<State, XdgToplevelDragManagerV1, _>(1, ())
}
