//! Input event processing — keyboard and pointer.

use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, ButtonState, Event, InputBackend, InputEvent, KeyState, KeyboardKeyEvent,
        PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
    },
    desktop::{Window, WindowSurfaceType},
    input::{
        keyboard::FilterResult,
        pointer::{AxisFrame, ButtonEvent, MotionEvent},
    },
    utils::{Logical, Point, SERIAL_COUNTER, Serial},
};

use crate::{
    decorations::{self, CursorShape, Focus, PointerHitResult},
    focus::PointerFocusTarget,
    state::State,
};

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
        _ => {}
    }
}

fn handle_keyboard<B: InputBackend>(state: &mut State, event: &B::KeyboardKeyEvent) {
    let serial = SERIAL_COUNTER.next_serial();
    let time = Event::time_msec(event);
    let key_code = event.key_code();
    let key_state: KeyState = event.state();

    let keyboard = state.seat.get_keyboard().unwrap();
    tracing::debug!(key_code = key_code.raw(), ?key_state, "keyboard event");
    keyboard.input::<(), _>(state, key_code, key_state, serial, time, |_, _, _| FilterResult::Forward);
}

fn handle_pointer_motion<B: InputBackend>(state: &mut State, event: &B::PointerMotionEvent) {
    let serial = SERIAL_COUNTER.next_serial();
    let delta = event.delta();
    state.pointer_location += delta;
    clamp_pointer_location(state);

    update_cursor_shape(state);

    let under = surface_under(state);
    let pointer = state.seat.get_pointer().unwrap();
    pointer.motion(state, under, &MotionEvent { location: state.pointer_location, serial, time: event.time_msec() });
    pointer.frame(state);
}

fn handle_pointer_motion_absolute<B: InputBackend>(state: &mut State, event: &B::PointerMotionAbsoluteEvent) {
    let serial = SERIAL_COUNTER.next_serial();

    // Use combined output bounds so absolute events span all monitors.
    let combined_geo = state.combined_output_geometry();
    let pos = event.position_transformed(combined_geo.size);
    state.pointer_location = (pos.x + f64::from(combined_geo.loc.x), pos.y + f64::from(combined_geo.loc.y)).into();

    update_cursor_shape(state);

    let under = surface_under(state);
    tracing::trace!(x = pos.x, y = pos.y, has_target = under.is_some(), "pointer motion absolute",);
    let pointer = state.seat.get_pointer().unwrap();
    pointer.motion(state, under, &MotionEvent { location: state.pointer_location, serial, time: event.time_msec() });
    pointer.frame(state);
}

fn handle_pointer_button<B: InputBackend>(state: &mut State, event: &B::PointerButtonEvent) {
    let serial = SERIAL_COUNTER.next_serial();
    let button = event.button_code();
    let button_state = event.state();

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

        let pointer = state.seat.get_pointer().unwrap();
        if !pointer.is_grabbed() {
            // Unified front-to-back hit test — the first window whose full
            // bounds contain the pointer owns the click.  This prevents
            // clicking through a foreground window's area to interact with
            // a background window's decorations.
            match decorations::pointer_hit_test(&state.space, state.pointer_location) {
                PointerHitResult::Ssd(window, focus) if focus.is_resize() => {
                    let keyboard = state.seat.get_keyboard().unwrap();
                    keyboard.set_focus(state, Some(crate::focus::KeyboardFocusTarget::Window(window.clone())), serial);
                    state.space.raise_element(&window, true);
                    crate::grabs::handle_resize_request(state, &state.seat.clone(), &window, focus, serial);
                    return;
                }
                PointerHitResult::Ssd(window, Focus::Header) => {
                    // Right-click on the header → open context menu
                    const BTN_RIGHT: u32 = 0x111;
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
                    handle_decoration_click(state, &window, deco_click, serial, button);
                    return;
                }
                PointerHitResult::Ssd(_, _) => {
                    // Unreachable — all Focus variants are either resize or Header.
                }
                PointerHitResult::ClientArea(window, _window_loc) => {
                    tracing::debug!("pointer button press — setting keyboard focus");
                    let keyboard = state.seat.get_keyboard().unwrap();
                    keyboard.set_focus(state, Some(crate::focus::KeyboardFocusTarget::Window(window.clone())), serial);
                    state.space.raise_element(&window, true);
                }
                PointerHitResult::Empty => {
                    // If there are minimized windows and no visible window was
                    // clicked, restore the most recently minimized one.
                    if let Some((window, pos)) = state.minimized_windows.pop() {
                        state.space.map_element(window.clone(), pos, true);
                        let keyboard = state.seat.get_keyboard().unwrap();
                        keyboard.set_focus(
                            state,
                            Some(crate::focus::KeyboardFocusTarget::Window(window.clone())),
                            serial,
                        );
                        state.space.raise_element(&window, true);
                        tracing::debug!("restored minimized window");
                    } else {
                        tracing::debug!("pointer button press — no window under cursor");
                    }
                }
            }
        }
    }

    let pointer = state.seat.get_pointer().unwrap();
    pointer.button(state, &ButtonEvent { button, state: button_state, serial, time: event.time_msec() });
    pointer.frame(state);
}

