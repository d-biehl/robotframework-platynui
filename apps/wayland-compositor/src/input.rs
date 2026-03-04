//! Input event processing — keyboard and pointer.

use std::borrow::Cow;

use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, ButtonState, Event, InputBackend, InputEvent, KeyState, KeyboardKeyEvent,
        PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
    },
    desktop::{Window, WindowSurfaceType, layer_map_for_output},
    input::{
        keyboard::FilterResult,
        pointer::{AxisFrame, ButtonEvent, MotionEvent},
    },
    utils::{Logical, Point, Rectangle, SERIAL_COUNTER, Serial},
    wayland::{seat::WaylandFocus, shell::wlr_layer::Layer as WlrLayer},
    xwayland::X11Surface,
};

use crate::{
    decorations::{self, CursorShape, Focus, PointerHitResult},
    focus::PointerFocusTarget,
    state::State,
};

/// Linux input event code for the left mouse button (`BTN_LEFT`).
pub(crate) const BTN_LEFT: u32 = 0x110;
/// Linux input event code for the right mouse button (`BTN_RIGHT`).
const BTN_RIGHT: u32 = 0x111;
/// Maximum elapsed time between two clicks to count as a double-click.
const DOUBLE_CLICK_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(400);
/// Logical scroll pixels per discrete scroll notch.
const SCROLL_PIXELS_PER_NOTCH: f64 = 3.0;
/// High-resolution scroll units per discrete notch (v120 standard).
const V120_UNITS_PER_NOTCH: f64 = 120.0;

/// Process input events from any backend.
pub fn process_input_event<B: InputBackend>(state: &mut State, event: InputEvent<B>) {
    match event {
        InputEvent::Keyboard { event } => handle_keyboard::<B>(state, &event),
        InputEvent::PointerMotion { event } => handle_pointer_motion::<B>(state, &event),
        InputEvent::PointerMotionAbsolute { event } => {
            handle_pointer_motion_absolute::<B>(state, &event);
        }
        InputEvent::PointerButton { event } => handle_pointer_button::<B>(state, &event),
        InputEvent::PointerAxis { event } => handle_pointer_axis::<B>(state, &event),
        _ => {
            tracing::debug!("unhandled input event (touch, tablet, gesture, or device hotplug)");
        }
    }
}

/// Release all currently pressed keys and pointer buttons.
///
/// Called when the winit host window loses focus.  The host window manager
/// (e.g. GNOME Shell) intercepts key combos like Alt+Tab, swallowing the
/// release events.  Releasing immediately on focus-loss ensures Wayland
/// clients inside the compositor see correct modifier/button state right
/// away instead of remaining stuck until focus returns.
pub fn release_all_pressed_inputs(state: &mut State) {
    // --- Keyboard keys ---
    let keyboard = state.keyboard();
    let pressed_keys = keyboard.pressed_keys();
    if !pressed_keys.is_empty() {
        tracing::info!(count = pressed_keys.len(), "releasing stuck keys on focus loss");
        for key_code in pressed_keys {
            let serial = SERIAL_COUNTER.next_serial();
            keyboard.input::<(), _>(state, key_code, KeyState::Released, serial, 0, |_, _, _| FilterResult::Forward);
        }
    }

    // --- Pointer buttons ---
    let pressed_buttons: Vec<u32> = state.pressed_buttons.drain(..).collect();
    if !pressed_buttons.is_empty() {
        tracing::info!(count = pressed_buttons.len(), "releasing stuck pointer buttons on focus loss");
        let pointer = state.pointer();
        for button in pressed_buttons {
            let serial = SERIAL_COUNTER.next_serial();
            pointer.button(state, &ButtonEvent { serial, time: 0, button, state: ButtonState::Released });
        }
        let pointer = state.pointer();
        pointer.frame(state);
    }
}

