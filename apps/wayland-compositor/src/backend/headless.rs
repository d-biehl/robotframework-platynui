//! Headless backend — off-screen rendering for CI environments.
//!
//! The event loop processes Wayland client requests without any visible output.
//! Screenshots use a lazily-initialized [`GlowRenderer`] (EGL on a DRI render
//! node).  Set `LIBGL_ALWAYS_SOFTWARE=1` for environments without a hardware GPU.

use std::sync::Arc;
use std::time::Duration;

use smithay::{
    reexports::{
        calloop::{EventLoop, Interest, Mode, PostAction, generic::Generic},
        wayland_server::Display,
    },
    utils::{Physical, Size},
    wayland::socket::ListeningSocketSource,
};

use crate::{CompositorArgs, client::ClientState, config::CompositorConfig, state::State};

/// Run the compositor in headless mode.
///
/// # Errors
///
/// Returns an error if socket creation, event loop setup, or runtime fails.
pub fn run(args: &CompositorArgs, config: CompositorConfig) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop: EventLoop<'static, State> = EventLoop::try_new()?;
    let display: Display<State> = Display::new()?;

    // Create the listening socket
    let listening_socket = if let Some(ref name) = args.socket_name {
        ListeningSocketSource::with_name(name)?
    } else {
        ListeningSocketSource::new_auto()?
    };
    let socket_name = listening_socket.socket_name().to_string_lossy().into_owned();

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
        crate::security::SecurityPolicy::from_args(args.restrict_protocols.as_deref()),
        config,
    );
    state.backend_name = "headless";

    // Register the Display as a calloop event source so that client messages
    // are processed immediately when data arrives on the fd.
    event_loop.handle().insert_source(Generic::new(display, Interest::READ, Mode::Level), |_, display, state| {
        // SAFETY: the Display is owned by calloop for its entire lifetime.
        // This pattern is used by smithay's own smallvil example.
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

    tracing::info!(backend = "headless", socket = %socket_name, "event loop starting");

    // Main event loop — protocol dispatch + frame callbacks.
    while state.running && !shutdown.is_set() {
        event_loop.dispatch(Some(Duration::from_millis(16)), &mut state)?;

        // Send frame callbacks to all mapped windows and their popups.
        // Without frame callbacks, clients that block on the next frame
        // (like GTK4 during popup creation) would hang indefinitely.
        let now = state.frame_clock_now();
        for window in state.space.elements() {
            let output = state
                .output_at_point({
                    let loc = state.space.element_location(window).unwrap_or_default();
                    let size = window.geometry().size;
                    (f64::from(loc.x + size.w / 2), f64::from(loc.y + size.h / 2)).into()
                })
                .clone();
            window.send_frame(&output, now, Some(Duration::ZERO), |_, _| Some(output.clone()));
        }

        // Refresh the space *before* flushing so that `wl_surface.enter`
        // events are included in the same flush as configure / sync events.
        state.space.refresh();
        state.popup_manager.cleanup();

        if let Err(err) = state.display_handle.flush_clients() {
            tracing::warn!(%err, "failed to flush Wayland clients");
        }
    }

    tracing::info!("compositor shutting down");
    Ok(())
}
