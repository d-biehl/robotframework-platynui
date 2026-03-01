//! `xdg_shell` handler — toplevels + popups + fullscreen.

use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::{
    desktop::{PopupKind, Window, find_popup_root_surface, get_popup_toplevel_coords},
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
        crate::workspace::map_window(&mut self.space, window);
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

        // Constrain the popup to fit within the output bounds.  Without this,
        // popups may appear at wrong positions or get clipped by the output
        // edge.  The positioner's constraint_adjustment flags (flip, slide,
        // resize) are applied by `get_unconstrained_geometry`.
        if surface.get_parent_surface().is_some() {
            unconstrain_popup(&surface, &self.space, &self.output);
        }

        let final_geo = surface.with_pending_state(|state| state.geometry);
        tracing::debug!(
            x = final_geo.loc.x,
            y = final_geo.loc.y,
            w = final_geo.size.w,
            h = final_geo.size.h,
            "new_popup: final constrained geometry",
        );

        // Track the popup in the popup manager first so it's in the tree
        // before any configure/commit processing.
        let popup = PopupKind::from(surface.clone());
        if let Err(err) = self.popup_manager.track_popup(popup) {
            tracing::warn!(?err, "new_popup: failed to track popup");
        } else {
            tracing::debug!("new_popup: popup tracked successfully");
        }

        // Send wl_surface.enter(output) immediately so the client knows which
        // output the popup is on *before* it receives the configure event.
        // GTK4 uses this to determine the scale factor; without it, the popup
        // may not render until the next Space::refresh() cycle delivers the
        // enter event one frame later.
        self.output.enter(surface.wl_surface());

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

        // Find the Window that owns this root surface so we can build a
        // KeyboardFocusTarget for the grab root.
        let root = self
            .space
            .elements()
            .find(|w| w.wl_surface().is_some_and(|s| *s == root_surface))
            .cloned()
            .map(KeyboardFocusTarget::Window);

        let Some(root) = root else {
            tracing::warn!("popup grab: root window not found in space");
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
        unconstrain_popup(&surface, &self.space, &self.output);
        surface.send_repositioned(token);
        if let Err(err) = surface.send_configure() {
            tracing::warn!(?err, "reposition_request: failed to send configure");
        }
    }

    fn maximize_request(&mut self, surface: ToplevelSurface) {
        let window = self.space.elements().find(|w| w.toplevel().is_some_and(|t| *t == surface)).cloned();

        let Some(window) = window else {
            tracing::warn!("maximize_request: window not found in space");
            surface.send_configure();
            return;
        };

        // Save the current position before maximizing.
        if let Some(current_loc) = self.space.element_location(&window) {
            self.pre_maximize_positions.retain(|(w, _)| w != &window);
            self.pre_maximize_positions.push((window.clone(), current_loc));
        }

        let output_geo = self.output_geometry_for_window(&window);
        surface.with_pending_state(|s| {
            s.states.set(xdg_toplevel::State::Maximized);
            s.size = Some((output_geo.size.w, output_geo.size.h - crate::decorations::TITLEBAR_HEIGHT).into());
        });

        // Move below titlebar if SSD, otherwise to output origin.
        let y_offset =
            if crate::decorations::window_has_ssd(&window) { crate::decorations::TITLEBAR_HEIGHT } else { 0 };
        self.space.map_element(window, (output_geo.loc.x, output_geo.loc.y + y_offset), true);

        surface.send_configure();
        tracing::debug!("maximize_request: window maximized");
    }

    fn unmaximize_request(&mut self, surface: ToplevelSurface) {
        let window = self.space.elements().find(|w| w.toplevel().is_some_and(|t| *t == surface)).cloned();

        let Some(window) = window else {
            tracing::warn!("unmaximize_request: window not found in space");
            return;
        };

        // Restore saved position.
        let restore_pos = self
            .pre_maximize_positions
            .iter()
            .position(|(w, _)| w == &window)
            .map(|i| self.pre_maximize_positions.remove(i).1);

        surface.with_pending_state(|s| {
            s.states.unset(xdg_toplevel::State::Maximized);
            s.size = None;
        });

        if let Some(pos) = restore_pos {
            self.space.map_element(window, pos, true);
        }

        surface.send_configure();
        tracing::debug!("unmaximize_request: window restored");
    }

    fn fullscreen_request(&mut self, surface: ToplevelSurface, wl_output: Option<wl_output::WlOutput>) {
        let window = self.space.elements().find(|w| w.toplevel().is_some_and(|t| *t == surface)).cloned();

        let Some(window) = window else {
            tracing::warn!("fullscreen_request: window not found in space");
            surface.send_configure();
            return;
        };

        // Determine the target output: use the requested output, or fall back
        // to the output that best contains the window.
        let output_geo = if let Some(ref wl) = wl_output
            && let Some(output) = Output::from_resource(wl)
        {
            self.space.output_geometry(&output).unwrap_or_default()
        } else {
            self.output_geometry_for_window(&window)
        };

        // Save the current position and pending size before going fullscreen.
        let current_loc = self.space.element_location(&window).unwrap_or_default();
        let current_size = surface.with_pending_state(|s| s.size);
        self.pre_fullscreen_states.retain(|(w, _, _)| w != &window);
        self.pre_fullscreen_states.push((window.clone(), current_loc, current_size));

        // Set fullscreen state: full output size, no titlebar offset.
        surface.with_pending_state(|s| {
            s.states.set(xdg_toplevel::State::Fullscreen);
            s.size = Some(output_geo.size);
            s.fullscreen_output = wl_output;
        });

        // Move to the output origin (no titlebar).
        self.space.map_element(window, output_geo.loc, true);

        surface.send_configure();
        tracing::debug!(
            output_x = output_geo.loc.x,
            output_y = output_geo.loc.y,
            output_w = output_geo.size.w,
            output_h = output_geo.size.h,
            "fullscreen_request: window set to fullscreen",
        );
    }

    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        let window = self.space.elements().find(|w| w.toplevel().is_some_and(|t| *t == surface)).cloned();

        let Some(window) = window else {
            tracing::warn!("unfullscreen_request: window not found in space");
            return;
        };

        // Restore saved position and size.
        let saved = self
            .pre_fullscreen_states
            .iter()
            .position(|(w, _, _)| w == &window)
            .map(|i| self.pre_fullscreen_states.remove(i));

        surface.with_pending_state(|s| {
            s.states.unset(xdg_toplevel::State::Fullscreen);
            s.size = saved.as_ref().and_then(|(_, _, size)| *size);
            s.fullscreen_output = None;
        });

        if let Some((_, pos, _)) = saved {
            self.space.map_element(window, pos, true);
        }

        surface.send_configure();
        tracing::debug!("unfullscreen_request: window restored from fullscreen");
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        // Remove the window from the space when its toplevel is destroyed
        let window = self.space.elements().find(|w| w.toplevel().is_some_and(|t| *t == surface)).cloned();
        if let Some(window) = window {
            self.space.unmap_elem(&window);
            // Clean up compositor-side state for this window
            self.pre_maximize_positions.retain(|(w, _)| w != &window);
            self.pre_fullscreen_states.retain(|(w, _, _)| w != &window);
            self.minimized_windows.retain(|(w, _)| w != &window);
        } else {
            // Window might have been minimized (not in space) — clean it up
            self.minimized_windows.retain(|(w, _)| w.toplevel().is_some_and(|t| *t != surface));
        }
    }
}

