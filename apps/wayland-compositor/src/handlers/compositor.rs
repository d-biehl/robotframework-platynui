//! `wl_compositor` + `wl_subcompositor` handler.

use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    desktop::{PopupKind, layer_map_for_output},
    reexports::wayland_server::{Client, protocol::wl_surface::WlSurface},
    utils::Transform,
    wayland::{
        compositor::{CompositorClientState, CompositorHandler, send_surface_state, with_states},
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

        // Send preferred buffer scale/transform (wl_compositor v6).
        // Determine the output scale from the window's output, falling back
        // to the primary output for layer surfaces, popups, etc.
        let scale = self.space.elements().find(|w| w.wl_surface().is_some_and(|s| *s == *surface)).map_or_else(
            || self.output.current_scale().integer_scale(),
            |w| self.output_for_window(w).current_scale().integer_scale(),
        );
        with_states(surface, |data| {
            send_surface_state(surface, data, scale, Transform::Normal);
        });

        // If this surface belongs to a mapped window, handle the commit
        let window = self.space.elements().find(|w| w.wl_surface().is_some_and(|s| *s == *surface)).cloned();
        if let Some(window) = window {
            window.on_commit();

            // Forward title/app_id/state changes to foreign-toplevel protocols
            crate::handlers::foreign_toplevel::update_toplevel_metadata(self, &window);
        }

        // Handle popup commits — moves tracked popups from unmapped to the tree
        self.popup_manager.commit(surface);

        // Handle layer surface commits — after the client commits, the pending
        // layer state (anchor, size, exclusive zone, margin) becomes current.
        // Re-arranging the layer map computes the correct geometry and sends
        // the initial (or updated) `configure` event.  Without this, the layer
        // surface never receives a configure and the client waits forever.
        handle_layer_surface_commit(self, surface);

        // Safety net: ensure popup gets its initial configure even if new_popup
        // didn't send one (e.g. because the parent wasn't set yet at that point).
        if let Some(popup) = self.popup_manager.find_popup(surface) {
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

/// Re-arrange the layer map when a layer surface commits.
///
/// After a client commits, the pending layer-shell state (anchor, size,
/// exclusive zone, margin) becomes the current state.  Calling `arrange()`
/// recomputes the layout and updates the non-exclusive (usable) zone.
/// `send_pending_configure()` then sends the initial or any updated
/// configure event for the layer surface.
fn handle_layer_surface_commit(state: &mut State, surface: &WlSurface) {
    // Find the output whose layer map contains this surface and re-arrange it.
    for output in &state.outputs {
        let mut map = layer_map_for_output(output);
        if let Some(layer) = map.layer_for_surface(surface, smithay::desktop::WindowSurfaceType::TOPLEVEL) {
            let layer = layer.clone();
            map.arrange();
            let zone = map.non_exclusive_zone();
            drop(map); // release MutexGuard before protocol call

            tracing::trace!(
                namespace = layer.namespace(),
                output = output.name(),
                zone_x = zone.loc.x,
                zone_y = zone.loc.y,
                zone_w = zone.size.w,
                zone_h = zone.size.h,
                "layer map arranged",
            );

            if let Some(serial) = layer.layer_surface().send_pending_configure() {
                let serial: u32 = serial.into();
                tracing::debug!(
                    namespace = layer.namespace(),
                    output = output.name(),
                    serial,
                    "sent layer surface configure",
                );
            }
            return;
        }
    }
}
