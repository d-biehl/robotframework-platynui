//! Pointer grabs for interactive window move and resize.

use smithay::{
    desktop::Window,
    input::{
        SeatHandler,
        pointer::{
            AxisFrame, ButtonEvent, Focus as GrabFocus, GestureHoldBeginEvent, GestureHoldEndEvent,
            GesturePinchBeginEvent, GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent,
            GestureSwipeEndEvent, GestureSwipeUpdateEvent, GrabStartData, MotionEvent, PointerGrab, PointerInnerHandle,
            RelativeMotionEvent,
        },
    },
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel,
    utils::{Logical, Point, Size},
};

use crate::{decorations::Focus, input::BTN_LEFT, state::State};

/// Minimum window width during interactive resize (logical pixels).
const MIN_WINDOW_WIDTH: i32 = 100;
/// Minimum window height during interactive resize (logical pixels).
const MIN_WINDOW_HEIGHT: i32 = 50;

/// Implement the gesture pass-through methods of [`PointerGrab`] for a grab type.
///
/// All gesture events are simply forwarded to the underlying pointer handle,
/// which is the correct behaviour for grabs that only care about motion/button.
macro_rules! impl_grab_gesture_passthrough {
    () => {
        fn gesture_swipe_begin(
            &mut self,
            data: &mut State,
            handle: &mut PointerInnerHandle<'_, State>,
            event: &GestureSwipeBeginEvent,
        ) {
            handle.gesture_swipe_begin(data, event);
        }

        fn gesture_swipe_update(
            &mut self,
            data: &mut State,
            handle: &mut PointerInnerHandle<'_, State>,
            event: &GestureSwipeUpdateEvent,
        ) {
            handle.gesture_swipe_update(data, event);
        }

        fn gesture_swipe_end(
            &mut self,
            data: &mut State,
            handle: &mut PointerInnerHandle<'_, State>,
            event: &GestureSwipeEndEvent,
        ) {
            handle.gesture_swipe_end(data, event);
        }

        fn gesture_pinch_begin(
            &mut self,
            data: &mut State,
            handle: &mut PointerInnerHandle<'_, State>,
            event: &GesturePinchBeginEvent,
        ) {
            handle.gesture_pinch_begin(data, event);
        }

        fn gesture_pinch_update(
            &mut self,
            data: &mut State,
            handle: &mut PointerInnerHandle<'_, State>,
            event: &GesturePinchUpdateEvent,
        ) {
            handle.gesture_pinch_update(data, event);
        }

        fn gesture_pinch_end(
            &mut self,
            data: &mut State,
            handle: &mut PointerInnerHandle<'_, State>,
            event: &GesturePinchEndEvent,
        ) {
            handle.gesture_pinch_end(data, event);
        }

        fn gesture_hold_begin(
            &mut self,
            data: &mut State,
            handle: &mut PointerInnerHandle<'_, State>,
            event: &GestureHoldBeginEvent,
        ) {
            handle.gesture_hold_begin(data, event);
        }

        fn gesture_hold_end(
            &mut self,
            data: &mut State,
            handle: &mut PointerInnerHandle<'_, State>,
            event: &GestureHoldEndEvent,
        ) {
            handle.gesture_hold_end(data, event);
        }
    };
}

/// Compute the Y coordinate of the restored window so that the cursor stays
/// inside the titlebar.
///
/// - **SSD**: The titlebar is drawn *above* the element origin, from
///   `element_y − TITLEBAR_HEIGHT` to `element_y`.  We place the element so
///   the cursor lands in the centre of that band:
///   `element_y = cursor_y + TITLEBAR_HEIGHT / 2`.
/// - **CSD**: The titlebar is part of the client surface starting at the
///   element origin.  We place the element so the cursor is roughly in the
///   vertical centre of a typical CSD titlebar (~30 px):
///   `element_y = cursor_y − TITLEBAR_HEIGHT / 2`.
fn unmaximize_y(cursor_y: f64, window: &Window) -> i32 {
    #[allow(clippy::cast_possible_truncation)]
    if crate::decorations::window_has_ssd(window) {
        // SSD: titlebar is above the element → push element below cursor.
        (cursor_y + f64::from(crate::decorations::TITLEBAR_HEIGHT) / 2.0) as i32
    } else {
        // CSD: titlebar starts at element origin → push element above cursor.
        (cursor_y - f64::from(crate::decorations::TITLEBAR_HEIGHT) / 2.0) as i32
    }
}