fn handle_keyboard<B: InputBackend>(state: &mut State, event: &B::KeyboardKeyEvent) {
    use smithay::backend::session::Session;

    let serial = SERIAL_COUNTER.next_serial();
    let time = Event::time_msec(event);
    let key_code = event.key_code();
    let key_state: KeyState = event.state();

    let keyboard = state.keyboard();
    tracing::debug!(key_code = key_code.raw(), ?key_state, "keyboard event");
    keyboard.input::<(), _>(state, key_code, key_state, serial, time, |data, _modifiers, handle| {
        // Intercept VT switching keysyms (Ctrl+Alt+F1..F12) on the DRM backend.
        // XKB produces XF86_Switch_VT_<n> keysyms (0x1008_FE01..0x1008_FE0C)
        // when Ctrl+Alt+F<n> is pressed — we forward these to libseat.
        const XF86_SWITCH_VT_1: u32 = 0x1008_FE01;
        const XF86_SWITCH_VT_12: u32 = 0x1008_FE0C;

        let sym = handle.modified_sym();
        if (XF86_SWITCH_VT_1..=XF86_SWITCH_VT_12).contains(&sym.raw()) {
            if let Some(ref mut backend) = data.drm_backend {
                #[allow(clippy::cast_possible_wrap)]
                let vt = (sym.raw() - XF86_SWITCH_VT_1 + 1) as i32;
                tracing::info!(vt, "switching VT");
                if let Err(err) = backend.session.change_vt(vt) {
                    tracing::warn!(%err, vt, "failed to switch VT");
                }
            }
            return FilterResult::Intercept(());
        }

        FilterResult::Forward
    });
}

fn handle_pointer_motion<B: InputBackend>(state: &mut State, event: &B::PointerMotionEvent) {
    let serial = SERIAL_COUNTER.next_serial();
    let delta = event.delta();
    state.pointer_location += delta;
    clamp_pointer_location(state);

    update_cursor_shape(state);

    let under = surface_under(state);
    let pointer = state.pointer();
    pointer.motion(state, under, &MotionEvent { location: state.pointer_location, serial, time: event.time_msec() });
    pointer.frame(state);
}

fn handle_pointer_motion_absolute<B: InputBackend>(state: &mut State, event: &B::PointerMotionAbsoluteEvent) {
    let serial = SERIAL_COUNTER.next_serial();

    // Use combined output bounds so absolute events span all monitors.
    let combined_geo = state.combined_output_geometry();
    let pos = event.position_transformed(combined_geo.size);
    // For absolute events (winit backend), the host cursor IS the truth.
    // Do NOT clamp to outputs — doing so would create a disconnect between
    // the visual host cursor and the logical pointer position, breaking
    // subsequent interactions (e.g. can't click on a window a second time).
    // Dead-zone handling for windows is done in the move grab instead.
    state.pointer_location = (pos.x + f64::from(combined_geo.loc.x), pos.y + f64::from(combined_geo.loc.y)).into();

    update_cursor_shape(state);

    let under = surface_under(state);
    tracing::trace!(x = pos.x, y = pos.y, has_target = under.is_some(), "pointer motion absolute",);
    let pointer = state.pointer();
    pointer.motion(state, under, &MotionEvent { location: state.pointer_location, serial, time: event.time_msec() });
    pointer.frame(state);
}

fn handle_pointer_button<B: InputBackend>(state: &mut State, event: &B::PointerButtonEvent) {
    let button = event.button_code();
    let button_state = event.state();
    let time = event.time_msec();
    process_pointer_button(state, button, button_state, time);
}

