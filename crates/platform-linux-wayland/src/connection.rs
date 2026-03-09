//! Wayland display connection management.
//!
//! Connects to the compositor via `wayland-client`, performs an initial
//! roundtrip to enumerate globals (including `wl_output` and optionally
//! `zxdg_output_manager_v1`), and stores it all in process-global state.

use std::collections::HashMap;
use std::sync::Mutex;

use platynui_core::platform::{PlatformError, PlatformErrorKind};
use tracing::{debug, warn};
use wayland_client::protocol::wl_output::{self, WlOutput};
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::{Connection, Dispatch, QueueHandle, globals::GlobalListContents};
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_manager_v1::ZxdgOutputManagerV1;
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_v1::{self, ZxdgOutputV1};

use crate::capabilities::CompositorType;

// ---------------------------------------------------------------------------
//  Per-output collected state
// ---------------------------------------------------------------------------

/// Information collected from `wl_output` and optionally `zxdg_output_v1` events.
#[derive(Debug, Clone, Default)]
pub struct OutputInfo {
    /// Physical position in compositor-global coordinates.
    pub x: i32,
    pub y: i32,
    /// Current mode dimensions (hardware pixels).
    pub width: i32,
    pub height: i32,
    /// Scale factor advertised by the compositor.
    pub scale: i32,
    /// Output transform (rotation/flip) from `wl_output.geometry`.
    pub transform: Option<wl_output::Transform>,
    /// Human-readable name (from `wl_output.name` since v4 or `zxdg_output_v1.name`).
    pub name: Option<String>,
    /// Human-readable description.
    pub description: Option<String>,
    /// Logical position from `xdg-output` (preferred over geometry x/y).
    pub logical_x: Option<i32>,
    pub logical_y: Option<i32>,
    /// Logical size from `xdg-output`.
    pub logical_width: Option<i32>,
    pub logical_height: Option<i32>,
}

impl OutputInfo {
    /// Whether the output transform involves a 90° or 270° rotation,
    /// which swaps width and height.
    fn is_rotated(&self) -> bool {
        matches!(
            self.transform,
            Some(
                wl_output::Transform::_90
                    | wl_output::Transform::_270
                    | wl_output::Transform::Flipped90
                    | wl_output::Transform::Flipped270
            )
        )
    }

    /// Effective position — prefers xdg-output logical, falls back to `wl_output` geometry.
    #[must_use]
    pub fn effective_x(&self) -> i32 {
        self.logical_x.unwrap_or(self.x)
    }

    #[must_use]
    pub fn effective_y(&self) -> i32 {
        self.logical_y.unwrap_or(self.y)
    }

    /// Effective size — prefers xdg-output logical, falls back to mode / scale
    /// (accounting for output transform).
    #[must_use]
    pub fn effective_width(&self) -> i32 {
        self.logical_width.unwrap_or_else(|| {
            let hw = if self.is_rotated() { self.height } else { self.width };
            if self.scale > 0 { hw / self.scale } else { hw }
        })
    }

    #[must_use]
    pub fn effective_height(&self) -> i32 {
        self.logical_height.unwrap_or_else(|| {
            let hw = if self.is_rotated() { self.width } else { self.height };
            if self.scale > 0 { hw / self.scale } else { hw }
        })
    }
}

// ---------------------------------------------------------------------------
//  Dispatch state — collects globals + output info
// ---------------------------------------------------------------------------

/// Internal state passed to the `wayland-client` event queue.
struct RegistryState {
    outputs: HashMap<u32, (WlOutput, OutputInfo)>,
    xdg_output_manager: Option<ZxdgOutputManagerV1>,
}