/// State saved when a move grab starts on a maximized window.  The actual
/// unmaximize is deferred until the pointer moves — a simple click-and-release
/// on the titlebar must NOT restore the window.
struct MaximizedMoveState {
    /// Location of the maximized window in the space.
    location: Point<i32, Logical>,
    /// Width of the maximized window (from toplevel state or X11 geometry).
    width: i32,
}

/// A pointer grab that moves a window interactively.
///
/// When the window was maximized at grab start, [`maximized`] holds the
/// pre-unmaximize geometry.  The first `motion` event triggers the actual
/// restore so that a plain click on the titlebar does not unmaximize.
pub struct MoveSurfaceGrab {
    pub start_data: GrabStartData<State>,
    pub window: Window,
    pub initial_window_location: Point<i32, Logical>,
    /// `Some` while the window is still maximized and waiting for the first
    /// pointer motion to trigger unmaximize.
    maximized: Option<MaximizedMoveState>,
}

impl PointerGrab<State> for MoveSurfaceGrab {
    #[allow(clippy::cast_possible_truncation)]
    fn motion(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        _focus: Option<(<State as SeatHandler>::PointerFocus, Point<f64, Logical>)>,
        event: &MotionEvent,
    ) {
        // No focus during grab — the grabbed window tracks the pointer directly
        handle.motion(data, None, event);

        // Deferred unmaximize: the first motion event after grab start triggers
        // the actual restore so that a simple click-release on the titlebar
        // does not accidentally unmaximize the window.
        if let Some(max_state) = self.maximized.take() {
            let pointer_x = event.location.x;
            let maximized_width = f64::from(max_state.width.max(1));

            // Proportion of cursor across the maximized window (0.0 .. 1.0).
            let ratio = ((pointer_x - f64::from(max_state.location.x)) / maximized_width).clamp(0.0, 1.0);

            // --- Wayland toplevel ---
            if let Some(toplevel) = self.window.toplevel() {
                // Consume the saved pre-maximize position entry (no longer needed).
                if let Some(idx) = data.pre_maximize_positions.iter().position(|(w, _)| w == &self.window) {
                    data.pre_maximize_positions.remove(idx);
                }

                toplevel.with_pending_state(|s| {
                    s.states.unset(xdg_toplevel::State::Maximized);
                    s.size = None;
                });
                toplevel.send_configure();

                // Restored width is unknown until the client commits; use half
                // the maximized width as a reasonable heuristic.
                let restored_width = f64::from((max_state.width / 2).max(1));
                let new_x = (pointer_x - restored_width * ratio) as i32;
                let new_y = unmaximize_y(event.location.y, &self.window);
                let new_loc: Point<i32, Logical> = (new_x, new_y).into();

                data.space.map_element(self.window.clone(), new_loc, true);

                // Reset grab anchors so subsequent deltas are relative to the
                // new (restored) window position and the current cursor.
                self.initial_window_location = new_loc;
                self.start_data = GrabStartData {
                    focus: self.start_data.focus.clone(),
                    button: self.start_data.button,
                    location: event.location,
                };

                tracing::debug!(?new_loc, ratio, restored_width, "move_grab: deferred unmaximize (Wayland)");
                return;
            }

            // --- X11 ---
            if let Some(x11) = self.window.x11_surface() {
                // Consume the saved pre-maximize position entry (no longer needed).
                if let Some(idx) = data.pre_maximize_positions.iter().position(|(w, _)| w == &self.window) {
                    data.pre_maximize_positions.remove(idx);
                }

                if let Err(err) = x11.set_maximized(false) {
                    tracing::warn!(%err, "failed to unmaximize X11 window during drag");
                }
                if let Err(err) = x11.configure(None) {
                    tracing::warn!(%err, "failed to configure X11 window during drag");
                }

                let restored_width = f64::from((max_state.width / 2).max(1));
                let new_x = (pointer_x - restored_width * ratio) as i32;
                let new_y = unmaximize_y(event.location.y, &self.window);
                let new_loc: Point<i32, Logical> = (new_x, new_y).into();

                data.space.map_element(self.window.clone(), new_loc, true);
                self.initial_window_location = new_loc;
                self.start_data = GrabStartData {
                    focus: self.start_data.focus.clone(),
                    button: self.start_data.button,
                    location: event.location,
                };

                tracing::debug!(?new_loc, ratio, restored_width, "move_grab: deferred unmaximize (X11)");
                return;
            }
        }

        let delta = event.location - self.start_data.location;
        let prev_location = self.start_data.location;

        // Always update the grab anchor to the current pointer position so
        // that the next frame's delta is just the frame-to-frame motion.
        // This converts the grab from "absolute" (initial + total_delta)
        // to "incremental" (position += frame_delta) which is mathematically
        // equivalent for continuous motion but crucial for dead-zone handling.
        self.start_data = GrabStartData {
            focus: self.start_data.focus.clone(),
            button: self.start_data.button,
            location: event.location,
        };

        // Only move the window when the pointer is on a valid output AND
        // was on a valid output in the previous frame.  This prevents:
        // - Dragging windows into dead zones (L-shaped multi-monitor gaps)
        // - A sudden jump when transitioning from dead zone back to an
        //   output (the transition frame's delta would span the entire zone)
        let on_output = data.point_in_any_output(event.location);
        let was_on_output = data.point_in_any_output(prev_location);

        if on_output && was_on_output {
            let new_location = self.initial_window_location + Point::from((delta.x as i32, delta.y as i32));
            self.initial_window_location = new_location;
            data.space.map_element(self.window.clone(), new_location, true);
        }
    }

