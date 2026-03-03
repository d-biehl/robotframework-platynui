//! `xdg_shell` handler — toplevels + popups + fullscreen.

use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::{
    desktop::{PopupKind, Window, find_popup_root_surface, get_popup_toplevel_coords, layer_map_for_output},
    input::Seat,
    output::Output,
    reexports::wayland_server::protocol::{wl_output, wl_seat::WlSeat},
    utils::{Logical, Rectangle, Serial},
    wayland::{
        seat::WaylandFocus,
        shell::xdg::{PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState},
    },
};

use crate::{focus::KeyboardFocusTarget, state::State};

impl XdgShellHandler for State {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        // Mark the surface as activated so the client knows it has focus
        surface.with_pending_state(|state| {
            state.states.set(xdg_toplevel::State::Activated);
        });

        // Wrap in a desktop Window and map it
        let window = Window::new_wayland_window(surface);
        let usable_origin = self.usable_geometry_for_output(&self.output.clone()).loc;
        crate::workspace::map_window(&mut self.space, window.clone(), usable_origin);

        // Note: We do NOT announce to foreign-toplevel protocols here because
        // at `new_toplevel` time the client has not yet called `set_title` /
        // `set_app_id` — those arrive before the first `wl_surface.commit()`.
        // Announcing with empty title/app_id causes taskbars like ironbar to
        // ignore the handle ("Handle is missing information!").
        // Instead, `update_toplevel_metadata` in the `commit` handler will
        // lazily announce the toplevel once title/app_id become available.
    }

    fn new_popup(&mut self, surface: PopupSurface, positioner: PositionerState) {
        let geo = positioner.get_geometry();
        tracing::debug!(
            geo_x = geo.loc.x,
            geo_y = geo.loc.y,
            geo_w = geo.size.w,
            geo_h = geo.size.h,
            has_parent = surface.get_parent_surface().is_some(),
            rect_size_w = positioner.rect_size.w,
            rect_size_h = positioner.rect_size.h,
            anchor_rect_x = positioner.anchor_rect.loc.x,
            anchor_rect_y = positioner.anchor_rect.loc.y,
            anchor_rect_w = positioner.anchor_rect.size.w,
            anchor_rect_h = positioner.anchor_rect.size.h,
            anchor_edges = ?positioner.anchor_edges,
            gravity = ?positioner.gravity,
            constraint_adj = ?positioner.constraint_adjustment,
            offset_x = positioner.offset.x,
            offset_y = positioner.offset.y,
            reactive = positioner.reactive,
            parent_size = ?positioner.parent_size,
            "new_popup: positioner state",
        );

        // Apply the positioner geometry so the configure event carries the
        // correct position and size for the popup.
        surface.with_pending_state(|state| {
            state.geometry = geo;
            state.positioner = positioner;
        });

        // Track the popup in the popup manager so it's in the tree
        // before any configure/commit processing.
        let popup = PopupKind::from(surface.clone());
        if let Err(err) = self.popup_manager.track_popup(popup) {
            tracing::warn!(?err, "new_popup: failed to track popup");
        } else {
            tracing::debug!("new_popup: popup tracked successfully");
        }

        // For layer-shell popups, the parent is not yet set at this point
        // (smithay calls XdgShellHandler::new_popup before the layer-shell's
        // get_popup sets the parent).  Constraining, output enter, and the
        // initial configure are deferred to WlrLayerShellHandler::new_popup
        // where the parent is available.  The commit handler's safety net
        // (ensure_popup_initial_configure) guarantees the configure is sent
        // before the first client commit even if the layer-shell callback
        // is delayed.
        if surface.get_parent_surface().is_none() {
            tracing::debug!("new_popup: parent not yet set (layer-shell popup), deferring constrain + configure");
            return;
        }

        // Constrain the popup to fit within the output bounds.  Without this,
        // popups may appear at wrong positions or get clipped by the output
        // edge.  The positioner's constraint_adjustment flags (flip, slide,
        // resize) are applied by `get_unconstrained_geometry`.
        unconstrain_popup(&surface, self);

        let final_geo = surface.with_pending_state(|state| state.geometry);
        tracing::debug!(
            x = final_geo.loc.x,
            y = final_geo.loc.y,
            w = final_geo.size.w,
            h = final_geo.size.h,
            "new_popup: final constrained geometry",
        );

        // Send wl_surface.enter(output) immediately so the client knows which
        // output the popup is on *before* it receives the configure event.
        // GTK4 uses this to determine the scale factor; without it, the popup
        // may not render until the next Space::refresh() cycle delivers the
        // enter event one frame later.
        let popup_output = find_popup_parent_output(&surface, self).cloned().unwrap_or_else(|| self.output.clone());
        popup_output.enter(surface.wl_surface());

        // The client waits for the initial configure before it can commit a
        // buffer, so we MUST send it here.  Without this the client hangs.
        if let Err(err) = surface.send_configure() {
            tracing::warn!(?err, "new_popup: failed to send initial configure");
        } else {
            tracing::debug!("new_popup: initial configure sent");
        }
    }

    fn move_request(&mut self, surface: ToplevelSurface, seat: WlSeat, serial: Serial) {
        // Find the window wrapping this surface
        let window = self.space.elements().find(|w| w.toplevel().is_some_and(|t| *t == surface)).cloned();

        if let Some(window) = window
            && let Some(seat) = smithay::input::Seat::from_resource(&seat)
        {
            crate::grabs::handle_move_request(self, &seat, &window, serial);
        }
    }

    fn resize_request(
        &mut self,
        surface: ToplevelSurface,
        seat: WlSeat,
        serial: Serial,
        edges: xdg_toplevel::ResizeEdge,
    ) {
        let window = self.space.elements().find(|w| w.toplevel().is_some_and(|t| *t == surface)).cloned();

        if let Some(window) = window
            && let Some(seat) = smithay::input::Seat::from_resource(&seat)
        {
            let edge = match edges {
                xdg_toplevel::ResizeEdge::Top => crate::decorations::Focus::ResizeTop,
                xdg_toplevel::ResizeEdge::Bottom => crate::decorations::Focus::ResizeBottom,
                xdg_toplevel::ResizeEdge::Left => crate::decorations::Focus::ResizeLeft,
                xdg_toplevel::ResizeEdge::Right => crate::decorations::Focus::ResizeRight,
                xdg_toplevel::ResizeEdge::TopLeft => crate::decorations::Focus::ResizeTopLeft,
                xdg_toplevel::ResizeEdge::TopRight => crate::decorations::Focus::ResizeTopRight,
                xdg_toplevel::ResizeEdge::BottomLeft => crate::decorations::Focus::ResizeBottomLeft,
                xdg_toplevel::ResizeEdge::BottomRight => crate::decorations::Focus::ResizeBottomRight,
                _ => return,
            };
            crate::grabs::handle_resize_request(self, &seat, &window, edge, serial);
        }
    }

    fn grab(&mut self, surface: PopupSurface, seat: WlSeat, serial: Serial) {
        tracing::debug!(?serial, "grab: popup grab requested");

        let Some(seat) = Seat::from_resource(&seat) else {
            tracing::warn!("grab: seat not found");
            return;
        };

        let popup = PopupKind::from(surface);

        // Find the root toplevel surface for this popup chain
        let Ok(root_surface) = find_popup_root_surface(&popup) else {
            tracing::warn!("popup grab: could not find root surface");
            return;
        };

        // Find the root surface owner (Window or LayerSurface) so we can build
        // a KeyboardFocusTarget for the grab root.
        let root: Option<KeyboardFocusTarget> = self
            .space
            .elements()
            .find(|w| w.wl_surface().is_some_and(|s| *s == root_surface))
            .cloned()
            .map(KeyboardFocusTarget::Window)
            .or_else(|| {
                // Root is not a Window — try Layer-Surface (e.g. ironbar panels).
                for output in &self.outputs {
                    let map = layer_map_for_output(output);
                    if let Some(layer) =
                        map.layer_for_surface(&root_surface, smithay::desktop::WindowSurfaceType::TOPLEVEL)
                    {
                        let layer = layer.clone();
                        drop(map);
                        return Some(KeyboardFocusTarget::LayerSurface(layer));
                    }
                    drop(map);
                }
                None
            });

        let Some(root) = root else {
            tracing::warn!("popup grab: root surface not found in space or layer maps");
            return;
        };

        let mut grab = match self.popup_manager.grab_popup(root, popup, &seat, serial) {
            Ok(grab) => {
                tracing::debug!("grab: grab_popup succeeded");
                grab
            }
            Err(err) => {
                tracing::warn!(?err, "grab: grab_popup failed");
                return;
            }
        };

        if let Some(keyboard) = seat.get_keyboard() {
            if keyboard.is_grabbed()
                && !(keyboard.has_grab(serial) || keyboard.has_grab(grab.previous_serial().unwrap_or(serial)))
            {
                tracing::debug!("popup grab: keyboard already grabbed with different serial, ungrabbing");
                grab.ungrab(smithay::desktop::PopupUngrabStrategy::All);
                return;
            }
            // Set keyboard focus to the topmost popup *before* installing the
            // grab.  This sends wl_keyboard.enter to the popup surface so the
            // client knows it has keyboard focus immediately — some toolkits
            // (e.g. GTK4) depend on this to function correctly.
            let focus = grab.current_grab();
            tracing::debug!(has_focus = focus.is_some(), "grab: setting keyboard focus to popup");
            if let Some(focus) = focus {
                keyboard.set_focus(self, Some(focus), serial);
            }
            keyboard.set_grab(self, smithay::desktop::PopupKeyboardGrab::new(&grab), serial);
            tracing::debug!("grab: keyboard grab installed");
        }
        if let Some(pointer) = seat.get_pointer() {
            if pointer.is_grabbed()
                && !(pointer.has_grab(serial) || pointer.has_grab(grab.previous_serial().unwrap_or(serial)))
            {
                tracing::debug!("popup grab: pointer already grabbed with different serial, ungrabbing");
                grab.ungrab(smithay::desktop::PopupUngrabStrategy::All);
                return;
            }
            pointer.set_grab(
                self,
                smithay::desktop::PopupPointerGrab::new(&grab),
                serial,
                smithay::input::pointer::Focus::Keep,
            );
            tracing::debug!("grab: pointer grab installed");
        }

        tracing::debug!("grab: popup grab setup complete");
    }

    fn reposition_request(&mut self, surface: PopupSurface, positioner: PositionerState, token: u32) {
        surface.with_pending_state(|state| {
            state.geometry = positioner.get_geometry();
            state.positioner = positioner;
        });
        unconstrain_popup(&surface, self);
        // `send_repositioned` internally calls `send_configure_internal` which
        // sends both the `repositioned` event and the `configure` event,
        // bypassing the reactive/already-configured checks that
        // `send_configure()` enforces.  No separate configure call needed.
        surface.send_repositioned(token);
    }

    fn maximize_request(&mut self, surface: ToplevelSurface) {
        do_maximize(self, &surface);
    }

    fn unmaximize_request(&mut self, surface: ToplevelSurface) {
        do_unmaximize(self, &surface);
    }

    fn fullscreen_request(&mut self, surface: ToplevelSurface, wl_output: Option<wl_output::WlOutput>) {
        do_fullscreen(self, &surface, wl_output);
    }

    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        do_unfullscreen(self, &surface);
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        // Remove the window from the space when its toplevel is destroyed
        let window = self.space.elements().find(|w| w.toplevel().is_some_and(|t| *t == surface)).cloned();
        if let Some(ref window) = window {
            // Close foreign-toplevel handles before unmapping
            crate::handlers::foreign_toplevel::close_toplevel(self, window);

            self.space.unmap_elem(window);
            // Clean up compositor-side state for this window
            self.pre_maximize_positions.retain(|(w, _)| w != window);
            self.pre_fullscreen_states.retain(|(w, _, _)| w != window);
            self.minimized_windows.retain(|(w, _)| w != window);
        } else {
            // Window might have been minimized (not in space) — clean it up.
            // Also close any foreign-toplevel handle for minimized windows.
            let minimized = self
                .minimized_windows
                .iter()
                .find(|(w, _)| w.toplevel().is_some_and(|t| *t == surface))
                .map(|(w, _)| w.clone());
            if let Some(ref win) = minimized {
                crate::handlers::foreign_toplevel::close_toplevel(self, win);
            }
            self.minimized_windows.retain(|(w, _)| w.toplevel().is_some_and(|t| *t != surface));
        }
    }
}