/// Core pointer button logic shared by backend input events and virtual
/// pointer (VNC / remote input).
///
/// Handles decoration hit-testing, window focus/raise, titlebar buttons,
/// context menus, and forwarding the event to the Wayland seat.
#[allow(clippy::too_many_lines)]
pub(crate) fn process_pointer_button(state: &mut State, button: u32, button_state: ButtonState, time: u32) {
    // Track pressed buttons for focus-loss cleanup.
    match button_state {
        ButtonState::Pressed => state.pressed_buttons.push(button),
        ButtonState::Released => state.pressed_buttons.retain(|&b| b != button),
    }

    let serial = SERIAL_COUNTER.next_serial();

    tracing::debug!(
        button,
        ?button_state,
        ?serial,
        pointer_x = state.pointer_location.x,
        pointer_y = state.pointer_location.y,
        is_grabbed = state.seat.get_pointer().is_some_and(|p| p.is_grabbed()),
        "pointer button event",
    );

    // On press, update keyboard focus to the window under the pointer.
    // Only change focus when no pointer grab is active (e.g. a popup grab).
    // During a popup grab the PopupPointerGrab/PopupKeyboardGrab manage
    // focus — forcibly setting it here would conflict with the grab.
    if button_state == ButtonState::Pressed {
        // --- Context menu interaction ----------------------------------------
        // If a context menu is open, handle the click before anything else:
        // clicks on a menu item execute the action; clicks anywhere else (or
        // on non-interactive menu areas) simply dismiss the menu.
        if let Some(menu) = state.context_menu.take() {
            if let Some(item_idx) = menu.item_at(state.pointer_location) {
                if let Some(action) = decorations::TitlebarContextMenu::item_action(item_idx) {
                    handle_decoration_click(state, &menu.window, action, serial, button);
                }
                return; // consume the click
            }
            if menu.contains(state.pointer_location) {
                return; // clicked on separator/padding — consume the click
            }
            // Clicked outside menu — menu is already dismissed (taken above);
            // fall through to normal handling so the click reaches the target.
        }

        // Bridge X11 pointer grabs across SSD / empty-desktop areas.
        // See `bridge_x11_pointer_grab` for the architectural rationale.
        if bridge_x11_pointer_grab(state, button, serial, time) {
            return;
        }

        let pointer = state.pointer();
        if !pointer.is_grabbed() {
            // Unified front-to-back hit test — the first window whose full
            // bounds contain the pointer owns the click.  This prevents
            // clicking through a foreground window's area to interact with
            // a background window's decorations.
            match decorations::pointer_hit_test(&state.space, state.pointer_location) {
                PointerHitResult::Ssd(window, focus) if focus.is_resize() => {
                    let keyboard = state.keyboard();
                    keyboard.set_focus(state, Some(crate::focus::KeyboardFocusTarget::Window(window.clone())), serial);
                    state.space.raise_element(&window, true);
                    crate::grabs::handle_resize_request(state, &state.seat.clone(), &window, focus, serial);
                    return;
                }
                PointerHitResult::Ssd(window, Focus::Header) => {
                    // Right-click on the header → open context menu
                    if button == BTN_RIGHT {
                        open_titlebar_context_menu(state, &window, serial);
                        return;
                    }

                    let window_loc = state.space.element_location(&window).unwrap_or_default();
                    let client_size = window.geometry().size;
                    let titlebar_loc = window_loc - Point::from((0, decorations::TITLEBAR_HEIGHT));
                    let deco_click =
                        decorations::titlebar_button_hit_test(state.pointer_location, titlebar_loc, client_size)
                            .unwrap_or(decorations::DecorationClick::TitleBar);

                    // Buttons (Close/Maximize/Minimize): defer action to
                    // release so the user can cancel by moving the pointer
                    // away.  TitleBar (move / double-click) triggers
                    // immediately on press.
                    match deco_click {
                        decorations::DecorationClick::Close
                        | decorations::DecorationClick::Maximize
                        | decorations::DecorationClick::Minimize => {
                            // Focus + raise immediately so the window feels
                            // responsive, but defer the actual action.
                            let keyboard = state.keyboard();
                            keyboard.set_focus(
                                state,
                                Some(crate::focus::KeyboardFocusTarget::Window(window.clone())),
                                serial,
                            );
                            state.space.raise_element(&window, true);
                            state.pressed_titlebar_button = Some((window, deco_click));
                        }
                        decorations::DecorationClick::TitleBar => {
                            handle_decoration_click(state, &window, deco_click, serial, button);
                        }
                    }
                    return;
                }
                PointerHitResult::Ssd(_, _) => {
                    // Exhaustive: all Focus variants are covered above (resize guard + Header).
                }
                PointerHitResult::ClientArea(window, _window_loc) => {
                    tracing::debug!("pointer button press — setting keyboard focus");
                    let keyboard = state.keyboard();
                    keyboard.set_focus(state, Some(crate::focus::KeyboardFocusTarget::Window(window.clone())), serial);
                    state.space.raise_element(&window, true);
                }
                PointerHitResult::Empty => {
                    tracing::debug!("pointer button press — no window under cursor");
                }
            }
        }
    }

    // -- Button release: execute deferred titlebar button action ---------------
    if button_state == ButtonState::Released
        && let Some((window, pending_click)) = state.pressed_titlebar_button.take()
    {
        // Only fire the action if the pointer is still over the *same*
        // button.  If the user moved the pointer away, the click is
        // cancelled — standard UI behaviour.
        let still_over_same = (|| {
            let window_loc = state.space.element_location(&window)?;
            let client_size = window.geometry().size;
            let titlebar_loc = window_loc - Point::from((0, decorations::TITLEBAR_HEIGHT));
            decorations::titlebar_button_hit_test(state.pointer_location, titlebar_loc, client_size)
        })()
        .is_some_and(|hit| hit == pending_click);

        if still_over_same {
            handle_decoration_click(state, &window, pending_click, serial, button);
        }
        // Either way, the pending state is consumed (already taken above).
        // Don't forward this release to the client — it was decoration-only.
        return;
    }

    let pointer = state.pointer();
    pointer.button(state, &ButtonEvent { button, state: button_state, serial, time });
    pointer.frame(state);
}