    fn relative_motion(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        focus: Option<(<State as SeatHandler>::PointerFocus, Point<f64, Logical>)>,
        event: &RelativeMotionEvent,
    ) {
        handle.relative_motion(data, focus, event);
    }

    fn button(&mut self, data: &mut State, handle: &mut PointerInnerHandle<'_, State>, event: &ButtonEvent) {
        handle.button(data, event);

        // End the grab when the initiating button is released
        if !handle.current_pressed().contains(&self.start_data.button) {
            handle.unset_grab(self, data, event.serial, event.time, true);
        }
    }

    fn axis(&mut self, data: &mut State, handle: &mut PointerInnerHandle<'_, State>, details: AxisFrame) {
        handle.axis(data, details);
    }

    fn frame(&mut self, data: &mut State, handle: &mut PointerInnerHandle<'_, State>) {
        handle.frame(data);
    }

    impl_grab_gesture_passthrough!();

    fn start_data(&self) -> &GrabStartData<State> {
        &self.start_data
    }

    fn unset(&mut self, data: &mut State) {
        // After a move, tell X11 clients their new position so they can
        // calculate correct screen coordinates for override-redirect
        // windows (menus, tooltips).
        if let Some(x11) = self.window.x11_surface()
            && let Some(loc) = data.space.element_location(&self.window)
        {
            let size = x11.geometry().size;
            if let Err(err) =
                x11.configure(smithay::utils::Rectangle::new(loc, size))
            {
                tracing::warn!(%err, "failed to configure X11 window after move");
            }
        }
    }
}