/// Find the output that contains a popup's root parent surface.
///
/// Returns the output for Window-parented popups (via `output_for_window`)
/// or Layer-Surface-parented popups (via the output whose layer map contains
/// the root surface).  Returns `None` if the root surface can't be found.
pub(crate) fn find_popup_parent_output<'a>(surface: &PopupSurface, state: &'a State) -> Option<&'a Output> {
    let root_surface = find_popup_root_surface(&PopupKind::from(surface.clone())).ok()?;

    // Try Window first.
    if let Some(window) = state.space.elements().find(|w| w.wl_surface().is_some_and(|s| *s == root_surface)) {
        return Some(state.output_for_window(window));
    }

    // Try Layer Surface.
    for output in &state.outputs {
        let map = layer_map_for_output(output);
        if map.layer_for_surface(&root_surface, smithay::desktop::WindowSurfaceType::TOPLEVEL).is_some() {
            drop(map);
            return Some(output);
        }
        drop(map);
    }

    None
}

/// Constrain a popup's geometry to fit within its parent's output.
///
/// This transforms the output rectangle into the popup's parent-relative
/// coordinate system and calls `get_unconstrained_geometry` which applies
/// the positioner's flip/slide/resize constraint adjustments.
///
/// Supports both window-parented popups and layer-surface-parented popups
/// (e.g. ironbar panel menus).
pub(crate) fn unconstrain_popup(surface: &PopupSurface, state: &State) {
    let Ok(root_surface) = find_popup_root_surface(&PopupKind::from(surface.clone())) else {
        return;
    };

    // Sum of all parent popup offsets in the chain (for nested popups).
    let popup_chain_offset = get_popup_toplevel_coords(&PopupKind::from(surface.clone()));

    // Try to find the root as a Window in the space.
    if let Some((window, window_loc)) = state
        .space
        .elements()
        .find(|w| w.wl_surface().is_some_and(|s| *s == root_surface))
        .and_then(|w| state.space.element_location(w).map(|loc| (w.clone(), loc)))
    {
        // Window-parented popup — use the window's output.
        let window_geo_offset = window.geometry().loc;
        let window_origin = window_loc + window_geo_offset;
        let output = state.output_for_window(&window);
        let output_rect = state.space.output_geometry(output).unwrap_or_default();
        let target: Rectangle<i32, Logical> =
            Rectangle::new(output_rect.loc - window_origin - popup_chain_offset, output_rect.size);

        tracing::debug!(
            target_x = target.loc.x,
            target_y = target.loc.y,
            target_w = target.size.w,
            target_h = target.size.h,
            window_x = window_origin.x,
            window_y = window_origin.y,
            chain_x = popup_chain_offset.x,
            chain_y = popup_chain_offset.y,
            "unconstrain_popup: window parent constraint rect",
        );

        let geometry = surface.with_pending_state(|s| s.positioner.get_unconstrained_geometry(target));
        surface.with_pending_state(|s| {
            s.geometry = geometry;
        });
        return;
    }

    // Try to find the root as a Layer-Surface — iterate all outputs' layer maps.
    for output in &state.outputs {
        let map = layer_map_for_output(output);
        if let Some(layer) = map.layer_for_surface(&root_surface, smithay::desktop::WindowSurfaceType::TOPLEVEL) {
            let layer_geo = map.layer_geometry(layer).unwrap_or_default();
            drop(map);

            let output_geo = state.space.output_geometry(output).unwrap_or_default();
            // Layer geometry is output-local; translate to global coords.
            let layer_origin = output_geo.loc + layer_geo.loc;
            let target: Rectangle<i32, Logical> =
                Rectangle::new(output_geo.loc - layer_origin - popup_chain_offset, output_geo.size);

            tracing::debug!(
                target_x = target.loc.x,
                target_y = target.loc.y,
                target_w = target.size.w,
                target_h = target.size.h,
                layer_x = layer_origin.x,
                layer_y = layer_origin.y,
                chain_x = popup_chain_offset.x,
                chain_y = popup_chain_offset.y,
                output = output.name(),
                "unconstrain_popup: layer surface parent constraint rect",
            );

            let geometry = surface.with_pending_state(|s| s.positioner.get_unconstrained_geometry(target));
            surface.with_pending_state(|s| {
                s.geometry = geometry;
            });
            return;
        }
        drop(map);
    }

    tracing::warn!("unconstrain_popup: could not find parent surface in space or layer maps");
}

