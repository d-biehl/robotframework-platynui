//! Headless backend — off-screen rendering for CI environments.
//!
//! The event loop processes Wayland client requests without any visible output.
//! A periodic timer sends frame callbacks so clients that block on the next
//! frame (like GTK4 during popup creation) don't hang indefinitely.
//! Screenshots use a lazily-initialized [`GlowRenderer`] (EGL on a DRI render
//! node).  Set `LIBGL_ALWAYS_SOFTWARE=1` for environments without a hardware GPU.

use std::time::Duration;

use smithay::{
    reexports::{
        calloop::{
            EventLoop,
            timer::{TimeoutAction, Timer},
        },
        wayland_server::Display,
    },
    utils::{Physical, Size},
};

use crate::{CompositorArgs, config::CompositorConfig, state::State};

/// Frame callback interval — one frame period at ~60 FPS.
const FRAME_INTERVAL: Duration = Duration::from_millis(16);

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
    state.init_dmabuf_with_fallback_formats();

    // Register Wayland display + listening socket + set WAYLAND_DISPLAY
    super::register_wayland_sources(&event_loop.handle(), display, listening_socket, &socket_name)?;

    // Register signal handlers, watchdog, XWayland, control socket, readiness
    let shutdown = super::setup_services(&event_loop.handle(), &mut state, args, timeout)?;

    // Periodic wake-up so the idle callback runs at least once per frame
    // period, even when no client events arrive.  This prevents clients that
    // wait for a frame callback before their first commit from hanging.
    event_loop.handle().insert_source(Timer::from_duration(FRAME_INTERVAL), |_, (), _state| {
        TimeoutAction::ToDuration(FRAME_INTERVAL)
    })?;

    tracing::info!(backend = "headless", socket = %socket_name, "event loop starting");

    // The idle callback runs after every event-loop iteration (including
    // timer wake-ups), so frame callbacks go out as soon as a client commits
    // rather than waiting for the next 16 ms tick.
    event_loop.run(None, &mut state, |state| {
        state.send_frame_callbacks(Duration::ZERO);
        state.flush_and_refresh();

        if !state.running || shutdown.is_set() {
            state.loop_signal.stop();
        }
    })?;

    tracing::info!("compositor shutting down");
    Ok(())
}