/// Try to start a window move grab. Returns `true` if the grab was initiated.
///
/// If the window is currently maximized the grab records the maximized geometry
/// but does **not** unmaximize yet — that is deferred to the first pointer
/// motion event inside `MoveSurfaceGrab::motion()`.  This way a simple
/// click-and-release on the titlebar does not accidentally restore the window.
pub fn handle_move_request(
    data: &mut State,
    seat: &smithay::input::Seat<State>,
    window: &Window,
    serial: smithay::utils::Serial,
) -> bool {
    let Some(pointer) = seat.get_pointer() else {
        return false;
    };

    // Verify the serial matches a recent pointer button press
    if !pointer.has_grab(serial) {
        // Fall back: allow the grab anyway (some clients send stale serials)
        tracing::debug!("move_request with non-matching serial, allowing anyway");
    }

    let start_data = pointer.grab_start_data().unwrap_or_else(|| GrabStartData {
        focus: None,
        button: BTN_LEFT,
        location: pointer.current_location(),
    });

    let initial_window_location = data.space.element_location(window).unwrap_or_default();

    // Detect maximized state *without* unmaximizing — store it for deferred
    // restore on first motion.
    let maximized = detect_maximized_state(window, initial_window_location);

    // Set compositor cursor so the move cursor persists even if the client
    // resets its cursor on pointer-leave (caused by GrabFocus::Clear).
    data.compositor_cursor_shape = crate::decorations::CursorShape::Move;

    let grab = MoveSurfaceGrab { start_data, window: window.clone(), initial_window_location, maximized };

    pointer.set_grab(data, grab, serial, GrabFocus::Clear);
    true
}

/// Check whether `window` is maximized and return the saved geometry if so.
fn detect_maximized_state(window: &Window, location: Point<i32, Logical>) -> Option<MaximizedMoveState> {
    // Wayland toplevel
    if let Some(toplevel) = window.toplevel()
        && toplevel.current_state().states.contains(xdg_toplevel::State::Maximized)
    {
        let width = toplevel.current_state().size.map_or(0, |s| s.w);
        return Some(MaximizedMoveState { location, width });
    }
    // X11
    if let Some(x11) = window.x11_surface()
        && x11.is_maximized()
    {
        let width = x11.geometry().size.w;
        return Some(MaximizedMoveState { location, width });
    }
    None
}

/// A pointer grab that resizes a window interactively.
pub struct ResizeSurfaceGrab {
    pub start_data: GrabStartData<State>,
    pub window: Window,
    pub focus: Focus,
    pub initial_window_location: Point<i32, Logical>,
    pub initial_window_size: Size<i32, Logical>,
}

impl PointerGrab<State> for ResizeSurfaceGrab {
    #[allow(clippy::cast_possible_truncation)]
    fn motion(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        _focus: Option<(<State as SeatHandler>::PointerFocus, Point<f64, Logical>)>,
        event: &MotionEvent,
    ) {
        handle.motion(data, None, event);

        let delta = event.location - self.start_data.location;
        let dx = delta.x as i32;
        let dy = delta.y as i32;

        let (mut new_w, mut new_h) = (self.initial_window_size.w, self.initial_window_size.h);
        let (mut new_x, mut new_y) = (self.initial_window_location.x, self.initial_window_location.y);

        match self.focus {
            Focus::ResizeLeft | Focus::ResizeTopLeft | Focus::ResizeBottomLeft => {
                new_w -= dx;
                new_x += dx;
            }
            Focus::ResizeRight | Focus::ResizeTopRight | Focus::ResizeBottomRight => {
                new_w += dx;
            }
            _ => {}
        }

        match self.focus {
            Focus::ResizeTop | Focus::ResizeTopLeft | Focus::ResizeTopRight => {
                new_h -= dy;
                new_y += dy;
            }
            Focus::ResizeBottom | Focus::ResizeBottomLeft | Focus::ResizeBottomRight => {
                new_h += dy;
            }
            _ => {}
        }

        // Enforce minimum size
        if new_w < MIN_WINDOW_WIDTH {
            if new_x != self.initial_window_location.x {
                new_x -= MIN_WINDOW_WIDTH - new_w;
            }
            new_w = MIN_WINDOW_WIDTH;
        }
        if new_h < MIN_WINDOW_HEIGHT {
            if new_y != self.initial_window_location.y {
                new_y -= MIN_WINDOW_HEIGHT - new_h;
            }
            new_h = MIN_WINDOW_HEIGHT;
        }

        if let Some(toplevel) = self.window.toplevel() {
            toplevel.with_pending_state(|s| {
                s.states.set(xdg_toplevel::State::Resizing);
                s.size = Some(Size::from((new_w, new_h)));
            });
            toplevel.send_configure();
        }

        // X11 windows: resize via X11 configure request
        if let Some(x11) = self.window.x11_surface()
            && let Err(err) =
                x11.configure(smithay::utils::Rectangle::new((new_x, new_y).into(), (new_w, new_h).into()))
        {
            tracing::warn!(%err, "failed to configure X11 window during resize");
        }

        data.space.map_element(self.window.clone(), (new_x, new_y), true);
    }