#[allow(clippy::cast_possible_truncation)]
fn handle_pointer_axis<B: InputBackend>(state: &mut State, event: &B::PointerAxisEvent) {
    let source = event.source();
    let horizontal_amount = event.amount(Axis::Horizontal).unwrap_or_else(|| {
        event.amount_v120(Axis::Horizontal).unwrap_or(0.0) * SCROLL_PIXELS_PER_NOTCH / V120_UNITS_PER_NOTCH
    });
    let vertical_amount = event.amount(Axis::Vertical).unwrap_or_else(|| {
        event.amount_v120(Axis::Vertical).unwrap_or(0.0) * SCROLL_PIXELS_PER_NOTCH / V120_UNITS_PER_NOTCH
    });

    let mut frame = AxisFrame::new(event.time_msec()).source(source);

    if horizontal_amount != 0.0 {
        frame = frame.value(Axis::Horizontal, horizontal_amount);
        if let Some(v120) = event.amount_v120(Axis::Horizontal) {
            frame = frame.v120(Axis::Horizontal, v120 as i32);
        }
    }
    if vertical_amount != 0.0 {
        frame = frame.value(Axis::Vertical, vertical_amount);
        if let Some(v120) = event.amount_v120(Axis::Vertical) {
            frame = frame.v120(Axis::Vertical, v120 as i32);
        }
    }

    let pointer = state.pointer();
    pointer.axis(state, frame);
    pointer.frame(state);
}

/// Update the compositor's cursor shape based on what the pointer hovers over.
///
/// Uses the unified [`pointer_hit_test`](decorations::pointer_hit_test) to
/// determine the correct cursor.  For SSD zones, the [`Focus`] variant maps
/// directly to a cursor shape via [`Focus::cursor_shape()`].  Because the
/// hit-test iterates front-to-back, a foreground CSD window correctly blocks
/// resize-cursor detection on background SSD windows.
pub(crate) fn update_cursor_shape(state: &mut State) {
    // Don't override cursor during an active grab (move/resize in progress)
    if state.seat.get_pointer().is_some_and(|p| p.is_grabbed()) {
        return;
    }

    state.compositor_cursor_shape = match decorations::pointer_hit_test(&state.space, state.pointer_location) {
        PointerHitResult::Ssd(_, focus) => CursorShape::from(focus),
        _ => CursorShape::Default,
    };
}

/// Clamp the pointer location to the actual output coverage area.
///
/// For non-rectangular multi-monitor layouts (e.g. L-shaped), the pointer
/// is snapped to the nearest output boundary rather than the rectangular
/// bounding box.  This prevents the cursor from entering dead zones.
pub(crate) fn clamp_pointer_location(state: &mut State) {
    state.pointer_location = state.clamp_to_outputs(state.pointer_location);
}

/// Returns `true` if any X11 override-redirect windows (menus, tooltips,
/// dropdowns) are currently mapped in the space.
fn has_x11_override_redirect_windows(state: &State) -> bool {
    state.space.elements().any(|w| w.x11_surface().is_some_and(X11Surface::is_override_redirect))
}

