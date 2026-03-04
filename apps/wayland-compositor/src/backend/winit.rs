//! Winit backend — nested compositor running in a window for development.
//!
//! Uses `smithay::backend::winit` with a GL renderer to display client surfaces
//! in a desktop window. Useful for interactive testing and debugging.

use std::time::Duration;

use smithay::{
    backend::{
        renderer::{damage::OutputDamageTracker, glow::GlowRenderer},
        winit::{self, WinitEvent, WinitGraphicsBackend},
    },
    reexports::{
        calloop::EventLoop,
        wayland_server::Display,
        winit::{platform::pump_events::PumpStatus, window::CursorIcon},
    },
    utils::{Physical, Size},
};

use crate::{CompositorArgs, config::CompositorConfig, state::State};

/// The winit backend renders via OpenGL, which has a Y-up coordinate system.
/// Wayland uses Y-down. `Flipped180` compensates for this mismatch so that
/// rendered content appears right-side up in the window.
const WINIT_RENDER_TRANSFORM: smithay::utils::Transform = smithay::utils::Transform::Flipped180;

/// Run the compositor in a winit window.
///
/// # Errors
///
/// Returns an error if winit initialization, socket creation, or runtime fails.
#[allow(clippy::too_many_lines)]
pub fn run(args: &CompositorArgs, config: CompositorConfig) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop: EventLoop<'static, State> = EventLoop::try_new()?;
    let display: Display<State> = Display::new()?;

    // Initialize the winit backend with GlowRenderer (wraps GlesRenderer,
    // provides glow::Context for GPU-accelerated egui titlebar rendering).
    let (mut backend, mut winit_evt): (WinitGraphicsBackend<GlowRenderer>, _) = winit::init()?;

    // Create the listening socket
    let (listening_socket, socket_name) = super::create_listening_socket(args)?;

    // Use the CLI-specified size as the per-output resolution. For single
    // output this matches the winit window; for multi-output each virtual
    // monitor gets these dimensions and the window is resized to fit.
    let output_size: Size<i32, Physical> = (args.width.cast_signed(), args.height.cast_signed()).into();
    let timeout = if args.timeout > 0 { Some(Duration::from_secs(args.timeout)) } else { None };

    let mut state = State::new(
        display.handle(),
        event_loop.handle(),
        event_loop.get_signal(),
        socket_name.clone(),
        output_size,
        timeout,
        crate::resolve_xkb_config(args),
        args.outputs,
        args.output_layout,
        args.scale,
        crate::security::SecurityPolicy::from_args(args.restrict_protocols.as_deref()),
        config,
    );
    state.backend_name = "winit";
    state.window_scale = args.window_scale;
    state.software_cursor = args.software_cursor;

    // Pre-initialize the screenshot renderer with a shared EGL context so
    // that screenshots can access the main renderer's GL textures (titlebar
    // textures, client surfaces) without cross-context errors.
    match super::create_shared_glow_renderer(backend.renderer()) {
        Ok(renderer) => state.screenshot_renderer = Some(renderer),
        Err(err) => tracing::warn!(%err, "failed to create shared screenshot renderer, will use standalone"),
    }

    // Register Wayland display + listening socket + set WAYLAND_DISPLAY
    super::register_wayland_sources(&event_loop.handle(), display, listening_socket, &socket_name)?;

    // Register signal handlers, watchdog, XWayland, control socket, readiness
    let shutdown = super::setup_services(&event_loop.handle(), &mut state, args, timeout)?;

    // Use a static damage tracker with Flipped180 to compensate for GL's
    // inverted Y-axis. We keep the output itself at Transform::Normal so
    // clients see the correct orientation.
    // With multi-output, the tracker covers the entire combined physical area
    // so windows on any output are rendered correctly.
    //
    // When --window-scale is active, both the winit window and the rendering
    // are scaled down proportionally.  Wayland clients still see the real
    // output scale/mode.
    let tracker_size = state.render_size();

    // Resize the winit window to the render area so all virtual
    // monitors are visible when --outputs > 1 or --window-scale is set.
    if tracker_size.w != output_size.w || tracker_size.h != output_size.h {
        use smithay::reexports::winit::dpi::PhysicalSize;
        let _ = backend
            .window()
            .request_inner_size(PhysicalSize::new(tracker_size.w.unsigned_abs(), tracker_size.h.unsigned_abs()));
        tracing::info!(
            width = tracker_size.w,
            height = tracker_size.h,
            window_scale = state.window_scale,
            outputs = state.outputs.len(),
            "resized winit window for multi-output/window-scale",
        );
    }

    let render_scale = state.max_output_scale() * state.window_scale;
    let mut damage_tracker = OutputDamageTracker::new(tracker_size, render_scale, WINIT_RENDER_TRANSFORM);

    tracing::info!(backend = "winit", socket = %socket_name, "event loop starting");

    while state.running && !shutdown.is_set() {
        // Dispatch calloop sources FIRST: process pending client messages,
        // accept new connections, and fire timers/watchdog.  This ensures
        // clients get prompt responses even when render_frame() blocks on
        // GPU vsync.  Without this, roundtrip latency can grow to seconds
        // because the renderer's eglSwapBuffers delays all client dispatch.
        event_loop.dispatch(Some(Duration::from_millis(1)), &mut state)?;
        state.space.refresh();
        state.popup_manager.cleanup();

        // Flush responses from the dispatch above so clients (especially
        // GTK-based ones like waybar that do blocking roundtrips during
        // init) don't stall waiting for our reply.
        if let Err(err) = state.display_handle.flush_clients() {
            tracing::warn!(%err, "failed to flush Wayland clients (pre-render)");
        }

        let mut close_requested = false;
        let mut focus_lost = false;
        let mut input_events = Vec::new();
        let mut new_size: Option<Size<i32, Physical>> = None;

        let pump_status = winit_evt.dispatch_new_events(|event| match event {
            WinitEvent::Resized { size, .. } => {
                new_size = Some(size);
            }
            WinitEvent::Focus(focused) => {
                if !focused {
                    focus_lost = true;
                }
            }
            WinitEvent::Redraw => {}
            WinitEvent::Input(input) => {
                input_events.push(input);
            }
            WinitEvent::CloseRequested => {
                close_requested = true;
            }
        });

        if close_requested || matches!(pump_status, PumpStatus::Exit(_)) {
            state.running = false;
            break;
        }

        // When the host window loses focus, release any keys that are still
        // pressed.  The host WM (e.g. GNOME) intercepts combos like Alt+Tab
        // and swallows the release events.  Releasing immediately ensures
        // Wayland clients see clean modifier state right away.
        if focus_lost {
            crate::input::release_all_pressed_inputs(&mut state);
        }

        for input in input_events {
            crate::input::process_input_event(&mut state, input);
        }

        if let Some(size) = new_size {
            if state.outputs.len() > 1 {
                // Multi-output: resize the monitors on the right/bottom edges
                // so the combined layout fills the new window exactly.  This
                // keeps the pointer mapping consistent (logical layout always
                // spans the full window).
                state.resize_edge_outputs(size);
                state.reconfigure_windows_for_outputs();
            } else {
                // Single output: resize changes the virtual monitor resolution.
                // Derive the output mode from the window size, inverting
                // window_scale so that render_size() == window_size.
                let ws = state.window_scale.max(f64::EPSILON);
                #[allow(clippy::cast_possible_truncation)]
                let mode_size: Size<i32, Physical> =
                    ((f64::from(size.w) / ws).round() as i32, (f64::from(size.h) / ws).round() as i32).into();
                let mode = smithay::output::Mode { size: mode_size, refresh: crate::state::DEFAULT_REFRESH_MHTZ };
                // Remove stale modes so wlr-randr doesn't accumulate one
                // entry per resize event.  Keep only the new current mode.
                for old in state.output.modes() {
                    if old != mode {
                        state.output.delete_mode(old);
                    }
                }
                state.output.change_current_state(Some(mode), None, None, None);
                state.output.set_preferred(mode);
            }
            let tracker_size = state.render_size();
            let render_scale = state.max_output_scale() * state.window_scale;
            damage_tracker = OutputDamageTracker::new(tracker_size, render_scale, WINIT_RENDER_TRANSFORM);
        }

        // Handle output configuration changes from wlr-output-management
        // (e.g. wlr-randr --scale 2). Rebuild the damage tracker and
        // reconfigure maximized/fullscreen windows for the new viewport.
        if state.output_config_changed {
            state.output_config_changed = false;

            // Notify output management clients (e.g. kanshi) about the change.
            crate::handlers::output_management::notify_output_config_changed(&mut state);

            state.reconfigure_windows_for_outputs();

            let new_render = state.render_size();
            {
                use smithay::reexports::winit::dpi::PhysicalSize;
                let _ = backend
                    .window()
                    .request_inner_size(PhysicalSize::new(new_render.w.unsigned_abs(), new_render.h.unsigned_abs()));
            }
            let new_scale = state.max_output_scale() * state.window_scale;
            damage_tracker = OutputDamageTracker::new(new_render, new_scale, WINIT_RENDER_TRANSFORM);

            tracing::debug!(
                w = new_render.w,
                h = new_render.h,
                scale = new_scale,
                "rebuilt damage tracker after output configuration change",
            );
        }

        // Always render so clients receive frame callbacks.  Without
        // continuous frame callbacks, clients like terminal emulators
        // won't update their display and input appears to be broken.
        render_frame(&mut backend, &mut damage_tracker, &mut state);

        // Final flush after rendering: sends frame callbacks and any
        // events generated during the render pass.
        if let Err(err) = state.display_handle.flush_clients() {
            tracing::warn!(%err, "failed to flush Wayland clients (post-render)");
        }
    }

    tracing::info!("compositor shutting down");
    Ok(())
}