    fn relative_motion(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        focus: Option<(<State as SeatHandler>::PointerFocus, Point<f64, Logical>)>,
        event: &RelativeMotionEvent,
    ) {
        handle.relative_motion(data, focus, event);
    }

    fn button(&mut self, data: &mut State, handle: &mut PointerInnerHandle<'_, State>, event: &ButtonEvent) {
        handle.button(data, event);

        if !handle.current_pressed().contains(&self.start_data.button) {
            // End the resize: remove resizing state
            if let Some(toplevel) = self.window.toplevel() {
                toplevel.with_pending_state(|s| {
                    s.states.unset(xdg_toplevel::State::Resizing);
                });
                toplevel.send_configure();
            }

            // X11: send a final configure with the current geometry
            if let Some(x11) = self.window.x11_surface()
                && let Err(err) = x11.configure(None)
            {
                tracing::warn!(%err, "failed to configure X11 window after resize");
            }

            handle.unset_grab(self, data, event.serial, event.time, true);
        }
    }

    fn axis(&mut self, data: &mut State, handle: &mut PointerInnerHandle<'_, State>, details: AxisFrame) {
        handle.axis(data, details);
    }

    fn frame(&mut self, data: &mut State, handle: &mut PointerInnerHandle<'_, State>) {
        handle.frame(data);
    }

    impl_grab_gesture_passthrough!();

    fn start_data(&self) -> &GrabStartData<State> {
        &self.start_data
    }

    fn unset(&mut self, data: &mut State) {
        // After a resize, tell X11 clients their new position/size so they
        // have consistent screen coordinates for popup placement.
        if let Some(x11) = self.window.x11_surface()
            && let Some(loc) = data.space.element_location(&self.window)
        {
            let size = x11.geometry().size;
            if let Err(err) =
                x11.configure(smithay::utils::Rectangle::new(loc, size))
            {
                tracing::warn!(%err, "failed to configure X11 window after resize");
            }
        }
    }
}

/// Try to start a window resize grab. Returns `true` if the grab was initiated.
pub fn handle_resize_request(
    data: &mut State,
    seat: &smithay::input::Seat<State>,
    window: &Window,
    focus: Focus,
    serial: smithay::utils::Serial,
) -> bool {
    let Some(pointer) = seat.get_pointer() else {
        return false;
    };

    if !pointer.has_grab(serial) {
        tracing::debug!("resize_request with non-matching serial, allowing anyway");
    }

    let start_data = pointer.grab_start_data().unwrap_or_else(|| GrabStartData {
        focus: None,
        button: BTN_LEFT,
        location: pointer.current_location(),
    });

    let initial_window_location = data.space.element_location(window).unwrap_or_default();
    let initial_window_size = window.geometry().size;

    // Set compositor cursor so the resize cursor persists during the grab.
    // Without this, CSD-initiated resizes would lose the cursor when
    // GrabFocus::Clear triggers a pointer-leave and the client resets its cursor.
    data.compositor_cursor_shape = crate::decorations::CursorShape::from(focus);

    let grab =
        ResizeSurfaceGrab { start_data, window: window.clone(), focus, initial_window_location, initial_window_size };

    pointer.set_grab(data, grab, serial, GrabFocus::Clear);
    true
}
