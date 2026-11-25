//! `xdg-toplevel-tag-v1` handler — persistent toplevel identification tags.
//!
//! Clients use this staging protocol to assign a human-readable tag and
//! description to toplevels, enabling compositors to persist window
//! properties (position, size, rules) across session restarts.
//!
//! The tag is an untranslated, human-readable identifier like `"main window"`,
//! `"settings"`, or `"e-mail composer"`. The description is a translated
//! variant suitable for display in UI or screen readers.
//!
//! Our compositor stores tags and descriptions in-memory per toplevel for
//! potential automation queries but does not persist them across sessions.
//!
//! Smithay has its own `wayland::xdg_toplevel_tag` module; this compositor
//! keeps a handler of its own, which stores tag and description per toplevel.
//! It implements `GlobalDispatch2` / `Dispatch2` on the generated bindings from
//! `wayland-protocols`.

use smithay::reexports::{
    wayland_protocols::xdg::toplevel_tag::v1::server::xdg_toplevel_tag_manager_v1::{self, XdgToplevelTagManagerV1},
    wayland_server::{Client, DataInit, DisplayHandle, New, Resource, backend::GlobalId},
};
use smithay::wayland::{Dispatch2, GlobalDispatch2};

use crate::state::State;

/// Tag and description for a toplevel, set via `xdg-toplevel-tag-v1`.
///
/// The tag is an untranslated identifier used by the compositor for
/// persistent window matching. The description is a translated string
/// suitable for display in UI.
#[derive(Debug, Clone, Default)]
pub struct ToplevelTagInfo {
    /// Untranslated tag (e.g. `"main window"`, `"settings"`).
    pub tag: Option<String>,
    /// Translated description, typically a localized version of the tag.
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// GlobalDispatch — advertise the xdg_toplevel_tag_manager_v1 global
// ---------------------------------------------------------------------------

impl GlobalDispatch2<XdgToplevelTagManagerV1, State> for () {
    fn bind(
        &self,
        _state: &mut State,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<XdgToplevelTagManagerV1>,
        data_init: &mut DataInit<'_, State>,
    ) {
        data_init.init(resource, ());
    }
}

// ---------------------------------------------------------------------------
// Dispatch — handle manager requests (destroy, set_toplevel_tag, set_toplevel_description)
// ---------------------------------------------------------------------------

impl Dispatch2<XdgToplevelTagManagerV1, State> for () {
    fn request(
        &self,
        state: &mut State,
        _client: &Client,
        _resource: &XdgToplevelTagManagerV1,
        request: xdg_toplevel_tag_manager_v1::Request,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, State>,
    ) {
        match request {
            xdg_toplevel_tag_manager_v1::Request::SetToplevelTag { toplevel, tag } => {
                tracing::debug!(toplevel = %toplevel.id(), tag, "toplevel tag set");

                // Resolve the wl_surface ObjectId for this toplevel.
                if let Some(tl) = state.xdg_shell_state.get_toplevel(&toplevel) {
                    let surface_id = tl.wl_surface().id();
                    state.toplevel_tags.entry(surface_id).or_default().tag = Some(tag);
                }
            }
            xdg_toplevel_tag_manager_v1::Request::SetToplevelDescription { toplevel, description } => {
                tracing::debug!(
                    toplevel = %toplevel.id(),
                    description,
                    "toplevel description set"
                );

                if let Some(tl) = state.xdg_shell_state.get_toplevel(&toplevel) {
                    let surface_id = tl.wl_surface().id();
                    state.toplevel_tags.entry(surface_id).or_default().description = Some(description);
                }
            }
            xdg_toplevel_tag_manager_v1::Request::Destroy => {}
            _ => {
                tracing::debug!("unhandled toplevel tag manager request");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Registration helper
// ---------------------------------------------------------------------------

/// Register the `xdg_toplevel_tag_manager_v1` global.
pub fn init_toplevel_tag(dh: &DisplayHandle) -> GlobalId {
    dh.create_global::<State, XdgToplevelTagManagerV1, _>(1, ())
}
