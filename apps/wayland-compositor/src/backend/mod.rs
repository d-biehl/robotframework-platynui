//! Backend abstraction — headless, winit, and DRM backends.
//!
//! Each backend module provides a `run()` function that:
//! 1. Creates the appropriate renderer and output
//! 2. Sets up the Wayland display and socket
//! 3. Runs the event loop
//! 4. Handles rendering and frame callbacks

pub mod drm;
pub mod headless;
pub mod winit;

/// Create a `GlowRenderer` that shares the GL object namespace with an existing renderer.
///
/// Uses [`EGLContext::new_shared`] to derive a new EGL context that shares
/// textures, programs, sync objects, and other GL resources with the parent.
/// Smithay automatically propagates the parent's [`ContextId`] via shared
/// `user_data`, so texture-compatibility checks pass without extra work.
///
/// This is the preferred way to create a screenshot renderer when a main
/// renderer already exists (winit or DRM backends).
///
/// # Errors
///
/// Returns an error if EGL context sharing or GL initialization fails.
pub fn create_shared_glow_renderer(
    parent: &smithay::backend::renderer::glow::GlowRenderer,
) -> Result<smithay::backend::renderer::glow::GlowRenderer, Box<dyn std::error::Error>> {
    use smithay::backend::egl::EGLContext;
    use smithay::backend::renderer::glow::GlowRenderer;

    let parent_ctx = parent.egl_context();
    let shared_ctx = EGLContext::new_shared(parent_ctx.display(), parent_ctx)?;

    #[allow(unsafe_code)]
    // SAFETY: The shared EGLContext is valid and has the same GL namespace as the parent.
    let renderer = unsafe { GlowRenderer::new(shared_ctx)? };

    Ok(renderer)
}

/// Create a standalone `GlowRenderer` for offscreen use (screenshots, headless).
///
/// Prefers a real DRI render node (e.g. `/dev/dri/renderD128`) via GBM, which is
/// GPU-accelerated and also works with llvmpipe when `LIBGL_ALWAYS_SOFTWARE=1` is
/// set. When no render node exists — e.g. a headless CI runner without a GPU —
/// GBM is not an option, since it wraps a kernel DRM device and *requires* a
/// `/dev/dri/*` node. We then fall back to Mesa's surfaceless EGL platform, which
/// renders into client memory via llvmpipe and needs no device node at all.
///
/// # Errors
///
/// Returns an error if neither the GBM nor the surfaceless EGL path can be
/// initialized (e.g. Mesa is not available), or if GL setup fails.
pub fn create_offscreen_glow_renderer()
-> Result<smithay::backend::renderer::glow::GlowRenderer, Box<dyn std::error::Error>> {
    use smithay::backend::egl::EGLContext;
    use smithay::backend::renderer::glow::GlowRenderer;

    let egl_display = create_offscreen_egl_display()?;
    let egl_context = EGLContext::new(&egl_display)?;

    #[allow(unsafe_code)]
    // SAFETY: The EGLContext is valid.  GlowRenderer manages GL state internally.
    let renderer = unsafe { GlowRenderer::new(egl_context)? };

    Ok(renderer)
}

/// Build the offscreen [`EGLDisplay`]: GBM on a real render node, else surfaceless.
///
/// A surfaceless context is enough here because the offscreen renderer composites
/// into an FBO and never creates a window surface, so no DRI device is required.
fn create_offscreen_egl_display() -> Result<smithay::backend::egl::EGLDisplay, Box<dyn std::error::Error>> {
    use smithay::backend::egl::{EGLDisplay, native::EGLSurfacelessDisplay};

    // 1) Preferred: a DRI render node via GBM (GPU-accelerated; also fine with
    //    llvmpipe when LIBGL_ALWAYS_SOFTWARE=1).
    if let Ok(render_node) = find_render_node() {
        match open_gbm_egl_display(&render_node) {
            Ok(display) => return Ok(display),
            Err(e) => tracing::warn!(
                "GBM EGL display on {} failed ({e}); falling back to surfaceless software EGL",
                render_node.display()
            ),
        }
    } else {
        tracing::info!("no DRI render node found; using surfaceless software EGL (llvmpipe) for offscreen rendering");
    }

    // 2) Fallback: Mesa's surfaceless platform — no device node required.
    #[allow(unsafe_code)]
    // SAFETY: EGLSurfacelessDisplay selects EGL_PLATFORM_SURFACELESS_MESA with
    // EGL_DEFAULT_DISPLAY; there is no native window/device handle to keep alive.
    let display = unsafe { EGLDisplay::new(EGLSurfacelessDisplay)? };
    Ok(display)
}

/// Open a GBM-backed [`EGLDisplay`] on a specific DRI render node.
fn open_gbm_egl_display(
    render_node: &std::path::Path,
) -> Result<smithay::backend::egl::EGLDisplay, Box<dyn std::error::Error>> {
    use smithay::backend::allocator::gbm::GbmDevice;
    use smithay::backend::egl::EGLDisplay;

    let fd = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(render_node)
        .map_err(|e| format!("failed to open {}: {e}", render_node.display()))?;

    let gbm = GbmDevice::new(fd)?;

    #[allow(unsafe_code)]
    // SAFETY: GbmDevice implements EGLNativeDisplay — standard EGL-on-GBM init.
    let display = unsafe { EGLDisplay::new(gbm)? };
    Ok(display)
}