/// Constrain a popup's geometry to fit within the output.
///
/// This transforms the output rectangle into the popup's parent-relative
/// coordinate system and calls `get_unconstrained_geometry` which applies
/// the positioner's flip/slide/resize constraint adjustments.
fn unconstrain_popup(surface: &PopupSurface, space: &smithay::desktop::Space<Window>, output: &Output) {
    let Ok(root_surface) = find_popup_root_surface(&PopupKind::from(surface.clone())) else {
        return;
    };

    // Find the window that owns the root toplevel surface.
    let Some((window, window_loc)) = space
        .elements()
        .find(|w| w.wl_surface().is_some_and(|s| *s == root_surface))
        .and_then(|w| space.element_location(w).map(|loc| (w.clone(), loc)))
    else {
        return;
    };

    // The window's geometry offset (CSD decorations shift the content origin).
    let window_geo_offset = window.geometry().loc;

    // The window's content origin in global (output) space.
    let window_origin = window_loc + window_geo_offset;

    // Sum of all parent popup offsets in the chain (for nested popups).
    let popup_chain_offset = get_popup_toplevel_coords(&PopupKind::from(surface.clone()));

    // Transform the output rect into the coordinate system relative to the
    // popup's direct parent: output_rect - window_origin - popup_chain_offset.
    let output_rect = space.output_geometry(output).unwrap_or_default();
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
        "unconstrain_popup: constraint target rect",
    );

    let geometry = surface.with_pending_state(|state| state.positioner.get_unconstrained_geometry(target));
    surface.with_pending_state(|state| {
        state.geometry = geometry;
    });
}
