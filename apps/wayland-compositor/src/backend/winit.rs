//! Winit backend — nested compositor running in a window for development.
//!
//! Uses `smithay::backend::winit` with a GL renderer to display client surfaces
//! in a desktop window. Useful for interactive testing and debugging.

use std::sync::Arc;
use std::time::Duration;

use smithay::{
    backend::{
        renderer::{damage::OutputDamageTracker, glow::GlowRenderer},
        winit::{self, WinitEvent, WinitGraphicsBackend},
    },
    reexports::{
        calloop::{EventLoop, Interest, Mode, PostAction, generic::Generic},
        wayland_server::Display,
        winit::{platform::pump_events::PumpStatus, window::CursorIcon},
    },
    utils::{Physical, Size},
    wayland::socket::ListeningSocketSource,
};

use crate::{CompositorArgs, client::ClientState, config::CompositorConfig, state::State};

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
    let listening_socket = if let Some(ref name) = args.socket_name {
        ListeningSocketSource::with_name(name)?
    } else {
        ListeningSocketSource::new_auto()?
    };
    let socket_name = listening_socket.socket_name().to_string_lossy().into_owned();

    // Use the CLI-specified size as the per-output resolution. For single
    // output this matches the winit window; for multi-output each virtual
    // monitor gets these dimensions and the window is resized to fit.
    let output_size: Size<i32, Physical> = (args.width.cast_signed(), args.height.cast_signed()).into();
    let timeout = if args.timeout > 0 { Some(Duration::from_secs(args.timeout)) } else { None };

    // Register the Display as a calloop event source so that client messages
    // are processed *immediately* when data arrives on the fd, rather than
    // being deferred to a manual `dispatch_clients()` once per frame.
    // Both smithay's smallvil and cosmic-comp use this pattern — it is
    // essential for responsive popup/grab handling because GTK4 and Qt do
    // blocking `wl_display_roundtrip()` calls during popup creation.
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
        crate::security::SecurityPolicy::from_args(args.restrict_protocols.as_deref()),
        config,
    );
    state.backend_name = "winit";

    // Pre-initialize the screenshot renderer with a shared EGL context so
    // that screenshots can access the main renderer's GL textures (titlebar
    // textures, client surfaces) without cross-context errors.
    match super::create_shared_glow_renderer(backend.renderer()) {
        Ok(renderer) => state.screenshot_renderer = Some(renderer),
        Err(err) => tracing::warn!(%err, "failed to create shared screenshot renderer, will use standalone"),
    }

    event_loop.handle().insert_source(Generic::new(display, Interest::READ, Mode::Level), |_, display, state| {
        // SAFETY: the Display is owned by calloop for its entire lifetime;
        // we never drop it while calloop references it.  This pattern is
        // used by smithay's own smallvil example and cosmic-comp.
        #[allow(unsafe_code)]
        let result = unsafe { display.get_mut().dispatch_clients(state) };
        match result {
            Ok(_) => Ok(PostAction::Continue),
            Err(err) => {
                tracing::error!(%err, "I/O error dispatching Wayland clients");
                state.running = false;
                Err(err)
            }
        }
    })?;

    // Accept new client connections
    event_loop.handle().insert_source(listening_socket, |client_stream, (), state| {
        if let Err(err) = state.display_handle.insert_client(client_stream, Arc::new(ClientState::default())) {
            tracing::warn!(%err, "failed to insert new client");
        }
    })?;

    // Set WAYLAND_DISPLAY for child processes
    crate::environment::set_wayland_display(&socket_name);

    // Register signal handlers
    let shutdown = crate::signals::ShutdownFlag::register()?;

    // Register watchdog timer if requested
    if let Some(duration) = timeout {
        crate::signals::register_watchdog(&event_loop.handle(), duration)?;
    }

    // Start XWayland if requested — readiness notification is deferred until XWayland is ready
    let xwayland_requested = args.xwayland;

    if args.xwayland {
        // Store readiness parameters — they will be used when XWayland signals ready
        state.print_env = args.print_env;
        state.ready_fd = args.ready_fd;
        state.exit_with_child = args.exit_with_child;
        state.child_command.clone_from(&args.child_command);
        state.xwayland_shell_state =
            Some(smithay::wayland::xwayland_shell::XWaylandShellState::new::<State>(&state.display_handle));
        state.start_xwayland();
    }

    // Set up test-control IPC socket (enabled by default, before readiness
    // notification so PLATYNUI_CONTROL_SOCKET is available for --print-env
    // and child processes)
    if !args.no_control_socket {
        match crate::control::setup_control_socket(&event_loop.handle(), &socket_name) {
            Ok(control_path) => crate::environment::set_control_socket_env(&control_path),
            Err(err) => tracing::warn!(%err, "failed to set up control socket"),
        }
    }

    // Notify readiness and spawn child immediately if XWayland is not requested
    if !xwayland_requested {
        crate::ready::notify_ready(&socket_name, args.ready_fd, args.print_env);
        state.exit_with_child = args.exit_with_child;
        state.child_command.clone_from(&args.child_command);
        state.spawn_child_if_requested();
    }

    // Use a static damage tracker with Flipped180 to compensate for GL's
    // inverted Y-axis. We keep the output itself at Transform::Normal so
    // clients see the correct orientation.
    // With multi-output, the tracker covers the entire combined output area
    // so windows on any output are rendered correctly.
    let combined_geo = state.combined_output_geometry();
    let tracker_size: Size<i32, Physical> = (combined_geo.size.w, combined_geo.size.h).into();

    // Resize the winit window to the combined output area so all virtual
    // monitors are visible when --outputs > 1.
    if combined_geo.size.w != output_size.w || combined_geo.size.h != output_size.h {
        use smithay::reexports::winit::dpi::PhysicalSize;
        let _ = backend.window().request_inner_size(PhysicalSize::new(
            combined_geo.size.w.unsigned_abs(),
            combined_geo.size.h.unsigned_abs(),
        ));
        tracing::info!(
            width = combined_geo.size.w,
            height = combined_geo.size.h,
            outputs = state.outputs.len(),
            "resized winit window for multi-output",
        );
    }

    let mut damage_tracker = OutputDamageTracker::new(tracker_size, 1.0, WINIT_RENDER_TRANSFORM);

    tracing::info!(backend = "winit", socket = %socket_name, "event loop starting");

    // Main event loop
    while state.running && !shutdown.is_set() {
        // Collect winit events without borrowing state in the closure
        let mut close_requested = false;
        let mut input_events = Vec::new();
        let mut new_size: Option<Size<i32, Physical>> = None;

        let pump_status = winit_evt.dispatch_new_events(|event| match event {
            WinitEvent::Resized { size, .. } => {
                new_size = Some(size);
            }
            WinitEvent::Focus(_) | WinitEvent::Redraw => {}
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

        // Process collected input events
        for input in input_events {
            crate::input::process_input_event(&mut state, input);
        }

        // Handle output resize
        if let Some(size) = new_size {
            let mode = smithay::output::Mode { size, refresh: 60_000 };
            state.output.change_current_state(Some(mode), None, None, None);
            // Recalculate damage tracker for the (possibly multi-output) combined area.
            let combined = state.combined_output_geometry();
            let tracker_size: Size<i32, Physical> = (combined.size.w, combined.size.h).into();
            damage_tracker = OutputDamageTracker::new(tracker_size, 1.0, WINIT_RENDER_TRANSFORM);
        }

        // Always render so clients receive frame callbacks.  Without
        // continuous frame callbacks, clients like terminal emulators
        // won't update their display and input appears to be broken.
        render_frame(&mut backend, &mut damage_tracker, &mut state);

        // Dispatch calloop sources: the Display source reads client messages
        // and calls `dispatch_clients()` when data arrives; the socket
        // listener accepts new connections; timers/watchdog fire.
        event_loop.dispatch(Some(Duration::from_millis(1)), &mut state)?;

        // Refresh the space *before* flushing so that `wl_surface.enter`
        // events (sent by `output_update` inside `Window::refresh`) are
        // included in the same flush as configure / sync-callback events.
        // This is critical because GTK4 needs the output enter to determine
        // the scale factor before rendering a popup surface.
        state.space.refresh();
        state.popup_manager.cleanup();

        // Flush all queued events to clients.  This sends protocol events
        // (configure, frame callbacks, input events) that were queued during
        // this iteration.
        if let Err(err) = state.display_handle.flush_clients() {
            tracing::warn!(%err, "failed to flush Wayland clients");
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
        let render_elements = crate::render::collect_render_elements(renderer, state, &output);

        match damage_tracker.render_output(renderer, &mut framebuffer, 0, &render_elements, [0.1, 0.1, 0.1, 1.0]) {
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
    let compositor_cursor = state.compositor_cursor_shape;
    if compositor_cursor == crate::decorations::CursorShape::Default {
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
    let now = state.frame_clock_now();
    for window in state.space.elements() {
        let win_output = state
            .output_at_point({
                let loc = state.space.element_location(window).unwrap_or_default();
                let size = window.geometry().size;
                (f64::from(loc.x + size.w / 2), f64::from(loc.y + size.h / 2)).into()
            })
            .clone();
        window.send_frame(&win_output, now, Some(Duration::ZERO), |_, _| Some(win_output.clone()));
    }
}
