//! `wl_compositor` + `wl_subcompositor` handler.

use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    desktop::PopupKind,
    reexports::wayland_server::{Client, protocol::wl_surface::WlSurface},
    wayland::{
        compositor::{CompositorClientState, CompositorHandler},
        seat::WaylandFocus,
    },
};

use crate::{client::ClientState, state::State};

impl CompositorHandler for State {
    fn compositor_state(&mut self) -> &mut smithay::wayland::compositor::CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        // XWayland's internal client uses `()` as client data and doesn't carry
        // our ClientState.  Provide a static fallback — this only applies to the
        // single XWayland connection so one allocation is acceptable.
        static FALLBACK: std::sync::OnceLock<CompositorClientState> = std::sync::OnceLock::new();

        if let Some(state) = client.get_data::<ClientState>() {
            return &state.compositor_state;
        }
        FALLBACK.get_or_init(CompositorClientState::default)
    }

    fn commit(&mut self, surface: &WlSurface) {
        tracing::trace!("commit: surface commit");

        // Process pending buffer state for renderers
        on_commit_buffer_handler::<Self>(surface);

        // If this surface belongs to a mapped window, handle the commit
        if let Some(window) = self.space.elements().find(|w| w.wl_surface().is_some_and(|s| *s == *surface)).cloned() {
            window.on_commit();
        }

        // Handle popup commits — moves tracked popups from unmapped to the tree
        self.popup_manager.commit(surface);

        // Safety net: ensure popup gets its initial configure even if new_popup
        // didn't send one (e.g. because the parent wasn't set yet at that point).
        if let Some(popup) = self.popup_manager.find_popup(surface) {
            if let PopupKind::Xdg(ref xdg) = popup {
                let geo = xdg.with_pending_state(|s| s.geometry);
                tracing::debug!(
                    geo_x = geo.loc.x,
                    geo_y = geo.loc.y,
                    geo_w = geo.size.w,
                    geo_h = geo.size.h,
                    initial_sent = xdg.is_initial_configure_sent(),
                    "commit: popup surface state",
                );
            }
            ensure_popup_initial_configure(&popup);
        }

        // Ensure pending xdg toplevels get an initial configure
        crate::workspace::handle_commit(&self.xdg_shell_state, surface);
    }
}

/// Send the initial configure for a popup if it hasn't been sent yet.
fn ensure_popup_initial_configure(popup: &PopupKind) {
    if let PopupKind::Xdg(surface) = popup
        && !surface.is_initial_configure_sent()
    {
        tracing::debug!("sending deferred initial popup configure");
        if let Err(err) = surface.send_configure() {
            tracing::warn!(?err, "failed to send deferred popup configure");
        }
    }
}