/// Render one frame into the winit window.
fn render_frame(
    backend: &mut WinitGraphicsBackend<GlowRenderer>,
    damage_tracker: &mut OutputDamageTracker,
    state: &mut State,
) {
    let output = state.output.clone();

    // Bind the backend (borrows it for rendering)
    let damage = {
        let Ok((renderer, mut framebuffer)) = backend.bind() else {
            tracing::warn!("failed to bind winit backend for rendering");
            return;
        };

        // Lazy-init the glow-based titlebar painter on the first frame.
        // GlowRenderer provides the GL context for offscreen egui rendering.
        if !state.titlebar_renderer.is_glow_initialized() {
            state.titlebar_renderer.init_glow(renderer);
        }

        // Build the combined render element list with correct z-ordering.
        // Decorations are interleaved with window surfaces so that a
        // background window's title bar never paints on top of a
        // foreground window.
        //
        // Uses the GlowRenderer path: titlebars are GPU-resident
        // TextureRenderElements (no pixel readback).
        let render_elements = crate::render::collect_render_elements(renderer, state, &output, state.software_cursor);

        match damage_tracker.render_output(
            renderer,
            &mut framebuffer,
            0,
            &render_elements,
            crate::state::BACKGROUND_COLOR,
        ) {
            Ok(result) => result.damage.cloned(),
            Err(err) => {
                tracing::warn!(%err, "render_output failed");
                None
            }
        }
    };
    // Backend borrow released here

    if let Err(err) = backend.submit(damage.as_deref()) {
        tracing::warn!(%err, "failed to submit frame to winit backend");
    }

    // Determine effective cursor: compositor overrides (SSD resize/move) take
    // priority, then the client-requested cursor (via wp-cursor-shape or
    // wl_pointer.set_cursor), then default.
    //
    // When software_cursor is enabled, the cursor is composited into the
    // frame buffer (by collect_render_elements), so we always hide the host
    // cursor to avoid a double-cursor effect.  This is necessary for
    // screencopy/VNC scenarios where the host cursor is invisible.
    let compositor_cursor = state.compositor_cursor_shape;
    if state.software_cursor {
        // All cursor shapes are rendered as software elements — hide host cursor.
        backend.window().set_cursor_visible(false);
    } else if compositor_cursor == crate::decorations::CursorShape::Default {
        // Client-requested cursor (app hover states: text beam, pointer hand, etc.)
        use smithay::input::pointer::CursorImageStatus;
        match &state.cursor_status {
            CursorImageStatus::Hidden => {
                backend.window().set_cursor_visible(false);
            }
            CursorImageStatus::Named(icon) => {
                backend.window().set_cursor_visible(true);
                backend.window().set_cursor(*icon);
            }
            CursorImageStatus::Surface(_) => {
                // Client set a custom surface as cursor — we composite it
                // into the frame via collect_render_elements(), so hide
                // the host cursor to avoid a double-cursor effect.
                backend.window().set_cursor_visible(false);
            }
        }
    } else {
        // Compositor-driven cursor for SSD interactions (resize borders, etc.)
        let icon = match compositor_cursor {
            crate::decorations::CursorShape::Default | crate::decorations::CursorShape::Move => CursorIcon::Default,
            crate::decorations::CursorShape::ResizeN => CursorIcon::NResize,
            crate::decorations::CursorShape::ResizeS => CursorIcon::SResize,
            crate::decorations::CursorShape::ResizeW => CursorIcon::WResize,
            crate::decorations::CursorShape::ResizeE => CursorIcon::EResize,
            crate::decorations::CursorShape::ResizeNw => CursorIcon::NwResize,
            crate::decorations::CursorShape::ResizeNe => CursorIcon::NeResize,
            crate::decorations::CursorShape::ResizeSw => CursorIcon::SwResize,
            crate::decorations::CursorShape::ResizeSe => CursorIcon::SeResize,
        };
        backend.window().set_cursor_visible(true);
        backend.window().set_cursor(icon);
    }

    // Send frame callbacks to clients — use the output each window is on.
    state.send_frame_callbacks();
}
