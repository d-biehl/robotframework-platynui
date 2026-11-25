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
//! Smithay provides no abstraction for this protocol, so this module
//! implements `GlobalDispatch2` / `Dispatch2` on the generated bindings from
//! `wayland-protocols`.

use smithay::reexports::{
    wayland_protocols::xdg::toplevel_drag::v1::server::{
        xdg_toplevel_drag_manager_v1::{self, XdgToplevelDragManagerV1},
        xdg_toplevel_drag_v1::{self, XdgToplevelDragV1},
    },
    wayland_server::{Client, DataInit, DisplayHandle, New, backend::GlobalId},
};
use smithay::wayland::{Dispatch2, GlobalDispatch2};

use crate::state::State;

// ---------------------------------------------------------------------------
// GlobalDispatch — advertise the xdg_toplevel_drag_manager_v1 global
// ---------------------------------------------------------------------------

impl GlobalDispatch2<XdgToplevelDragManagerV1, State> for () {
    fn bind(
        &self,
        _state: &mut State,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<XdgToplevelDragManagerV1>,
        data_init: &mut DataInit<'_, State>,
    ) {
        data_init.init(resource, ());
    }
}

// ---------------------------------------------------------------------------
// Dispatch — handle manager requests (destroy + get_xdg_toplevel_drag)
// ---------------------------------------------------------------------------

impl Dispatch2<XdgToplevelDragManagerV1, State> for () {
    fn request(
        &self,
        _state: &mut State,
        _client: &Client,
        _resource: &XdgToplevelDragManagerV1,
        request: xdg_toplevel_drag_manager_v1::Request,
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, State>,
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

impl Dispatch2<XdgToplevelDragV1, State> for () {
    fn request(
        &self,
        _state: &mut State,
        _client: &Client,
        _resource: &XdgToplevelDragV1,
        request: xdg_toplevel_drag_v1::Request,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, State>,
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