// ── Public functions for maximize/fullscreen ────────────────────────────────
//
// Extracted so that foreign-toplevel requests and other handlers can reuse
// the same logic without going through the XdgShellHandler trait methods.

/// Maximize a toplevel window (sets state, saves position, maps to usable area).
pub fn do_maximize(state: &mut State, surface: &ToplevelSurface) {
    let window = state.space.elements().find(|w| w.toplevel().is_some_and(|t| t == surface)).cloned();

    let Some(window) = window else {
        tracing::warn!("do_maximize: window not found in space");
        surface.send_configure();
        return;
    };

    // Save the current position before maximizing.
    if let Some(current_loc) = state.space.element_location(&window) {
        state.pre_maximize_positions.retain(|(w, _)| w != &window);
        state.pre_maximize_positions.push((window.clone(), current_loc));
    }

    let usable_geo = state.usable_geometry_for_window(&window);
    let output = state.output_for_window(&window);
    let output_geo = state.space.output_geometry(output);
    tracing::debug!(
        output_name = output.name(),
        ?output_geo,
        usable_x = usable_geo.loc.x,
        usable_y = usable_geo.loc.y,
        usable_w = usable_geo.size.w,
        usable_h = usable_geo.size.h,
        "do_maximize: target geometry",
    );
    surface.with_pending_state(|s| {
        s.states.set(xdg_toplevel::State::Maximized);
        s.size = Some((usable_geo.size.w, usable_geo.size.h - crate::decorations::TITLEBAR_HEIGHT).into());
    });

    // Move below titlebar if SSD, otherwise to usable area origin.
    let y_offset = if crate::decorations::window_has_ssd(&window) { crate::decorations::TITLEBAR_HEIGHT } else { 0 };
    state.space.map_element(window, (usable_geo.loc.x, usable_geo.loc.y + y_offset), true);

    surface.send_configure();
    tracing::debug!("do_maximize: window maximized");
}

