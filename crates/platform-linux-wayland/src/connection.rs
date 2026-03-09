//! Wayland display connection management.
//!
//! Connects to the compositor via `wayland-client`, performs an initial
//! roundtrip to enumerate globals (including `wl_output` and optionally
//! `zxdg_output_manager_v1`), and stores it all in process-global state.
//!
//! After initialization a background event loop monitors the Wayland
//! socket for output changes (hot-plug, resolution, scaling, layout) and
//! automatically updates the desktop output state via
//! [`crate::desktop::set_outputs`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use platynui_core::platform::{PlatformError, PlatformErrorKind};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use tracing::{debug, info, warn};
use wayland_client::protocol::wl_output::{self, WlOutput};
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, globals::GlobalListContents};
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_manager_v1::ZxdgOutputManagerV1;
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_v1::{self, ZxdgOutputV1};

use crate::capabilities::CompositorType;
use crate::desktop::OutputInfo;

// ---------------------------------------------------------------------------
//  Dispatch state — collects globals + output info
// ---------------------------------------------------------------------------

/// Internal state passed to the `wayland-client` event queue.
struct RegistryState {
    outputs: HashMap<u32, (WlOutput, OutputInfo)>,
    xdg_output_manager: Option<ZxdgOutputManagerV1>,
    compositor: CompositorType,
    /// When `true`, `wl_output.done` and `global_remove` events trigger a
    /// live rebuild of the desktop output state. Stays `false` during the
    /// initial enumeration roundtrips to avoid wasteful D-Bus calls.
    live: bool,
}

impl RegistryState {
    /// Rebuild the sorted output list, enrich via D-Bus, and update the
    /// global desktop state. Only called when [`Self::live`] is `true`.
    fn rebuild_and_update_outputs(&self) {
        let mut outputs: Vec<OutputInfo> = self.outputs.values().map(|(_, o)| o.clone()).collect();
        outputs.sort_by_key(|o| (o.effective_x(), o.effective_y()));
        crate::desktop::display_config::enrich_outputs(self.compositor, &mut outputs);
        info!(count = outputs.len(), "desktop outputs updated (live)");
        crate::desktop::set_outputs(outputs);
    }
}

// -- Dispatch: WlRegistry (handles dynamic global additions/removals) --

impl Dispatch<WlRegistry, GlobalListContents> for RegistryState {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        // Note: initial globals are NOT forwarded here by `registry_queue_init`
        // (they are consumed during its internal roundtrip). This handler only
        // fires for globals added/removed *after* initialization. Initial
        // binding is done in `connect_and_enumerate()` via `GlobalList`.
        match event {
            wl_registry::Event::Global { name, interface, version } => match interface.as_str() {
                "wl_output" => {
                    let output = registry.bind::<WlOutput, _, _>(name, version.min(4), qh, name);
                    // Request xdg_output for the new output if manager is available.
                    if let Some(ref mgr) = state.xdg_output_manager {
                        let _xdg_out = mgr.get_xdg_output(&output, qh, name);
                    }
                    state.outputs.insert(name, (output, OutputInfo { scale: 1, ..OutputInfo::default() }));
                    debug!(global_name = name, "bound wl_output (dynamic)");
                }
                "zxdg_output_manager_v1" => {
                    let mgr = registry.bind::<ZxdgOutputManagerV1, _, _>(name, version.min(3), qh, ());
                    state.xdg_output_manager = Some(mgr);
                    debug!(global_name = name, "bound zxdg_output_manager_v1 (dynamic)");
                }
                _ => {}
            },
            wl_registry::Event::GlobalRemove { name } => {
                if state.outputs.remove(&name).is_some() {
                    info!(global_name = name, "wl_output removed (hot-unplug)");
                    if state.live {
                        state.rebuild_and_update_outputs();
                    }
                }
            }
            _ => {}
        }
    }
}

// -- Dispatch: WlOutput --

