//! `XWayland` integration — runs an X11 server for legacy X11 applications.
//!
//! When the `xwayland` feature is enabled, the compositor can spawn an `XWayland`
//! process. X11 windows are mapped into the same `Space<Window>` as Wayland
//! windows. The `XwmHandler` implementation handles window lifecycle events.

use std::ffi::OsString;
use std::os::unix::io::OwnedFd;
use std::process::Stdio;

use smithay::{
    desktop::Window,
    utils::{Logical, Rectangle},
    wayland::{
        selection::SelectionTarget,
        xwayland_shell::{XWaylandShellHandler, XWaylandShellState},
    },
    xwayland::{
        X11Surface, X11Wm, XWayland, XWaylandEvent, XwmHandler,
        xwm::{Reorder, ResizeEdge, X11Window, XwmId},
    },
};

use crate::state::State;

/// Find the smithay `Window` in the space that wraps a given [`X11Surface`].
fn find_x11_window(state: &State, surface: &X11Surface) -> Option<Window> {
    state.space.elements().find(|w| w.x11_surface().is_some_and(|x| x.window_id() == surface.window_id())).cloned()
}

/// Remove an X11 window from the space and close its foreign-toplevel handles.
fn remove_x11_window(state: &mut State, surface: &X11Surface) {
    if let Some(element) = find_x11_window(state, surface) {
        crate::handlers::foreign_toplevel::close_toplevel(state, &element);
        state.pre_maximize_positions.retain(|(w, _, _)| w != &element);
        state.pre_fullscreen_states.retain(|(w, _, _)| w != &element);
        state.space.unmap_elem(&element);
    }
}

/// Optional `XWayland` state, present only when `XWayland` is running.
pub struct XWaylandState {
    /// The X11 window manager connection.
    pub wm: Option<X11Wm>,
    /// The `XWayland` client handle — needed by `X11Wm::start_wm`.
    pub client: Option<smithay::reexports::wayland_server::Client>,
    /// The display number `XWayland` is using (e.g., `:1`).
    pub display: u32,
}

impl State {
    /// Start `XWayland` if the feature is enabled and `Xwayland` binary is available.
    ///
    /// Registers `XWayland` as a calloop event source. When `XWayland` signals readiness,
    /// the `X11Wm` is created and stored in `state.xwayland`.
    ///
    /// # Panics
    ///
    /// Panics if the `XWayland` client handle was not stored before the ready event.
    pub fn start_xwayland(&mut self) {
        let (xwayland, client) = match XWayland::spawn(
            &self.display_handle,
            None,
            std::iter::empty::<(OsString, OsString)>(),
            true,
            Stdio::null(),
            Stdio::null(),
            |_| (),
        ) {
            Ok(result) => result,
            Err(err) => {
                tracing::warn!(%err, "failed to spawn XWayland — X11 apps will not work");
                return;
            }
        };

        // Store the client temporarily — we need it when the WM starts
        self.xwayland = Some(XWaylandState { wm: None, client: Some(client), display: 0 });

        if let Err(err) = self.loop_handle.insert_source(xwayland, move |event, (), state| match event {
            XWaylandEvent::Ready { x11_socket, display_number, .. } => {
                tracing::info!(display = display_number, "XWayland ready");

                // Set DISPLAY for child processes
                let display_value = format!(":{display_number}");
                #[allow(unsafe_code)]
                // SAFETY: Called from the single-threaded event loop.
                unsafe {
                    std::env::set_var("DISPLAY", &display_value);
                }

                // Print DISPLAY via deferred readiness notification
                if state.print_env {
                    println!("DISPLAY={display_value}");
                }
                // Send deferred readiness notification (was waiting for XWayland)
                crate::ready::notify_ready(&state.socket_name, state.ready_fd.take(), state.print_env);

                // Spawn child program now that both Wayland and XWayland are ready
                state.spawn_child_if_requested();

                // Retrieve the stored client
                let client = state.xwayland.as_mut().and_then(|s| s.client.take()).expect("XWayland client not stored");

                match X11Wm::start_wm(state.loop_handle.clone(), x11_socket, client) {
                    Ok(wm) => {
                        if let Some(ref mut xw_state) = state.xwayland {
                            xw_state.wm = Some(wm);
                            xw_state.display = display_number;
                        }
                    }
                    Err(err) => {
                        tracing::error!(%err, "failed to start X11 window manager");
                    }
                }
            }
            XWaylandEvent::Error => {
                tracing::warn!("XWayland terminated unexpectedly");
                state.xwayland = None;
            }
        }) {
            tracing::warn!(%err, "failed to register XWayland event source");
        }
    }
}

