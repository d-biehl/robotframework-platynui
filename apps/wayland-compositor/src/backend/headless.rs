//! Headless backend — off-screen rendering for CI environments.
//!
//! The event loop processes Wayland client requests without any visible output.
//! Screenshots use a lazily-initialized [`GlowRenderer`] (EGL on a DRI render
//! node).  Set `LIBGL_ALWAYS_SOFTWARE=1` for environments without a hardware GPU.

use std::time::Duration;

/// Event loop dispatch timeout — one frame period at ~60 FPS.
const FRAME_DISPATCH_TIMEOUT: Duration = Duration::from_millis(16);

use smithay::{
    reexports::{calloop::EventLoop, wayland_server::Display},
    utils::{Physical, Size},
};

use crate::{CompositorArgs, config::CompositorConfig, state::State};

/// Run the compositor in headless mode.
///
/// # Errors
///
/// Returns an error if socket creation, event loop setup, or runtime fails.
pub fn run(args: &CompositorArgs, config: CompositorConfig) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop: EventLoop<'static, State> = EventLoop::try_new()?;
    let display: Display<State> = Display::new()?;

    let (listening_socket, socket_name) = super::create_listening_socket(args)?;

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
    state.backend_name = "headless";

    // Register Wayland display + listening socket + set WAYLAND_DISPLAY
    super::register_wayland_sources(&event_loop.handle(), display, listening_socket, &socket_name)?;

    // Register signal handlers, watchdog, XWayland, control socket, readiness
    let shutdown = super::setup_services(&event_loop.handle(), &mut state, args, timeout)?;

    tracing::info!(backend = "headless", socket = %socket_name, "event loop starting");

    // Main event loop — protocol dispatch + frame callbacks.
    while state.running && !shutdown.is_set() {
        event_loop.dispatch(Some(FRAME_DISPATCH_TIMEOUT), &mut state)?;

        state.send_frame_callbacks();
        state.flush_and_refresh();
    }

    tracing::info!("compositor shutting down");
    Ok(())
}