/// Find the first available DRI render node (e.g. `/dev/dri/renderD128`).
fn find_render_node() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let dri_dir = std::path::Path::new("/dev/dri");
    if !dri_dir.exists() {
        return Err("no /dev/dri directory found — is a GPU or Mesa available?".into());
    }

    let mut entries: Vec<_> = std::fs::read_dir(dri_dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("renderD")))
        .collect();

    entries.sort();
    entries.into_iter().next().ok_or_else(|| {
        "no DRI render node found (e.g. /dev/dri/renderD128) — \
         install Mesa or set LIBGL_ALWAYS_SOFTWARE=1"
            .into()
    })
}

/// Create the Wayland listening socket from CLI arguments.
///
/// Returns the socket source and human-readable socket name.
///
/// # Errors
///
/// Returns an error if socket creation fails.
pub fn create_listening_socket(
    args: &crate::CompositorArgs,
) -> Result<(smithay::wayland::socket::ListeningSocketSource, String), Box<dyn std::error::Error>> {
    use smithay::wayland::socket::ListeningSocketSource;

    let listening_socket = if let Some(ref name) = args.socket_name {
        ListeningSocketSource::with_name(name)?
    } else {
        ListeningSocketSource::new_auto()?
    };
    let socket_name = listening_socket.socket_name().to_string_lossy().into_owned();
    Ok((listening_socket, socket_name))
}

/// Register the Wayland display and listening socket as calloop event sources.
///
/// Inserts both the [`Display`] (for client message dispatch) and the
/// [`ListeningSocketSource`] (for new client connections) into the event loop,
/// then sets `WAYLAND_DISPLAY` for child processes.
///
/// # Errors
///
/// Returns an error if registering either source fails.
pub fn register_wayland_sources(
    handle: &smithay::reexports::calloop::LoopHandle<'static, crate::state::State>,
    display: smithay::reexports::wayland_server::Display<crate::state::State>,
    listening_socket: smithay::wayland::socket::ListeningSocketSource,
    socket_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::Arc;

    use smithay::reexports::calloop::{Interest, Mode, PostAction, generic::Generic};

    use crate::client::ClientState;

    // Register the Display so client messages are processed immediately
    // when data arrives on the fd.  Both smithay's smallvil example and
    // cosmic-comp use this pattern.
    handle.insert_source(Generic::new(display, Interest::READ, Mode::Level), |_, display, state| {
        // SAFETY: the Display is owned by calloop for its entire lifetime.
        #[allow(unsafe_code)]
        let result = unsafe { display.get_mut().dispatch_clients(state) };
        match result {
            Ok(_) => {
                let _ = state.display_handle.flush_clients();
                Ok(PostAction::Continue)
            }
            Err(err) => {
                tracing::error!(%err, "I/O error dispatching Wayland clients");
                state.running = false;
                Err(err)
            }
        }
    })?;

    // Accept new client connections
    handle.insert_source(listening_socket, |client_stream, (), state| {
        if let Err(err) = state.display_handle.insert_client(client_stream, Arc::new(ClientState::default())) {
            tracing::warn!(%err, "failed to insert new client");
        }
    })?;

    // Set WAYLAND_DISPLAY for child processes
    crate::environment::set_wayland_display(socket_name);

    Ok(())
}

/// Set up common services: signal handlers, watchdog, `XWayland`, control socket,
/// and readiness notification.
///
/// Call this after all backend-specific event sources have been registered.
///
/// # Errors
///
/// Returns an error if signal or timer registration fails.
pub fn setup_services(
    handle: &smithay::reexports::calloop::LoopHandle<'static, crate::state::State>,
    state: &mut crate::state::State,
    args: &crate::CompositorArgs,
    timeout: Option<std::time::Duration>,
) -> Result<crate::signals::ShutdownFlag, Box<dyn std::error::Error>> {
    // Register signal handlers
    let shutdown = crate::signals::ShutdownFlag::register()?;

    // Register watchdog timer if requested
    if let Some(duration) = timeout {
        crate::signals::register_watchdog(handle, duration)?;
    }

    // Start XWayland if requested — readiness notification is deferred until
    // XWayland is ready.
    let xwayland_requested = args.xwayland;

    if args.xwayland {
        state.print_env = args.print_env;
        state.ready_fd = args.ready_fd;
        state.exit_with_child = args.exit_with_child;
        state.child_command.clone_from(&args.child_command);
        state.xwayland_shell_state = Some(smithay::wayland::xwayland_shell::XWaylandShellState::new::<
            crate::state::State,
        >(&state.display_handle));
        state.xwayland_keyboard_grab_state =
            Some(smithay::wayland::xwayland_keyboard_grab::XWaylandKeyboardGrabState::new::<crate::state::State>(
                &state.display_handle,
            ));
        state.start_xwayland();
    }

    // Set up test-control IPC socket (before readiness notification so that
    // PLATYNUI_CONTROL_SOCKET is available for --print-env and child processes).
    if !args.no_control_socket {
        match crate::control::setup_control_socket(handle, &state.socket_name) {
            Ok(control_path) => crate::environment::set_control_socket_env(&control_path),
            Err(err) => tracing::warn!(%err, "failed to set up control socket"),
        }
    }

    // Set up EIS server for libei input emulation.
    if !args.no_eis {
        match crate::eis::setup_eis_server(handle) {
            Ok(eis_path) => crate::environment::set_eis_socket_env(&eis_path),
            Err(err) => tracing::warn!(%err, "failed to set up EIS server"),
        }
    }

    // Notify readiness and spawn child immediately if XWayland is not requested
    if !xwayland_requested {
        crate::ready::notify_ready(&state.socket_name, args.ready_fd, args.print_env);
        state.exit_with_child = args.exit_with_child;
        state.child_command.clone_from(&args.child_command);
        state.spawn_child_if_requested();
    }

    Ok(shutdown)
}