/// Unmaximize a toplevel window (restores saved position).
pub fn do_unmaximize(state: &mut State, surface: &ToplevelSurface) {
    let window = state.space.elements().find(|w| w.toplevel().is_some_and(|t| t == surface)).cloned();

    let Some(window) = window else {
        tracing::warn!("do_unmaximize: window not found in space");
        return;
    };

    // Restore saved position.
    let restore_pos = state
        .pre_maximize_positions
        .iter()
        .position(|(w, _)| w == &window)
        .map(|i| state.pre_maximize_positions.remove(i).1);

    surface.with_pending_state(|s| {
        s.states.unset(xdg_toplevel::State::Maximized);
        s.size = None;
    });

    if let Some(pos) = restore_pos {
        state.space.map_element(window, pos, true);
    }

    surface.send_configure();
    tracing::debug!("do_unmaximize: window restored");
}

/// Set a toplevel window to fullscreen on the given output (or the window's current output).
pub fn do_fullscreen(state: &mut State, surface: &ToplevelSurface, wl_output: Option<wl_output::WlOutput>) {
    let window = state.space.elements().find(|w| w.toplevel().is_some_and(|t| t == surface)).cloned();

    let Some(window) = window else {
        tracing::warn!("do_fullscreen: window not found in space");
        surface.send_configure();
        return;
    };

    // Determine the target output: use the requested output, or fall back
    // to the output that best contains the window.
    let output_geo = if let Some(ref wl) = wl_output
        && let Some(output) = Output::from_resource(wl)
    {
        state.space.output_geometry(&output).unwrap_or_default()
    } else {
        state.output_geometry_for_window(&window)
    };

    // Save the current position and pending size before going fullscreen.
    let current_loc = state.space.element_location(&window).unwrap_or_default();
    let current_size = surface.with_pending_state(|s| s.size);
    state.pre_fullscreen_states.retain(|(w, _, _)| w != &window);
    state.pre_fullscreen_states.push((window.clone(), current_loc, current_size));

    // Set fullscreen state: full output size, no titlebar offset.
    surface.with_pending_state(|s| {
        s.states.set(xdg_toplevel::State::Fullscreen);
        s.size = Some(output_geo.size);
        s.fullscreen_output = wl_output;
    });

    // Move to the output origin (no titlebar).
    state.space.map_element(window, output_geo.loc, true);

    surface.send_configure();
    tracing::debug!(
        output_x = output_geo.loc.x,
        output_y = output_geo.loc.y,
        output_w = output_geo.size.w,
        output_h = output_geo.size.h,
        "do_fullscreen: window set to fullscreen",
    );
}

/// Restore a toplevel window from fullscreen.
pub fn do_unfullscreen(state: &mut State, surface: &ToplevelSurface) {
    let window = state.space.elements().find(|w| w.toplevel().is_some_and(|t| t == surface)).cloned();

    let Some(window) = window else {
        tracing::warn!("do_unfullscreen: window not found in space");
        return;
    };

    // Restore saved position and size.
    let saved = state
        .pre_fullscreen_states
        .iter()
        .position(|(w, _, _)| w == &window)
        .map(|i| state.pre_fullscreen_states.remove(i));

    surface.with_pending_state(|s| {
        s.states.unset(xdg_toplevel::State::Fullscreen);
        s.size = saved.as_ref().and_then(|(_, _, size)| *size);
        s.fullscreen_output = None;
    });

    if let Some((_, pos, _)) = saved {
        state.space.map_element(window, pos, true);
    }

    surface.send_configure();
    tracing::debug!("do_unfullscreen: window restored from fullscreen");
}