/// Bridge an X11 pointer grab across an SSD / empty-desktop gap.
///
/// # Background
///
/// In native X11, when a client holds an active pointer grab (as toolkits do
/// for menus), the X server delivers **every** button press to the grab owner
/// regardless of where on screen the click lands.  In the Wayland model,
/// `XWayland` only receives events that arrive via `wl_pointer` — which
/// requires an active pointer focus on a Wayland surface.
///
/// In compositors that use CSD (Client-Side Decorations), every pixel of the
/// screen is covered by a `wl_surface`, so the click always reaches some
/// client and `XWayland` sees it via its grab.  However, our compositor uses
/// SSD (Server-Side Decorations): the titlebar, resize borders, and empty
/// desktop have **no** `wl_surface`.  When the pointer is over one of these
/// areas, `surface_under()` returns `None` and `wl_pointer` has no focus
/// target — the click is consumed by the compositor and never reaches
/// `XWayland`.
///
/// This function bridges that gap: when an X11 pointer grab is likely active
/// (detected by the presence of override-redirect windows), it temporarily
/// sets `wl_pointer` focus to the grab-owner's surface and sends the button
/// press.  `XWayland` translates this into the X11 grab event the toolkit
/// expects, and the menu loop exits normally.
///
/// Returns `true` when a bridge was performed (consumes the click).
fn bridge_x11_pointer_grab(state: &mut State, button: u32, serial: Serial, time: u32) -> bool {
    if !has_x11_override_redirect_windows(state) {
        return false;
    }

    // Find the non-override-redirect X11 window that owns the grab.
    let parent = state.space.elements().find(|w| w.x11_surface().is_some_and(|x| !x.is_override_redirect())).cloned();

    let Some(parent) = parent else { return false };
    let Some(wl_surface) = parent.wl_surface().map(Cow::into_owned) else {
        return false;
    };
    let window_loc = state.space.element_location(&parent).unwrap_or_default().to_f64();
    let loc = state.pointer_location;

    let pointer = state.pointer();
    pointer.motion(
        state,
        Some((PointerFocusTarget::Surface(wl_surface), window_loc)),
        &MotionEvent { location: loc, serial, time },
    );
    pointer.button(state, &ButtonEvent { button, state: ButtonState::Pressed, serial, time });
    pointer.frame(state);
    true
}

/// Handle a click on a window decoration element.
fn handle_decoration_click(
    state: &mut State,
    window: &Window,
    click: crate::decorations::DecorationClick,
    serial: Serial,
    _button: u32,
) {
    use crate::decorations::DecorationClick;
    use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;

    tracing::debug!(?click, "decoration click");

    // Raise and focus the window
    let keyboard = state.keyboard();
    keyboard.set_focus(state, Some(crate::focus::KeyboardFocusTarget::Window(window.clone())), serial);
    state.space.raise_element(window, true);

    match click {
        DecorationClick::Close => {
            // Wayland: xdg_toplevel close event.
            // X11: WM_DELETE_WINDOW or destroy.
            if let Some(toplevel) = window.toplevel() {
                toplevel.send_close();
            }
            if let Some(x11) = window.x11_surface()
                && let Err(err) = x11.close()
            {
                tracing::warn!(%err, "failed to close X11 window");
            }
        }
        DecorationClick::Maximize => {
            if let Some(toplevel) = window.toplevel() {
                let is_maximized = toplevel.current_state().states.contains(xdg_toplevel::State::Maximized);
                if is_maximized {
                    crate::handlers::xdg_shell::do_unmaximize(state, toplevel);
                } else {
                    crate::handlers::xdg_shell::do_maximize(state, toplevel);
                }
            }
            if window.toplevel().is_none()
                && let Some(x11) = window.x11_surface()
            {
                maximize_x11_window(state, window, x11);
            }
        }
        DecorationClick::Minimize => {
            // Minimize the window, move focus, and notify foreign-toplevel clients.
            crate::handlers::foreign_toplevel::minimize_window(state, window);
            tracing::debug!("window minimized");
        }
        DecorationClick::TitleBar => {
            // Double-click on titlebar toggles maximize.
            let now = std::time::Instant::now();
            let is_double_click =
                state.last_titlebar_click.is_some_and(|last| now.duration_since(last) < DOUBLE_CLICK_TIMEOUT);

            if is_double_click {
                state.last_titlebar_click = None;
                // Toggle maximize — reuse the Maximize click handler.
                handle_decoration_click(state, window, DecorationClick::Maximize, serial, 0);
                return;
            }

            state.last_titlebar_click = Some(now);
            // Start an interactive move grab
            crate::grabs::handle_move_request(state, &state.seat.clone(), window, serial);
        }
    }
}