// -- XWaylandShellHandler --

impl XWaylandShellHandler for State {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        self.xwayland_shell_state.as_mut().expect("XWaylandShellState not initialized")
    }
}

smithay::delegate_xwayland_shell!(State);

// -- XwmHandler --

impl XwmHandler for State {
    fn xwm_state(&mut self, _xwm: XwmId) -> &mut X11Wm {
        self.xwayland.as_mut().and_then(|s| s.wm.as_mut()).expect("`XWayland` WM not initialized")
    }

    fn new_window(&mut self, _xwm: XwmId, _window: X11Surface) {
        // Window created but not yet requesting to be mapped — nothing to do
    }

    fn new_override_redirect_window(&mut self, _xwm: XwmId, _window: X11Surface) {
        // Override-redirect windows (tooltips, menus) — tracked when mapped
    }

    fn map_window_request(&mut self, _xwm: XwmId, window: X11Surface) {
        tracing::debug!(
            title = window.title(),
            class = ?window.class(),
            override_redirect = window.is_override_redirect(),
            is_decorated = window.is_decorated(),
            "X11 window map request",
        );

        if let Err(err) = window.set_mapped(true) {
            tracing::warn!(%err, "failed to set X11 window as mapped");
        }

        let smithay_window = Window::new_x11_window(window);
        let usable_origin = self.usable_geometry_for_output(&self.output.clone()).loc;
        crate::workspace::map_window(&mut self.space, smithay_window.clone(), usable_origin);

        // X11 windows that get SSD need their position shifted down for the title bar.
        // (Wayland windows get this in XdgDecorationHandler::new_decoration, but X11
        // windows don't participate in xdg-decoration negotiation.)
        if crate::decorations::window_has_ssd(&smithay_window)
            && let Some(loc) = self.space.element_location(&smithay_window)
        {
            self.space.map_element(smithay_window.clone(), (loc.x, loc.y + crate::decorations::TITLEBAR_HEIGHT), false);
        }

        // Tell the X11 client its actual compositor position so it can
        // calculate correct screen coordinates for override-redirect
        // windows (menus, tooltips, dropdowns).  Without this, the client
        // still thinks it is at its initial (often 0,0) position and any
        // popup it opens ends up in the wrong place.
        if let Some(x11) = smithay_window.x11_surface()
            && let Some(loc) = self.space.element_location(&smithay_window)
        {
            let size = x11.geometry().size;
            if let Err(err) = x11.configure(Rectangle::new(loc, size)) {
                tracing::warn!(%err, "failed to configure X11 window position after mapping");
            }
        }

        // Announce to foreign-toplevel protocols
        crate::handlers::foreign_toplevel::announce_new_toplevel(self, &smithay_window);
    }