impl Dispatch<WlOutput, u32> for RegistryState {
    fn event(
        state: &mut Self,
        _proxy: &WlOutput,
        event: wl_output::Event,
        global_name: &u32,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Handle Done early — triggers a full output rebuild without holding
        // a mutable borrow on the individual OutputInfo.
        if matches!(event, wl_output::Event::Done) {
            debug!(global_name, "wl_output.done");
            if state.live {
                state.rebuild_and_update_outputs();
            }
            return;
        }

        let Some((_, info)) = state.outputs.get_mut(global_name) else { return };
        match event {
            wl_output::Event::Geometry { x, y, transform, .. } => {
                info.x = x;
                info.y = y;
                if let wayland_client::WEnum::Value(t) = transform {
                    info.transform = Some(t);
                }
            }
            wl_output::Event::Mode { flags, width, height, .. } => {
                // Only care about the current mode.
                if let wayland_client::WEnum::Value(flags) = flags
                    && flags.contains(wl_output::Mode::Current)
                {
                    info.width = width;
                    info.height = height;
                }
            }
            wl_output::Event::Scale { factor } => {
                info.scale = factor;
            }
            wl_output::Event::Name { name } => {
                info.name = Some(name);
            }
            wl_output::Event::Description { description } => {
                info.description = Some(description);
            }
            _ => {} // Done handled above
        }
    }
}

// -- Dispatch: ZxdgOutputManagerV1 (no events) --

