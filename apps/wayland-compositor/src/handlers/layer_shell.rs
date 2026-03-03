//! `wlr-layer-shell-unstable-v1` — layer surfaces for panels, overlays, wallpapers, etc.
//!
//! Layer surfaces are rendered at fixed z-levels relative to normal windows:
//! `Background` < `Bottom` < (windows) < `Top` < `Overlay`.
//!
//! This handler creates desktop `LayerSurface` wrappers and maps them onto
//! the per-output `LayerMap` so that smithay handles exclusive-zone
//! calculations and hit-testing automatically.

use smithay::{
    desktop::{LayerSurface, layer_map_for_output},
    output::Output,
    reexports::wayland_server::protocol::wl_output::WlOutput,
    wayland::shell::{
        wlr_layer::{self, Layer, LayerSurfaceConfigure, WlrLayerShellHandler, WlrLayerShellState},
        xdg::PopupSurface,
    },
};

use crate::state::State;

impl WlrLayerShellHandler for State {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: wlr_layer::LayerSurface,
        wl_output: Option<WlOutput>,
        _layer: Layer,
        namespace: String,
    ) {
        // Wrap the protocol-level handle in the desktop helper.
        let layer_surface = LayerSurface::new(surface, namespace.clone());

        // Resolve the target output via `Output::from_resource`. If the client
        // didn't specify one, use the output under the pointer (or the primary
        // output as fallback).
        let output = wl_output
            .as_ref()
            .and_then(Output::from_resource)
            .unwrap_or_else(|| self.output_at_point(self.pointer_location).clone());

        // Map the surface onto the output's layer map. This sends the initial
        // configure event (size, anchor, etc.) via `arrange()`.
        let mut map = layer_map_for_output(&output);
        let result = map.map_layer(&layer_surface);
        drop(map); // release the MutexGuard before logging or any further work

        match result {
            Ok(()) => {
                let output_geo = self.space.output_geometry(&output);
                tracing::info!(namespace, output = output.name(), ?output_geo, "layer surface mapped",);
            }
            Err(err) => {
                tracing::warn!(%err, namespace, "failed to map layer surface");
            }
        }
    }

    /// Handle a popup assigned to a layer surface as its parent.
    ///
    /// At this point the popup's parent has been set by smithay (via
    /// `zwlr_layer_surface.get_popup`), so we can now properly constrain
    /// the popup within its parent's output and send the initial configure.
    ///
    /// Note: `XdgShellHandler::new_popup` is called *before* the parent is
    /// set for layer-shell popups, so it only tracks the popup and defers
    /// constraining + configure to this callback.
    fn new_popup(&mut self, _parent: wlr_layer::LayerSurface, popup: PopupSurface) {
        // Now that the parent is set, constrain the popup within its output.
        super::xdg_shell::unconstrain_popup(&popup, self);

        let final_geo = popup.with_pending_state(|state| state.geometry);
        tracing::debug!(
            x = final_geo.loc.x,
            y = final_geo.loc.y,
            w = final_geo.size.w,
            h = final_geo.size.h,
            "layer_shell::new_popup: constrained geometry",
        );

        // Send wl_surface.enter(output) so the client knows the correct
        // output (scale factor) for the popup.
        let popup_output =
            super::xdg_shell::find_popup_parent_output(&popup, self).cloned().unwrap_or_else(|| self.output.clone());
        popup_output.enter(popup.wl_surface());

        // Send the initial configure with the constrained geometry.
        if let Err(err) = popup.send_configure() {
            tracing::warn!(?err, "layer_shell::new_popup: failed to send configure");
        } else {
            tracing::debug!("layer_shell::new_popup: configure sent");
        }
    }

    fn layer_destroyed(&mut self, surface: wlr_layer::LayerSurface) {
        // Find the output this surface was mapped on and unmap it.
        for output in &self.outputs {
            let mut map = layer_map_for_output(output);
            // Find the matching desktop LayerSurface by comparing protocol handles.
            let desktop_surface =
                map.layers().find(|ls| ls.layer_surface().wl_surface() == surface.wl_surface()).cloned();
            if let Some(desktop_surface) = desktop_surface {
                map.unmap_layer(&desktop_surface);
                tracing::info!(
                    namespace = desktop_surface.namespace(),
                    output = output.name(),
                    "layer surface unmapped",
                );
                return;
            }
        }
    }

    fn ack_configure(
        &mut self,
        _surface: smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
        _configure: LayerSurfaceConfigure,
    ) {
        // No special handling needed — smithay tracks the acknowledged state.
    }
}
