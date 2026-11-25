//! `wp-tearing-control-v1` handler — async page flip presentation hint.
//!
//! Games and latency-sensitive applications use this protocol to request
//! tearing-enabled presentation.  Our compositor always uses vsync, so
//! the hint is accepted but silently ignored.
//!
//! Smithay provides no abstraction for this protocol, so this module
//! implements `GlobalDispatch2` / `Dispatch2` on the generated bindings from
//! `wayland-protocols`.

use smithay::reexports::{
    wayland_protocols::wp::tearing_control::v1::server::{
        wp_tearing_control_manager_v1::{self, WpTearingControlManagerV1},
        wp_tearing_control_v1::{self, WpTearingControlV1},
    },
    wayland_server::{Client, DataInit, DisplayHandle, New, Resource, backend::GlobalId},
};
use smithay::wayland::{Dispatch2, GlobalDispatch2};

use crate::state::State;

// ---------------------------------------------------------------------------
// GlobalDispatch — advertise the wp_tearing_control_manager_v1 global
// ---------------------------------------------------------------------------

impl GlobalDispatch2<WpTearingControlManagerV1, State> for () {
    fn bind(
        &self,
        _state: &mut State,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<WpTearingControlManagerV1>,
        data_init: &mut DataInit<'_, State>,
    ) {
        data_init.init(resource, ());
    }
}

// ---------------------------------------------------------------------------
// Dispatch — handle manager requests (destroy + get_tearing_control)
// ---------------------------------------------------------------------------

impl Dispatch2<WpTearingControlManagerV1, State> for () {
    fn request(
        &self,
        _state: &mut State,
        _client: &Client,
        _resource: &WpTearingControlManagerV1,
        request: wp_tearing_control_manager_v1::Request,
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, State>,
    ) {
        match request {
            wp_tearing_control_manager_v1::Request::GetTearingControl { id, surface } => {
                tracing::debug!(surface = %surface.id(), "tearing control created (no-op)");
                data_init.init(id, ());
            }
            wp_tearing_control_manager_v1::Request::Destroy => {}
            _ => {
                tracing::debug!("unhandled tearing control manager request");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Dispatch — handle per-surface tearing control requests
// ---------------------------------------------------------------------------

impl Dispatch2<WpTearingControlV1, State> for () {
    fn request(
        &self,
        _state: &mut State,
        _client: &Client,
        _resource: &WpTearingControlV1,
        request: wp_tearing_control_v1::Request,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, State>,
    ) {
        match request {
            wp_tearing_control_v1::Request::SetPresentationHint { hint } => {
                tracing::trace!(?hint, "tearing control presentation hint (ignored)");
            }
            wp_tearing_control_v1::Request::Destroy => {}
            _ => {
                tracing::debug!("unhandled tearing control request");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Registration helper
// ---------------------------------------------------------------------------

/// Register the `wp_tearing_control_manager_v1` global.
pub fn init_tearing_control(dh: &DisplayHandle) -> GlobalId {
    dh.create_global::<State, WpTearingControlManagerV1, _>(1, ())
}