impl Dispatch<ZxdgOutputManagerV1, ()> for RegistryState {
    fn event(
        _state: &mut Self,
        _proxy: &ZxdgOutputManagerV1,
        _event: <ZxdgOutputManagerV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

// -- Dispatch: ZxdgOutputV1 --

impl Dispatch<ZxdgOutputV1, u32> for RegistryState {
    fn event(
        state: &mut Self,
        _proxy: &ZxdgOutputV1,
        event: zxdg_output_v1::Event,
        global_name: &u32,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let Some((_, info)) = state.outputs.get_mut(global_name) else { return };
        match event {
            zxdg_output_v1::Event::LogicalPosition { x, y } => {
                info.logical_x = Some(x);
                info.logical_y = Some(y);
            }
            zxdg_output_v1::Event::LogicalSize { width, height } => {
                info.logical_width = Some(width);
                info.logical_height = Some(height);
            }
            zxdg_output_v1::Event::Name { name } => {
                // Deprecated since wl_output v4 — only use as fallback.
                if info.name.is_none() {
                    info.name = Some(name);
                }
            }
            zxdg_output_v1::Event::Description { description } => {
                // Deprecated since wl_output v4 — only use as fallback.
                if info.description.is_none() {
                    info.description = Some(description);
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
//  Global state + event loop session
// ---------------------------------------------------------------------------

/// Opaque handle returned by [`connect_and_enumerate`] carrying the event
/// queue and dispatch state. Passed to [`set_global_and_start`] to launch
/// the background event loop.
pub(crate) struct WaylandSession {
    event_queue: EventQueue<RegistryState>,
    state: RegistryState,
    // GlobalList must stay alive so that the WlRegistry proxy (and its
    // dispatch mapping) remains valid for ongoing Global/GlobalRemove events.
    globals: wayland_client::globals::GlobalList,
}

/// Process-global Wayland state populated during
/// [`crate::init::WaylandModule::initialize`].
struct WaylandGlobal {
    conn: Connection,
    compositor: CompositorType,
    shutdown: Arc<AtomicBool>,
}

static GLOBAL: Mutex<Option<WaylandGlobal>> = Mutex::new(None);

/// Connect to the Wayland display server, detect the compositor, and
/// enumerate outputs.
///
/// Returns the connection, detected compositor type, initial output
/// snapshot, and an opaque [`WaylandSession`] that carries the event queue
/// for subsequent live monitoring via [`set_global_and_start`].
///
/// # Errors
///
/// Returns `PlatformError` if the Wayland display connection or roundtrip fails.
pub(crate) fn connect_and_enumerate()
-> Result<(Connection, CompositorType, Vec<OutputInfo>, WaylandSession), PlatformError> {
    let conn = Connection::connect_to_env().map_err(|e| {
        PlatformError::new(
            PlatformErrorKind::InitializationFailed,
            format!("failed to connect to Wayland display: {e}"),
        )
    })?;

    let compositor = crate::capabilities::detect_compositor(&conn);

    let (globals, mut eq) = wayland_client::globals::registry_queue_init::<RegistryState>(&conn).map_err(|e| {
        PlatformError::new(PlatformErrorKind::InitializationFailed, format!("Wayland registry init failed: {e}"))
    })?;

    let qh = eq.handle();
    let mut state = RegistryState { outputs: HashMap::new(), xdg_output_manager: None, compositor, live: false };

    // registry_queue_init's internal roundtrip populates GlobalList but does NOT
    // forward initial Global events to our Dispatch<WlRegistry> handler. We must
    // use GlobalList to discover and bind globals directly.

    // Bind singleton: zxdg_output_manager_v1 (if available).
    if let Ok(mgr) = globals.bind::<ZxdgOutputManagerV1, _, _>(&qh, 1..=3, ()) {
        debug!("bound zxdg_output_manager_v1 via GlobalList");
        state.xdg_output_manager = Some(mgr);
    }

    // Bind multi-instance: all wl_output globals.
    let registry = globals.registry();
    for global in globals.contents().clone_list() {
        if global.interface == "wl_output" {
            let output = registry.bind::<WlOutput, _, _>(global.name, global.version.min(4), &qh, global.name);
            state.outputs.insert(global.name, (output, OutputInfo { scale: 1, ..OutputInfo::default() }));
            debug!(global_name = global.name, "bound wl_output via GlobalList");
        }
    }

    // First roundtrip: receives wl_output events (geometry, mode, scale, name, description, done).
    eq.roundtrip(&mut state).map_err(|e| {
        PlatformError::new(PlatformErrorKind::InitializationFailed, format!("Wayland roundtrip failed: {e}"))
    })?;

    // If xdg-output-manager is available, request xdg_output for each output.
    if let Some(ref mgr) = state.xdg_output_manager {
        for (&global_name, (wl_out, _)) in &state.outputs {
            // User data = global_name, so ZxdgOutputV1 dispatch routes to the right OutputInfo.
            let _xdg_out = mgr.get_xdg_output(wl_out, &qh, global_name);
        }

        // Second roundtrip: collects xdg_output events.
        eq.roundtrip(&mut state).map_err(|e| {
            PlatformError::new(
                PlatformErrorKind::InitializationFailed,
                format!("Wayland xdg-output roundtrip failed: {e}"),
            )
        })?;
    } else {
        warn!("zxdg_output_manager_v1 not available — using wl_output geometry/mode for monitor info");
    }

    // Clone the initial output snapshot (the originals stay in the state for
    // the live event loop).
    let mut outputs: Vec<OutputInfo> = state.outputs.values().map(|(_, o)| o.clone()).collect();
    outputs.sort_by_key(|o| (o.effective_x(), o.effective_y()));
    debug!(count = outputs.len(), "outputs enumerated");

    let session = WaylandSession { event_queue: eq, state, globals };

    Ok((conn, compositor, outputs, session))
}

/// Store the connection in global state and start the background event
/// loop that monitors for output changes (hot-plug, resolution, scaling).
///
/// The `session` carries the event queue and dispatch state produced by
/// [`connect_and_enumerate`]. Once started, the event loop automatically
/// calls [`crate::desktop::set_outputs`] whenever outputs change.
///
/// # Panics
///
/// Panics if the internal mutex is poisoned or if the dispatch thread
/// cannot be spawned.
pub(crate) fn set_global_and_start(conn: Connection, compositor: CompositorType, mut session: WaylandSession) {
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();
    let conn_clone = conn.clone();

    // Enable live output rebuilds now that initial setup is complete.
    session.state.live = true;

    thread::Builder::new()
        .name("wayland-events".to_string())
        .spawn(move || dispatch_loop(&conn_clone, session.event_queue, session.state, session.globals, &shutdown_clone))
        .expect("failed to spawn Wayland event loop thread");

    let mut guard = GLOBAL.lock().expect("wayland global mutex poisoned");
    *guard = Some(WaylandGlobal { conn, compositor, shutdown });
}

/// Signal the event loop to stop and clear global state.
///
/// The dispatch thread will exit on the next poll timeout (≤500 ms) or
/// immediately if it is currently idle. The thread is not joined — it is
/// lightweight and safe to abandon at process exit.
///
/// # Panics
///
/// Panics if the internal mutex is poisoned.
pub fn clear_global() {
    let mut guard = GLOBAL.lock().expect("wayland global mutex poisoned");
    if let Some(g) = guard.take() {
        g.shutdown.store(true, Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------------------
//  Background event loop
// ---------------------------------------------------------------------------

/// Background loop that polls the Wayland socket and dispatches events to
/// the [`RegistryState`] handlers.
///
/// Uses `prepare_read` + `poll` + `read` + `dispatch_pending` so we can
/// check the `shutdown` flag between iterations without blocking
/// indefinitely.
fn dispatch_loop(
    conn: &Connection,
    mut eq: EventQueue<RegistryState>,
    mut state: RegistryState,
    _globals: wayland_client::globals::GlobalList,
    shutdown: &AtomicBool,
) {
    debug!("Wayland event loop started");

    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        // prepare_read returns None when events are already buffered.
        if let Some(guard) = conn.prepare_read() {
            // Poll the Wayland fd with a 500 ms timeout so we can
            // periodically check the shutdown flag.
            let fd = guard.connection_fd();
            let mut pfd = [PollFd::new(&fd, PollFlags::IN)];
            let timeout = Timespec { tv_sec: 0, tv_nsec: 500_000_000 };
            match poll(&mut pfd, Some(&timeout)) {
                Ok(0) => {
                    // Timeout — no data. Drop the guard (cancels read)
                    // and loop back to check the shutdown flag.
                    drop(guard);
                    continue;
                }
                Ok(_) => {
                    // Data available — read it into the event queue's
                    // internal buffer.
                    if let Err(e) = guard.read() {
                        if !shutdown.load(Ordering::Relaxed) {
                            warn!(error = %e, "Wayland socket read error");
                        }
                        break;
                    }
                }
                Err(e) => {
                    // EINTR is harmless — retry on next iteration.
                    if e == rustix::io::Errno::INTR {
                        drop(guard);
                        continue;
                    }
                    if !shutdown.load(Ordering::Relaxed) {
                        warn!(error = %e, "poll error on Wayland fd");
                    }
                    break;
                }
            }
        }
        // else: Events already buffered — dispatch below handles them.

        // Dispatch all pending events to the RegistryState handlers.
        if let Err(e) = eq.dispatch_pending(&mut state) {
            if !shutdown.load(Ordering::Relaxed) {
                warn!(error = %e, "Wayland dispatch error");
            }
            break;
        }

        // Flush outgoing requests (e.g. xdg_output bindings for hot-plugged outputs).
        if let Err(e) = conn.flush() {
            if !shutdown.load(Ordering::Relaxed) {
                warn!(error = %e, "Wayland flush error");
            }
            break;
        }
    }

    debug!("Wayland event loop exiting");
}

// ---------------------------------------------------------------------------
//  Accessors
// ---------------------------------------------------------------------------

/// Access the global Wayland state.
///
/// # Panics
///
/// Panics if called before [`set_global_and_start`] (i.e. before platform
/// initialization) or if the internal mutex is poisoned.
#[allow(dead_code)]
pub fn with_global<F, R>(f: F) -> R
where
    F: FnOnce(&Connection, CompositorType) -> R,
{
    let guard = GLOBAL.lock().expect("wayland global mutex poisoned");
    let g = guard.as_ref().expect("Wayland platform not initialized — call initialize() first");
    f(&g.conn, g.compositor)
}

/// Return the detected compositor type.
///
/// # Panics
///
/// Panics if called before [`set_global_and_start`] or if the mutex is poisoned.
pub fn compositor() -> CompositorType {
    let guard = GLOBAL.lock().expect("wayland global mutex poisoned");
    let g = guard.as_ref().expect("Wayland platform not initialized — call initialize() first");
    g.compositor
}