#[allow(clippy::cast_possible_truncation)]
fn handle_pointer_axis<B: InputBackend>(state: &mut State, event: &B::PointerAxisEvent) {
    let source = event.source();
    let horizontal_amount = event
        .amount(Axis::Horizontal)
        .unwrap_or_else(|| event.amount_v120(Axis::Horizontal).unwrap_or(0.0) * 3.0 / 120.0);
    let vertical_amount =
        event.amount(Axis::Vertical).unwrap_or_else(|| event.amount_v120(Axis::Vertical).unwrap_or(0.0) * 3.0 / 120.0);

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

    let pointer = state.seat.get_pointer().unwrap();
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
fn update_cursor_shape(state: &mut State) {
    // Don't override cursor during an active grab (move/resize in progress)
    if state.seat.get_pointer().is_some_and(|p| p.is_grabbed()) {
        return;
    }

    state.compositor_cursor_shape = match decorations::pointer_hit_test(&state.space, state.pointer_location) {
        PointerHitResult::Ssd(_, focus) => CursorShape::from(focus),
        _ => CursorShape::Default,
    };
}

/// Clamp the pointer location to the combined output bounds.
fn clamp_pointer_location(state: &mut State) {
    let bounds = state.combined_output_geometry();
    state.pointer_location.x =
        state.pointer_location.x.clamp(f64::from(bounds.loc.x), f64::from(bounds.loc.x + bounds.size.w) - 1.0);
    state.pointer_location.y =
        state.pointer_location.y.clamp(f64::from(bounds.loc.y), f64::from(bounds.loc.y + bounds.size.h) - 1.0);
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
    let keyboard = state.seat.get_keyboard().unwrap();
    keyboard.set_focus(state, Some(crate::focus::KeyboardFocusTarget::Window(window.clone())), serial);
    state.space.raise_element(window, true);

    match click {
        DecorationClick::Close => {
            // Wayland: xdg_toplevel close event.
            // X11: WM_DELETE_WINDOW or destroy.
            if let Some(toplevel) = window.toplevel() {
                toplevel.send_close();
            }
            if let Some(x11) = window.x11_surface() {
                let _ = x11.close();
            }
        }
        DecorationClick::Maximize => {
            if let Some(toplevel) = window.toplevel() {
                let is_maximized = toplevel.current_state().states.contains(xdg_toplevel::State::Maximized);
                if is_maximized {
                    // Unmaximize — restore the saved position and remove the maximized state
                    let restore_pos = state
                        .pre_maximize_positions
                        .iter()
                        .position(|(w, _)| w == window)
                        .map(|i| state.pre_maximize_positions.remove(i).1);

                    toplevel.with_pending_state(|s| {
                        s.states.unset(xdg_toplevel::State::Maximized);
                        s.size = None;
                    });

                    if let Some(pos) = restore_pos {
                        state.space.map_element(window.clone(), pos, true);
                    }
                } else {
                    // Save the current position before maximizing
                    if let Some(current_loc) = state.space.element_location(window) {
                        state.pre_maximize_positions.retain(|(w, _)| w != window);
                        state.pre_maximize_positions.push((window.clone(), current_loc));
                    }

                    // Maximize — set to output size minus titlebar
                    let output_geo = state.output_geometry_for_window(window);
                    toplevel.with_pending_state(|s| {
                        s.states.set(xdg_toplevel::State::Maximized);
                        s.size =
                            Some((output_geo.size.w, output_geo.size.h - crate::decorations::TITLEBAR_HEIGHT).into());
                    });
                    // Move to the output's top-left (below titlebar)
                    state.space.map_element(
                        window.clone(),
                        (output_geo.loc.x, output_geo.loc.y + crate::decorations::TITLEBAR_HEIGHT),
                        true,
                    );
                }
                toplevel.send_configure();
            }
            if window.toplevel().is_none()
                && let Some(x11) = window.x11_surface()
            {
                maximize_x11_window(state, window, x11);
            }
        }
        DecorationClick::Minimize => {
            // Save the window's current position and unmap it from the space.
            let pos = state.space.element_location(window).unwrap_or_default();
            state.minimized_windows.push((window.clone(), pos));
            state.space.unmap_elem(window);

            // Move keyboard focus to the next visible window (if any).
            let next_window = state.space.elements().next_back().cloned();
            if let Some(next) = next_window {
                let keyboard = state.seat.get_keyboard().unwrap();
                keyboard.set_focus(state, Some(crate::focus::KeyboardFocusTarget::Window(next.clone())), serial);
                state.space.raise_element(&next, true);
            } else {
                let keyboard = state.seat.get_keyboard().unwrap();
                keyboard.set_focus(state, Option::<crate::focus::KeyboardFocusTarget>::None, serial);
            }

            tracing::debug!("window minimized");
        }
        DecorationClick::TitleBar => {
            // Double-click on titlebar toggles maximize.
            let now = std::time::Instant::now();
            let is_double_click =
                state.last_titlebar_click.is_some_and(|last| now.duration_since(last).as_millis() < 400);

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
        // Unmaximize — restore saved position
        let restore_pos = state
            .pre_maximize_positions
            .iter()
            .position(|(w, _)| w == window)
            .map(|i| state.pre_maximize_positions.remove(i).1);

        let _ = x11.set_maximized(false);
        // configure(None) lets the client choose its own size again
        let _ = x11.configure(None);

        if let Some(pos) = restore_pos {
            state.space.map_element(window.clone(), pos, true);
        }
    } else {
        // Save current position
        if let Some(current_loc) = state.space.element_location(window) {
            state.pre_maximize_positions.retain(|(w, _)| w != window);
            state.pre_maximize_positions.push((window.clone(), current_loc));
        }

        // Maximize to output size minus titlebar
        let output_geo = state.output_geometry_for_window(window);
        let max_size =
            smithay::utils::Size::from((output_geo.size.w, output_geo.size.h - crate::decorations::TITLEBAR_HEIGHT));
        let _ = x11.set_maximized(true);
        let _ = x11.configure(smithay::utils::Rectangle::new(
            (output_geo.loc.x, output_geo.loc.y + crate::decorations::TITLEBAR_HEIGHT).into(),
            max_size,
        ));
        state.space.map_element(
            window.clone(),
            (output_geo.loc.x, output_geo.loc.y + crate::decorations::TITLEBAR_HEIGHT),
            true,
        );
    }
}

/// Find the surface under the current pointer location.
///
/// Uses the unified [`pointer_hit_test`](decorations::pointer_hit_test) to
/// determine which window owns the pointer position.  If the point falls on
/// an SSD decoration area (title bar or resize border), returns `None` so
/// that the client does not receive pointer events for compositor-owned
/// regions.
///
/// For client-area hits, resolves the specific `WlSurface` (subsurface, popup,
/// or toplevel) under the cursor.
fn surface_under(state: &State) -> Option<(PointerFocusTarget, Point<f64, Logical>)> {
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
                Some((PointerFocusTarget::Surface(surface), surface_global.to_f64()))
            } else {
                tracing::trace!(
                    pointer_x = state.pointer_location.x,
                    pointer_y = state.pointer_location.y,
                    render_loc_x = render_loc.x,
                    render_loc_y = render_loc.y,
                    "surface_under: fallback to window target",
                );
                Some((PointerFocusTarget::Window(window), render_loc.to_f64()))
            }
        }
        // SSD decoration areas (title bar, resize border) or empty space
        // → no client surface should receive pointer events.
        _ => None,
    }
}

/// Open a right-click context menu on a window's SSD titlebar.
#[allow(clippy::cast_possible_truncation)]
fn open_titlebar_context_menu(state: &mut State, window: &Window, serial: Serial) {
    use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;

    // Raise and focus the window
    let keyboard = state.seat.get_keyboard().unwrap();
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
