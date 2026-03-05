//! `wp-tearing-control-v1` handler — async page flip presentation hint.
//!
//! Games and latency-sensitive applications use this protocol to request
//! tearing-enabled presentation.  Our compositor always uses vsync, so
//! the hint is accepted but silently ignored.
//!
//! Smithay 0.7 does not provide a high-level abstraction for this protocol.
//! We implement `GlobalDispatch` / `Dispatch` manually using the generated
//! bindings from `wayland-protocols 0.32`.

use smithay::reexports::{
    wayland_protocols::wp::tearing_control::v1::server::{
        wp_tearing_control_manager_v1::{self, WpTearingControlManagerV1},
        wp_tearing_control_v1::{self, WpTearingControlV1},
    },
    wayland_server::{Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource, backend::GlobalId},
};

use crate::state::State;

// ---------------------------------------------------------------------------
// GlobalDispatch — advertise the wp_tearing_control_manager_v1 global
// ---------------------------------------------------------------------------

impl GlobalDispatch<WpTearingControlManagerV1, ()> for State {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<WpTearingControlManagerV1>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

// ---------------------------------------------------------------------------
// Dispatch — handle manager requests (destroy + get_tearing_control)
// ---------------------------------------------------------------------------

impl Dispatch<WpTearingControlManagerV1, ()> for State {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &WpTearingControlManagerV1,
        request: wp_tearing_control_manager_v1::Request,
        _data: &(),
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
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

impl Dispatch<WpTearingControlV1, ()> for State {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &WpTearingControlV1,
        request: wp_tearing_control_v1::Request,
        _data: &(),
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
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