    fn map_window_notify(&mut self, _xwm: XwmId, _window: X11Surface) {
        // Notification that mapping succeeded — nothing extra needed
    }

    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, window: X11Surface) {
        // Override-redirect windows (menus, tooltips, dropdowns) set their own
        // position in X11 screen coordinates — honour that instead of cascading.
        let geo = window.geometry();
        tracing::debug!(
            x = geo.loc.x,
            y = geo.loc.y,
            w = geo.size.w,
            h = geo.size.h,
            "mapping override-redirect X11 window at its requested position",
        );
        let smithay_window = Window::new_x11_window(window);
        self.space.map_element(smithay_window, geo.loc, false);
    }

    fn unmapped_window(&mut self, _xwm: XwmId, window: X11Surface) {
        tracing::debug!(title = window.title(), "X11 window unmapped");
        remove_x11_window(self, &window);
    }

    fn destroyed_window(&mut self, _xwm: XwmId, window: X11Surface) {
        tracing::debug!(title = window.title(), "X11 window destroyed");
        remove_x11_window(self, &window);
    }

    fn configure_request(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        x: Option<i32>,
        y: Option<i32>,
        w: Option<u32>,
        h: Option<u32>,
        _reorder: Option<Reorder>,
    ) {
        let geo = window.geometry();
        let is_mapped = find_x11_window(self, &window).is_some();

        if is_mapped && !window.is_override_redirect() {
            // Mapped regular windows: allow resize but preserve the compositor-
            // managed position.  Letting the client move itself would desync
            // the Space element location and break popup coordinate math.
            let new_w = w.map_or(geo.size.w, u32::cast_signed);
            let new_h = h.map_or(geo.size.h, u32::cast_signed);

            if (new_w, new_h) == (geo.size.w, geo.size.h) {
                // Ack with current state — no change.
                if let Err(err) = window.configure(geo) {
                    tracing::warn!(%err, "failed to ack X11 configure request");
                }
            } else if let Err(err) = window.configure(Rectangle::new(geo.loc, (new_w, new_h).into())) {
                tracing::warn!(%err, "failed to configure X11 window resize");
            }
        } else {
            // Unmapped windows and override-redirect windows: grant whatever
            // the client requests (they manage their own position).
            let new_x = x.unwrap_or(geo.loc.x);
            let new_y = y.unwrap_or(geo.loc.y);
            let new_w = w.unwrap_or(geo.size.w.unsigned_abs());
            let new_h = h.unwrap_or(geo.size.h.unsigned_abs());

            if let Err(err) = window
                .configure(Rectangle::new((new_x, new_y).into(), (new_w.cast_signed(), new_h.cast_signed()).into()))
            {
                tracing::warn!(%err, "failed to configure X11 window");
            }
        }
    }

    fn configure_notify(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        geometry: Rectangle<i32, Logical>,
        _above: Option<X11Window>,
    ) {
        // Update the window position in our space if it was reconfigured
        if let Some(element) = find_x11_window(self, &window) {
            self.space.map_element(element, geometry.loc, true);
        }
    }

    fn resize_request(&mut self, _xwm: XwmId, window: X11Surface, _button: u32, resize_edge: ResizeEdge) {
        let Some(element) = find_x11_window(self, &window) else {
            return;
        };

        let edge = match resize_edge {
            ResizeEdge::Top => crate::decorations::Focus::ResizeTop,
            ResizeEdge::Bottom => crate::decorations::Focus::ResizeBottom,
            ResizeEdge::Left => crate::decorations::Focus::ResizeLeft,
            ResizeEdge::Right => crate::decorations::Focus::ResizeRight,
            ResizeEdge::TopLeft => crate::decorations::Focus::ResizeTopLeft,
            ResizeEdge::TopRight => crate::decorations::Focus::ResizeTopRight,
            ResizeEdge::BottomLeft => crate::decorations::Focus::ResizeBottomLeft,
            ResizeEdge::BottomRight => crate::decorations::Focus::ResizeBottomRight,
        };

        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        let keyboard = self.keyboard();
        keyboard.set_focus(self, Some(crate::focus::KeyboardFocusTarget::Window(element.clone())), serial);
        self.space.raise_element(&element, true);
        crate::grabs::handle_resize_request(self, &self.seat.clone(), &element, edge, serial);
    }

    fn move_request(&mut self, _xwm: XwmId, window: X11Surface, _button: u32) {
        let Some(element) = find_x11_window(self, &window) else {
            return;
        };

        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        let keyboard = self.keyboard();
        keyboard.set_focus(self, Some(crate::focus::KeyboardFocusTarget::Window(element.clone())), serial);
        self.space.raise_element(&element, true);
        crate::grabs::handle_move_request(self, &self.seat.clone(), &element, serial);
    }

    fn maximize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let Some(element) = find_x11_window(self, &window) else {
            return;
        };

        if window.is_maximized() {
            tracing::debug!(title = window.title(), "X11 maximize_request: already maximized");
            return;
        }

        // Save current position and size before maximizing.
        let current_loc = self.space.element_location(&element).unwrap_or_default();
        let current_size = Some(window.geometry().size);
        self.pre_maximize_positions.retain(|(w, _, _)| w != &element);
        self.pre_maximize_positions.push((element.clone(), current_loc, current_size));

        // Maximize to usable area minus titlebar.
        let usable_geo = self.usable_geometry_for_window(&element);
        let y_offset =
            if crate::decorations::window_has_ssd(&element) { crate::decorations::TITLEBAR_HEIGHT } else { 0 };
        let max_size = smithay::utils::Size::from((usable_geo.size.w, usable_geo.size.h - y_offset));

        if let Err(err) = window.set_maximized(true) {
            tracing::warn!(%err, "failed to maximize X11 window");
        }
        if let Err(err) =
            window.configure(Rectangle::new((usable_geo.loc.x, usable_geo.loc.y + y_offset).into(), max_size))
        {
            tracing::warn!(%err, "failed to configure X11 window for maximize");
        }
        self.space.map_element(element, (usable_geo.loc.x, usable_geo.loc.y + y_offset), true);

        tracing::debug!(title = window.title(), "X11 window maximized via _NET_WM_STATE");
    }

    fn unmaximize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let Some(element) = find_x11_window(self, &window) else {
            return;
        };

        if !window.is_maximized() {
            tracing::debug!(title = window.title(), "X11 unmaximize_request: not maximized");
            return;
        }

        // Restore saved position and size.
        let saved = self
            .pre_maximize_positions
            .iter()
            .position(|(w, _, _)| w == &element)
            .map(|i| self.pre_maximize_positions.remove(i));

        if let Err(err) = window.set_maximized(false) {
            tracing::warn!(%err, "failed to unmaximize X11 window");
        }

        if let Some((_, pos, size)) = saved {
            if let Err(err) = window.configure(size.map(|s| Rectangle::new(pos, s))) {
                tracing::warn!(%err, "failed to configure X11 window after unmaximize");
            }
            self.space.map_element(element, pos, true);
        } else if let Err(err) = window.configure(None) {
            tracing::warn!(%err, "failed to configure X11 window after unmaximize");
        }

        tracing::debug!(title = window.title(), "X11 window unmaximized via _NET_WM_STATE");
    }

    fn fullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let Some(element) = find_x11_window(self, &window) else {
            return;
        };

        let output_geo = self.output_geometry_for_window(&element);

        // Save the current position and size before going fullscreen.
        let current_loc = self.space.element_location(&element).unwrap_or_default();
        let current_size = Some(window.geometry().size);
        self.pre_fullscreen_states.retain(|(w, _, _)| w != &element);
        self.pre_fullscreen_states.push((element.clone(), current_loc, current_size));

        // Set fullscreen and resize to output dimensions.
        if let Err(err) = window.set_fullscreen(true) {
            tracing::warn!(%err, "failed to set X11 window fullscreen");
        }
        if let Err(err) = window.configure(output_geo) {
            tracing::warn!(%err, "failed to configure X11 window for fullscreen");
        }
        self.space.map_element(element, output_geo.loc, true);

        tracing::debug!(output_w = output_geo.size.w, output_h = output_geo.size.h, "X11 window set to fullscreen",);
    }

    fn unfullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let Some(element) = find_x11_window(self, &window) else {
            return;
        };

        // Restore saved position and size.
        let saved = self
            .pre_fullscreen_states
            .iter()
            .position(|(w, _, _)| w == &element)
            .map(|i| self.pre_fullscreen_states.remove(i));

        if let Err(err) = window.set_fullscreen(false) {
            tracing::warn!(%err, "failed to unset X11 window fullscreen");
        }

        if let Some((_, pos, size)) = saved {
            if let Some(size) = size
                && let Err(err) = window.configure(Rectangle::new(pos, size))
            {
                tracing::warn!(%err, "failed to configure X11 window after unfullscreen");
            }
            self.space.map_element(element, pos, true);
        }

        tracing::debug!("X11 window restored from fullscreen");
    }

    fn allow_selection_access(&mut self, _xwm: XwmId, _selection: SelectionTarget) -> bool {
        // Allow X11 clients to access clipboard
        true
    }

    fn send_selection(&mut self, _xwm: XwmId, selection: SelectionTarget, mime_type: String, fd: OwnedFd) {
        // Forward clipboard from Wayland to X11
        match selection {
            SelectionTarget::Clipboard => {
                if let Err(err) = smithay::wayland::selection::data_device::request_data_device_client_selection(
                    &self.seat, mime_type, fd,
                ) {
                    tracing::warn!(%err, "failed to send clipboard selection to XWayland");
                }
            }
            SelectionTarget::Primary => {
                if let Err(err) = smithay::wayland::selection::primary_selection::request_primary_client_selection(
                    &self.seat, mime_type, fd,
                ) {
                    tracing::warn!(%err, "failed to send primary selection to XWayland");
                }
            }
        }
    }

    fn new_selection(&mut self, _xwm: XwmId, selection: SelectionTarget, mime_types: Vec<String>) {
        tracing::debug!(?selection, ?mime_types, "X11 new selection");
        // X11 client set a new selection — propagate to Wayland clients
        match selection {
            SelectionTarget::Clipboard => {
                smithay::wayland::selection::data_device::set_data_device_selection(
                    &self.display_handle,
                    &self.seat,
                    mime_types,
                    (),
                );
            }
            SelectionTarget::Primary => {
                smithay::wayland::selection::primary_selection::set_primary_selection(
                    &self.display_handle,
                    &self.seat,
                    mime_types,
                    (),
                );
            }
        }
    }

    fn cleared_selection(&mut self, _xwm: XwmId, selection: SelectionTarget) {
        match selection {
            SelectionTarget::Clipboard => {
                smithay::wayland::selection::data_device::clear_data_device_selection(&self.display_handle, &self.seat);
            }
            SelectionTarget::Primary => {
                smithay::wayland::selection::primary_selection::clear_primary_selection(
                    &self.display_handle,
                    &self.seat,
                );
            }
        }
    }
}