/// Maximize or restore an X11 (`XWayland`) window.
///
/// Uses `X11Surface::is_maximized()` / `set_maximized()` and `configure()` to
/// resize the window, mirroring the Wayland xdg-toplevel maximize logic.
fn maximize_x11_window(state: &mut State, window: &Window, x11: &smithay::xwayland::X11Surface) {
    if x11.is_maximized() {
        // Unmaximize — restore saved position and size
        let saved = state
            .pre_maximize_positions
            .iter()
            .position(|(w, _, _)| w == window)
            .map(|i| state.pre_maximize_positions.remove(i));

        if let Err(err) = x11.set_maximized(false) {
            tracing::warn!(%err, "failed to unmaximize X11 window");
        }

        if let Some((_, pos, size)) = saved {
            if let Err(err) = x11.configure(size.map(|s| smithay::utils::Rectangle::new(pos, s))) {
                tracing::warn!(%err, "failed to configure X11 window after unmaximize");
            }
            state.space.map_element(window.clone(), pos, true);
        } else {
            // No saved state — let the client choose its own size
            if let Err(err) = x11.configure(None) {
                tracing::warn!(%err, "failed to configure X11 window after unmaximize");
            }
        }
    } else {
        // Save current position and size
        if let Some(current_loc) = state.space.element_location(window) {
            state.pre_maximize_positions.retain(|(w, _, _)| w != window);
            state.pre_maximize_positions.push((window.clone(), current_loc, Some(x11.geometry().size)));
        }

        // Maximize to usable area minus titlebar
        let usable_geo = state.usable_geometry_for_window(window);
        let max_size =
            smithay::utils::Size::from((usable_geo.size.w, usable_geo.size.h - crate::decorations::TITLEBAR_HEIGHT));
        if let Err(err) = x11.set_maximized(true) {
            tracing::warn!(%err, "failed to maximize X11 window");
        }
        if let Err(err) = x11.configure(smithay::utils::Rectangle::new(
            (usable_geo.loc.x, usable_geo.loc.y + crate::decorations::TITLEBAR_HEIGHT).into(),
            max_size,
        )) {
            tracing::warn!(%err, "failed to configure X11 window for maximize");
        }
        state.space.map_element(
            window.clone(),
            (usable_geo.loc.x, usable_geo.loc.y + crate::decorations::TITLEBAR_HEIGHT),
            true,
        );
    }
}

