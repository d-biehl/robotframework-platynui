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
    #[allow(clippy::too_many_lines)]
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

        // Grant the map request
        window.set_mapped(true).ok();

        // Wrap in a smithay Window and map into the space
        let smithay_window = Window::new_x11_window(window);
        crate::workspace::map_window(&mut self.space, smithay_window.clone());

        // X11 windows that get SSD need their position shifted down for the title bar.
        // (Wayland windows get this in XdgDecorationHandler::new_decoration, but X11
        // windows don't participate in xdg-decoration negotiation.)
        if crate::decorations::window_has_ssd(&smithay_window)
            && let Some(loc) = self.space.element_location(&smithay_window)
        {
            self.space.map_element(smithay_window, (loc.x, loc.y + crate::decorations::TITLEBAR_HEIGHT), false);
        }
    }

    fn map_window_notify(&mut self, _xwm: XwmId, _window: X11Surface) {
        // Notification that mapping succeeded — nothing extra needed
    }

    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, window: X11Surface) {
        // Override-redirect windows (menus, tooltips) — map them directly
        let smithay_window = Window::new_x11_window(window);
        crate::workspace::map_window(&mut self.space, smithay_window);
    }

    fn unmapped_window(&mut self, _xwm: XwmId, window: X11Surface) {
        tracing::debug!(title = window.title(), "X11 window unmapped");

        // Find and remove the window from our space
        let element = self
            .space
            .elements()
            .find(|w| w.x11_surface().is_some_and(|x| x.window_id() == window.window_id()))
            .cloned();

        if let Some(element) = element {
            self.pre_fullscreen_states.retain(|(w, _, _)| w != &element);
            self.space.unmap_elem(&element);
        }
    }

    fn destroyed_window(&mut self, _xwm: XwmId, window: X11Surface) {
        tracing::debug!(title = window.title(), "X11 window destroyed");

        let element = self
            .space
            .elements()
            .find(|w| w.x11_surface().is_some_and(|x| x.window_id() == window.window_id()))
            .cloned();

        if let Some(element) = element {
            self.pre_fullscreen_states.retain(|(w, _, _)| w != &element);
            self.space.unmap_elem(&element);
        }
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
        // Honor the client's configure request for position/size
        let geo = window.geometry();
        let new_x = x.unwrap_or(geo.loc.x);
        let new_y = y.unwrap_or(geo.loc.y);
        let new_w = w.unwrap_or(geo.size.w.unsigned_abs());
        let new_h = h.unwrap_or(geo.size.h.unsigned_abs());

        let _ =
            window.configure(Rectangle::new((new_x, new_y).into(), (new_w.cast_signed(), new_h.cast_signed()).into()));
    }

    fn configure_notify(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        geometry: Rectangle<i32, Logical>,
        _above: Option<X11Window>,
    ) {
        // Update the window position in our space if it was reconfigured
        let element = self
            .space
            .elements()
            .find(|w| w.x11_surface().is_some_and(|x| x.window_id() == window.window_id()))
            .cloned();

        if let Some(element) = element {
            self.space.map_element(element, geometry.loc, true);
        }
    }

    fn resize_request(&mut self, _xwm: XwmId, window: X11Surface, _button: u32, resize_edge: ResizeEdge) {
        let Some(element) = self
            .space
            .elements()
            .find(|w| w.x11_surface().is_some_and(|x| x.window_id() == window.window_id()))
            .cloned()
        else {
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
        let keyboard = self.seat.get_keyboard().unwrap();
        keyboard.set_focus(self, Some(crate::focus::KeyboardFocusTarget::Window(element.clone())), serial);
        self.space.raise_element(&element, true);
        crate::grabs::handle_resize_request(self, &self.seat.clone(), &element, edge, serial);
    }

    fn move_request(&mut self, _xwm: XwmId, window: X11Surface, _button: u32) {
        let Some(element) = self
            .space
            .elements()
            .find(|w| w.x11_surface().is_some_and(|x| x.window_id() == window.window_id()))
            .cloned()
        else {
            return;
        };

        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        let keyboard = self.seat.get_keyboard().unwrap();
        keyboard.set_focus(self, Some(crate::focus::KeyboardFocusTarget::Window(element.clone())), serial);
        self.space.raise_element(&element, true);
        crate::grabs::handle_move_request(self, &self.seat.clone(), &element, serial);
    }

    fn fullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let Some(element) = self
            .space
            .elements()
            .find(|w| w.x11_surface().is_some_and(|x| x.window_id() == window.window_id()))
            .cloned()
        else {
            return;
        };

        let output_geo = self.output_geometry_for_window(&element);

        // Save the current position and size before going fullscreen.
        let current_loc = self.space.element_location(&element).unwrap_or_default();
        let current_size = Some(window.geometry().size);
        self.pre_fullscreen_states.retain(|(w, _, _)| w != &element);
        self.pre_fullscreen_states.push((element.clone(), current_loc, current_size));

        // Set fullscreen and resize to output dimensions.
        let _ = window.set_fullscreen(true);
        let _ = window.configure(output_geo);
        self.space.map_element(element, output_geo.loc, true);

        tracing::debug!(output_w = output_geo.size.w, output_h = output_geo.size.h, "X11 window set to fullscreen",);
    }

    fn unfullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let Some(element) = self
            .space
            .elements()
            .find(|w| w.x11_surface().is_some_and(|x| x.window_id() == window.window_id()))
            .cloned()
        else {
            return;
        };

        // Restore saved position and size.
        let saved = self
            .pre_fullscreen_states
            .iter()
            .position(|(w, _, _)| w == &element)
            .map(|i| self.pre_fullscreen_states.remove(i));

        let _ = window.set_fullscreen(false);

        if let Some((_, pos, size)) = saved {
            if let Some(size) = size {
                let _ = window.configure(Rectangle::new(pos, size));
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