// -- Dispatch: WlRegistry (handles dynamic global additions after init) --

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
        if let wl_registry::Event::Global { name, interface, version } = event {
            match interface.as_str() {
                "wl_output" => {
                    let output = registry.bind::<WlOutput, _, _>(name, version.min(4), qh, name);
                    state.outputs.insert(name, (output, OutputInfo { scale: 1, ..OutputInfo::default() }));
                    debug!(global_name = name, "bound wl_output (dynamic)");
                }
                "zxdg_output_manager_v1" => {
                    let mgr = registry.bind::<ZxdgOutputManagerV1, _, _>(name, version.min(3), qh, ());
                    state.xdg_output_manager = Some(mgr);
                    debug!(global_name = name, "bound zxdg_output_manager_v1 (dynamic)");
                }
                _ => {}
            }
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
            wl_output::Event::Done => {
                debug!(global_name, ?info, "wl_output.done");
            }
            _ => {}
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
//  Global state
// ---------------------------------------------------------------------------

/// Process-global Wayland state populated during
/// [`crate::init::WaylandModule::initialize`].
struct WaylandGlobal {
    conn: Connection,
    compositor: CompositorType,
    outputs: Vec<OutputInfo>,
}

static GLOBAL: Mutex<Option<WaylandGlobal>> = Mutex::new(None);

/// Connect to the Wayland display server and enumerate outputs.
///
/// Uses `registry_queue_init` to discover globals, then binds all `wl_output`
/// instances and (if available) `zxdg_output_v1` to collect monitor data.
///
/// # Errors
///
/// Returns `PlatformError` if the Wayland display connection or roundtrip fails.
pub fn connect_and_enumerate() -> Result<(Connection, Vec<OutputInfo>), PlatformError> {
    let conn = Connection::connect_to_env().map_err(|e| {
        PlatformError::new(
            PlatformErrorKind::InitializationFailed,
            format!("failed to connect to Wayland display: {e}"),
        )
    })?;

    let (globals, mut eq) = wayland_client::globals::registry_queue_init::<RegistryState>(&conn).map_err(|e| {
        PlatformError::new(PlatformErrorKind::InitializationFailed, format!("Wayland registry init failed: {e}"))
    })?;

    let qh = eq.handle();
    let mut state = RegistryState { outputs: HashMap::new(), xdg_output_manager: None };

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

    // Keep globals (holds the WlRegistry) alive — drop would not affect the objects already bound,
    // but retaining it is cleaner.
    drop(globals);

    let outputs: Vec<OutputInfo> = state.outputs.into_values().map(|(_, info)| info).collect();
    debug!(count = outputs.len(), "outputs enumerated");

    Ok((conn, outputs))
}

/// Store connection, compositor type, and output info for global access.
///
/// # Panics
///
/// Panics if the internal mutex is poisoned.
pub fn set_global(conn: Connection, compositor: CompositorType, outputs: Vec<OutputInfo>) {
    let mut guard = GLOBAL.lock().expect("wayland global mutex poisoned");
    *guard = Some(WaylandGlobal { conn, compositor, outputs });
}

/// Clear global state during shutdown.
///
/// # Panics
///
/// Panics if the internal mutex is poisoned.
pub fn clear_global() {
    let mut guard = GLOBAL.lock().expect("wayland global mutex poisoned");
    *guard = None;
}

/// Access the global Wayland state.
///
/// # Panics
///
/// Panics if called before [`set_global`] (i.e. before platform initialization)
/// or if the internal mutex is poisoned.
#[allow(dead_code)]
pub fn with_global<F, R>(f: F) -> R
where
    F: FnOnce(&Connection, CompositorType) -> R,
{
    let guard = GLOBAL.lock().expect("wayland global mutex poisoned");
    let g = guard.as_ref().expect("Wayland platform not initialized — call initialize() first");
    f(&g.conn, g.compositor)
}

/// Access the collected output info.
///
/// # Panics
///
/// Panics if called before [`set_global`] or if the mutex is poisoned.
pub fn outputs() -> Vec<OutputInfo> {
    let guard = GLOBAL.lock().expect("wayland global mutex poisoned");
    let g = guard.as_ref().expect("Wayland platform not initialized — call initialize() first");
    g.outputs.clone()
}