/// Find the surface under the current pointer location.
///
/// Hit-test order (front to back):
/// 1. Overlay layer surfaces
/// 2. Top layer surfaces
/// 3. Windows (with SSD decoration handling)
/// 4. Bottom layer surfaces
/// 5. Background layer surfaces
///
/// For client-area hits, resolves the specific `WlSurface` (subsurface, popup,
/// or toplevel) under the cursor.
pub(crate) fn surface_under(state: &State) -> Option<(PointerFocusTarget, Point<f64, Logical>)> {
    let output = state.output_at_point(state.pointer_location);
    let output_geo = state.space.output_geometry(output).unwrap_or_default();

    // Check Overlay and Top layer surfaces first (above windows)
    if let Some(hit) = layer_surface_under(output, output_geo, state.pointer_location, WlrLayer::Overlay)
        .or_else(|| layer_surface_under(output, output_geo, state.pointer_location, WlrLayer::Top))
    {
        return Some(hit);
    }

    // Check windows (existing SSD-aware hit-test)
    match decorations::pointer_hit_test(&state.space, state.pointer_location) {
        PointerHitResult::ClientArea(window, render_loc) => {
            let point_in_window = state.pointer_location - render_loc.to_f64();

            if let Some((surface, surface_offset)) = window.surface_under(point_in_window, WindowSurfaceType::ALL) {
                let surface_global = render_loc + surface_offset;
                tracing::trace!(
                    pointer_x = state.pointer_location.x,
                    pointer_y = state.pointer_location.y,
                    render_loc_x = render_loc.x,
                    render_loc_y = render_loc.y,
                    in_window_x = point_in_window.x,
                    in_window_y = point_in_window.y,
                    surface_offset_x = surface_offset.x,
                    surface_offset_y = surface_offset.y,
                    surface_global_x = surface_global.x,
                    surface_global_y = surface_global.y,
                    "surface_under: resolved surface",
                );
                return Some((PointerFocusTarget::Surface(surface), surface_global.to_f64()));
            }
            tracing::trace!(
                pointer_x = state.pointer_location.x,
                pointer_y = state.pointer_location.y,
                render_loc_x = render_loc.x,
                render_loc_y = render_loc.y,
                "surface_under: fallback to window target",
            );
            return Some((PointerFocusTarget::Window(window), render_loc.to_f64()));
        }
        PointerHitResult::Ssd(_, _) => {
            // SSD decoration areas — compositor handles these, no client surface.
            return None;
        }
        PointerHitResult::Empty => {}
    }

    // Check Bottom and Background layer surfaces (below windows)
    if let Some(hit) = layer_surface_under(output, output_geo, state.pointer_location, WlrLayer::Bottom)
        .or_else(|| layer_surface_under(output, output_geo, state.pointer_location, WlrLayer::Background))
    {
        return Some(hit);
    }

    None
}

/// Hit-test a specific layer on the given output.
///
/// Returns the focus target and global surface location if a layer surface
/// under the pointer is found on the requested layer.
fn layer_surface_under(
    output: &smithay::output::Output,
    output_geo: Rectangle<i32, Logical>,
    pointer: Point<f64, Logical>,
    layer: WlrLayer,
) -> Option<(PointerFocusTarget, Point<f64, Logical>)> {
    let map = layer_map_for_output(output);
    // Convert pointer to output-local coordinates
    let point_in_output = pointer - output_geo.loc.to_f64();
    let layer_surface = map.layer_under(layer, point_in_output)?;
    let layer_geo = map.layer_geometry(layer_surface)?;

    let point_in_layer = point_in_output - layer_geo.loc.to_f64();
    if let Some((surface, surface_offset)) = layer_surface.surface_under(point_in_layer, WindowSurfaceType::ALL) {
        let surface_global = output_geo.loc + layer_geo.loc + surface_offset;
        return Some((PointerFocusTarget::Surface(surface), surface_global.to_f64()));
    }

    tracing::trace!(
        pointer_x = pointer.x,
        pointer_y = pointer.y,
        layer_geo_x = layer_geo.loc.x,
        layer_geo_y = layer_geo.loc.y,
        layer_geo_w = layer_geo.size.w,
        layer_geo_h = layer_geo.size.h,
        point_in_layer_x = point_in_layer.x,
        point_in_layer_y = point_in_layer.y,
        ?layer,
        "layer_surface_under: layer_under found surface but surface_under returned None",
    );
    None
}

/// Open a right-click context menu on a window's SSD titlebar.
#[allow(clippy::cast_possible_truncation)]
fn open_titlebar_context_menu(state: &mut State, window: &Window, serial: Serial) {
    use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;

    // Raise and focus the window
    let keyboard = state.keyboard();
    keyboard.set_focus(state, Some(crate::focus::KeyboardFocusTarget::Window(window.clone())), serial);
    state.space.raise_element(window, true);

    let is_maximized = if let Some(toplevel) = window.toplevel() {
        toplevel.current_state().states.contains(xdg_toplevel::State::Maximized)
    } else if let Some(x11) = window.x11_surface() {
        x11.is_maximized()
    } else {
        false
    };

    let position: Point<i32, Logical> = (state.pointer_location.x as i32, state.pointer_location.y as i32).into();

    state.context_menu = Some(decorations::TitlebarContextMenu { window: window.clone(), position, is_maximized });
    tracing::debug!(?position, is_maximized, "opened titlebar context menu");
}
