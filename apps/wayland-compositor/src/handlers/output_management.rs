//! Output management protocol handler (`wlr-output-management-unstable-v1`).
//!
//! Implements `zwlr_output_manager_v1` (v4) so tools like `wlr-randr` and
//! `kanshi` can query and reconfigure outputs at runtime.  Since smithay
//! has no built-in support, we implement [`GlobalDispatch`] and [`Dispatch`]
//! manually for all four protocol interfaces.
//!
//! ## Protocol flow
//!
//! 1. On bind, the compositor sends `head` events for each output, followed
//!    by head property events (name, description, modes, enabled, position,
//!    scale, transform), then a `done` event carrying a serial.
//! 2. Clients create a configuration via `create_configuration(serial)`.
//! 3. For each head, clients call `enable_head` (with property overrides) or
//!    `disable_head`.
//! 4. Clients call `apply` or `test`.
//! 5. The compositor responds with `succeeded`, `failed`, or `cancelled`.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU32, Ordering},
};

use smithay::{
    output::{Mode, Output},
    reexports::{
        wayland_protocols_wlr::output_management::v1::server::{
            zwlr_output_configuration_head_v1::{self, ZwlrOutputConfigurationHeadV1},
            zwlr_output_configuration_v1::{self, ZwlrOutputConfigurationV1},
            zwlr_output_head_v1::{self, ZwlrOutputHeadV1},
            zwlr_output_manager_v1::{self, ZwlrOutputManagerV1},
            zwlr_output_mode_v1::ZwlrOutputModeV1,
        },
        wayland_server::{
            Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource, WEnum, Weak,
            backend::{ClientId, GlobalId},
            protocol::wl_output::Transform,
        },
    },
    utils::Transform as SmithayTransform,
};

use crate::state::State;

// ---------------------------------------------------------------------------
// Serial counter for configuration invalidation
// ---------------------------------------------------------------------------

/// Monotonically increasing serial for output configuration changes.
static CONFIG_SERIAL: AtomicU32 = AtomicU32::new(1);

fn next_serial() -> u32 {
    CONFIG_SERIAL.fetch_add(1, Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// Manager global
// ---------------------------------------------------------------------------

/// Data attached to the `zwlr_output_manager_v1` global.
pub struct OutputManagementGlobalData {
    filter: Box<dyn Fn(&Client) -> bool + Send + Sync>,
}

/// Per-manager-instance data.
///
/// The inner `Vec`s use `Mutex` for interior mutability — the `bind`
/// handler receives `&OutputManagerData` (shared ref) but needs to push
/// newly created head/mode objects.
pub struct OutputManagerData {
    /// Head objects sent to this client (for updates).
    heads: Mutex<Vec<(Output, ZwlrOutputHeadV1)>>,
    /// Mode objects sent to this client (for `current_mode` references).
    modes: Mutex<Vec<(Output, Mode, ZwlrOutputModeV1)>>,
}

impl GlobalDispatch<ZwlrOutputManagerV1, OutputManagementGlobalData> for State {
    fn bind(
        state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<ZwlrOutputManagerV1>,
        _global_data: &OutputManagementGlobalData,
        data_init: &mut DataInit<'_, Self>,
    ) {
        let manager = data_init
            .init(resource, OutputManagerData { heads: Mutex::new(Vec::new()), modes: Mutex::new(Vec::new()) });

        // Send current state for all outputs.
        send_all_heads(state, &manager);

        let serial = CONFIG_SERIAL.load(Ordering::Relaxed);
        manager.done(serial);

        // Track this manager instance for output reconfiguration notifications.
        state.output_managers.push(manager.downgrade());
    }

    fn can_view(client: Client, global_data: &OutputManagementGlobalData) -> bool {
        (global_data.filter)(&client)
    }
}

impl Dispatch<ZwlrOutputManagerV1, OutputManagerData> for State {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &ZwlrOutputManagerV1,
        request: zwlr_output_manager_v1::Request,
        data: &OutputManagerData,
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let zwlr_output_manager_v1::Request::CreateConfiguration { id, serial } = request {
            let current_serial = CONFIG_SERIAL.load(Ordering::Relaxed);
            // Build head mapping from manager data.
            let head_map: Vec<(Weak<ZwlrOutputHeadV1>, Output)> = data
                .heads
                .lock()
                .expect("mutex poisoned")
                .iter()
                .map(|(output, head)| (head.downgrade(), output.clone()))
                .collect();
            let mode_map: Arc<Vec<(Weak<ZwlrOutputModeV1>, Output, Mode)>> = Arc::new(
                data.modes
                    .lock()
                    .expect("mutex poisoned")
                    .iter()
                    .map(|(output, mode, mode_obj)| (mode_obj.downgrade(), output.clone(), *mode))
                    .collect(),
            );

            data_init.init(
                id,
                OutputConfigurationData {
                    serial,
                    current_serial,
                    head_configs: Arc::new(Mutex::new(Vec::new())),
                    disabled_heads: Mutex::new(Vec::new()),
                    head_map,
                    mode_map,
                    used: Mutex::new(false),
                },
            );
        }
    }

    fn destroyed(_state: &mut Self, _client: ClientId, _resource: &ZwlrOutputManagerV1, _data: &OutputManagerData) {}
}

// ---------------------------------------------------------------------------
// Head (read-only output device)
// ---------------------------------------------------------------------------

/// Per-head data — links back to the output.
pub struct OutputHeadData {
    pub output: Output,
}

impl Dispatch<ZwlrOutputHeadV1, OutputHeadData> for State {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &ZwlrOutputHeadV1,
        request: zwlr_output_head_v1::Request,
        _data: &OutputHeadData,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        // Only request is `release` (v3 destructor).
        let _ = request;
    }

    fn destroyed(_state: &mut Self, _client: ClientId, _resource: &ZwlrOutputHeadV1, _data: &OutputHeadData) {}
}

// ---------------------------------------------------------------------------
// Mode (read-only)
// ---------------------------------------------------------------------------

/// Per-mode data.
pub struct OutputModeData {
    pub output: Output,
    pub mode: Mode,
}

impl Dispatch<ZwlrOutputModeV1, OutputModeData> for State {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &ZwlrOutputModeV1,
        request: <ZwlrOutputModeV1 as Resource>::Request,
        _data: &OutputModeData,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        // Only request is `release` (v3 destructor).
        let _ = request;
    }

    fn destroyed(_state: &mut Self, _client: ClientId, _resource: &ZwlrOutputModeV1, _data: &OutputModeData) {}
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Pending configuration — accumulates enable/disable requests.
pub struct OutputConfigurationData {
    /// Serial provided by the client.
    serial: u32,
    /// Serial that was current when the configuration was created.
    current_serial: u32,
    /// Per-head configuration (shared with child config-head objects via `Arc`).
    head_configs: Arc<Mutex<Vec<HeadConfig>>>,
    /// Heads to be disabled.
    disabled_heads: Mutex<Vec<Output>>,
    /// Mapping from head protocol objects to outputs.
    head_map: Vec<(Weak<ZwlrOutputHeadV1>, Output)>,
    /// Mapping from mode protocol objects to (output, mode).
    mode_map: Arc<Vec<(Weak<ZwlrOutputModeV1>, Output, Mode)>>,
    /// Whether apply/test has been called (protocol error to send twice).
    used: Mutex<bool>,
}

impl OutputConfigurationData {
    /// Resolve a head object to its output.
    fn resolve_head(&self, head: &ZwlrOutputHeadV1) -> Option<Output> {
        let weak = head.downgrade();
        self.head_map.iter().find(|(w, _)| w == &weak).map(|(_, o)| o.clone())
    }
}

/// Desired properties for an enabled head.
struct HeadConfig {
    output: Output,
    mode: Option<Mode>,
    custom_mode: Option<Mode>,
    position: Option<(i32, i32)>,
    transform: Option<Transform>,
    scale: Option<f64>,
}

impl Dispatch<ZwlrOutputConfigurationV1, OutputConfigurationData> for State {
    fn request(
        state: &mut Self,
        _client: &Client,
        resource: &ZwlrOutputConfigurationV1,
        request: zwlr_output_configuration_v1::Request,
        data: &OutputConfigurationData,
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            zwlr_output_configuration_v1::Request::EnableHead { id, head } => {
                let Some(output) = data.resolve_head(&head) else {
                    return;
                };
                let config = HeadConfig {
                    output: output.clone(),
                    mode: None,
                    custom_mode: None,
                    position: None,
                    transform: None,
                    scale: None,
                };
                let configs = Arc::clone(&data.head_configs);
                let mode_map = Arc::clone(&data.mode_map);
                let mut lock = configs.lock().expect("mutex poisoned");
                lock.push(config);
                let idx = lock.len() - 1;
                drop(lock);

                data_init.init(id, ConfigHeadData { index: idx, configs, mode_map });
            }
            zwlr_output_configuration_v1::Request::DisableHead { head } => {
                if let Some(output) = data.resolve_head(&head) {
                    data.disabled_heads.lock().expect("mutex poisoned").push(output);
                }
            }
            zwlr_output_configuration_v1::Request::Apply => {
                handle_apply_or_test(state, resource, data, true);
            }
            zwlr_output_configuration_v1::Request::Test => {
                handle_apply_or_test(state, resource, data, false);
            }
            zwlr_output_configuration_v1::Request::Destroy => {}
            _ => {
                tracing::debug!("unhandled output configuration request");
            }
        }
    }

    fn destroyed(
        _state: &mut Self,
        _client: ClientId,
        _resource: &ZwlrOutputConfigurationV1,
        _data: &OutputConfigurationData,
    ) {
    }
}

// ---------------------------------------------------------------------------
// Configuration head (per-head property overrides)
// ---------------------------------------------------------------------------

/// Per-head configuration data.
///
/// Shares the parent configuration's `head_configs` and `mode_map` via `Arc`
/// so property-setting requests can update the corresponding entry without
/// any `unsafe` code.
pub struct ConfigHeadData {
    index: usize,
    configs: Arc<Mutex<Vec<HeadConfig>>>,
    mode_map: Arc<Vec<(Weak<ZwlrOutputModeV1>, Output, Mode)>>,
}

impl Dispatch<ZwlrOutputConfigurationHeadV1, ConfigHeadData> for State {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &ZwlrOutputConfigurationHeadV1,
        request: zwlr_output_configuration_head_v1::Request,
        data: &ConfigHeadData,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            zwlr_output_configuration_head_v1::Request::SetMode { mode } => {
                let weak = mode.downgrade();
                if let Some((_, resolved_mode)) =
                    data.mode_map.iter().find(|(w, _, _)| w == &weak).map(|(_, o, m)| (o, m))
                {
                    let mut configs = data.configs.lock().expect("mutex poisoned");
                    if let Some(cfg) = configs.get_mut(data.index) {
                        cfg.mode = Some(*resolved_mode);
                    }
                }
            }
            zwlr_output_configuration_head_v1::Request::SetCustomMode { width, height, refresh } => {
                let mut configs = data.configs.lock().expect("mutex poisoned");
                if let Some(cfg) = configs.get_mut(data.index) {
                    cfg.custom_mode = Some(Mode { size: (width, height).into(), refresh });
                }
            }
            zwlr_output_configuration_head_v1::Request::SetPosition { x, y } => {
                let mut configs = data.configs.lock().expect("mutex poisoned");
                if let Some(cfg) = configs.get_mut(data.index) {
                    cfg.position = Some((x, y));
                }
            }
            zwlr_output_configuration_head_v1::Request::SetTransform { transform } => {
                let transform = match transform {
                    WEnum::Value(v) => v,
                    WEnum::Unknown(_) => return,
                };
                let mut configs = data.configs.lock().expect("mutex poisoned");
                if let Some(cfg) = configs.get_mut(data.index) {
                    cfg.transform = Some(transform);
                }
            }
            zwlr_output_configuration_head_v1::Request::SetScale { scale } => {
                let mut configs = data.configs.lock().expect("mutex poisoned");
                if let Some(cfg) = configs.get_mut(data.index) {
                    cfg.scale = Some(scale);
                }
            }
            _ => {
                tracing::debug!("ignoring unhandled output config-head request");
            }
        }
    }

    fn destroyed(
        _state: &mut Self,
        _client: ClientId,
        _resource: &ZwlrOutputConfigurationHeadV1,
        _data: &ConfigHeadData,
    ) {
    }
}

// ---------------------------------------------------------------------------
// Apply / Test logic
// ---------------------------------------------------------------------------

fn handle_apply_or_test(
    state: &mut State,
    config_resource: &ZwlrOutputConfigurationV1,
    data: &OutputConfigurationData,
    apply: bool,
) {
    // Mark as used (protocol error to apply/test twice).
    {
        let mut used = data.used.lock().expect("mutex poisoned");
        if *used {
            config_resource.post_error(
                zwlr_output_configuration_v1::Error::AlreadyUsed,
                "apply or test has already been called on this configuration",
            );
            return;
        }
        *used = true;
    }

    // Check serial freshness — if outdated, cancel.
    if data.serial != data.current_serial {
        config_resource.cancelled();
        return;
    }

    let head_configs = data.head_configs.lock().expect("mutex poisoned");
    let disabled = data.disabled_heads.lock().expect("mutex poisoned");

    if apply {
        // For DRM backend: activate outputs that need a CRTC before mapping.
        // Do this before configuring modes/positions so the DRM compositor exists.
        if let Some(ref mut backend) = state.drm_backend {
            for cfg in head_configs.iter() {
                if let Some(conn) = backend.connector_for_output(&cfg.output)
                    && let Err(err) = backend.activate_output(conn)
                {
                    tracing::warn!(
                        output = cfg.output.name(),
                        %err,
                        "output management: failed to activate DRM output",
                    );
                }
            }
        }

        // Apply enabled-head configurations.
        for cfg in head_configs.iter() {
            let mode = cfg.custom_mode.or(cfg.mode).unwrap_or_else(|| {
                cfg.output
                    .current_mode()
                    .unwrap_or(Mode { size: (1280, 720).into(), refresh: crate::state::DEFAULT_REFRESH_MHTZ })
            });

            let transform = cfg.transform.map(SmithayTransform::from);
            let scale = cfg.scale.filter(|&s| s > 0.0).map(smithay::output::Scale::Fractional);
            let position = cfg.position.map(Into::into);

            cfg.output.change_current_state(Some(mode), transform, scale, position);

            // (Re-)map the output in the space.  If the client supplied a
            // position, use that; otherwise use the output's current
            // protocol-level position (preserves the last known layout).
            let loc = cfg.position.unwrap_or_else(|| {
                let p = cfg.output.current_location();
                (p.x, p.y)
            });
            state.space.map_output(&cfg.output, loc);

            tracing::info!(
                output = cfg.output.name(),
                width = mode.size.w,
                height = mode.size.h,
                refresh = mode.refresh,
                position = ?cfg.position,
                scale = ?cfg.scale,
                "output management: applied configuration",
            );
        }

        // Handle disabled heads — unmap from space and release DRM hardware.
        for output in disabled.iter() {
            state.space.unmap_output(output);

            // For DRM backend: deactivate hardware (release CRTC).
            if let Some(ref mut backend) = state.drm_backend
                && let Some(conn) = backend.connector_for_output(output)
            {
                backend.deactivate_output(conn);
            }

            tracing::info!(output = output.name(), "output management: disabled output");
        }

        // Bump serial so stale configurations get cancelled.
        let _new_serial = next_serial();

        // Signal the event loop to rebuild the damage tracker and reconfigure
        // windows for the new output dimensions / scale.
        state.output_config_changed = true;
    }

    config_resource.succeeded();
}

// ---------------------------------------------------------------------------
// Sending head/mode events
// ---------------------------------------------------------------------------

/// Send all head events for the current outputs to a manager instance.
fn send_all_heads(state: &State, manager: &ZwlrOutputManagerV1) {
    for output in &state.outputs {
        send_head_for_output(state, manager, output);
    }
}

/// Send head and mode events for a single output.
fn send_head_for_output(state: &State, manager: &ZwlrOutputManagerV1, output: &Output) {
    let Some(client) = manager.client() else {
        return;
    };
    let dh = &state.display_handle;
    let Some(data) = manager.data::<OutputManagerData>() else {
        tracing::warn!("output management: manager resource missing OutputManagerData");
        return;
    };

    // Create head object.
    let Ok(head) = client.create_resource::<ZwlrOutputHeadV1, _, State>(
        dh,
        manager.version(),
        OutputHeadData { output: output.clone() },
    ) else {
        tracing::warn!(output = output.name(), "output management: failed to create head resource (client gone?)");
        return;
    };

    // Send head event to manager.
    manager.head(&head);

    // Send head properties.
    head.name(output.name());
    head.description(output.description());

    let phys = output.physical_properties();
    if phys.size.w > 0 || phys.size.h > 0 {
        head.physical_size(phys.size.w, phys.size.h);
    }

    // Send make/model if protocol version supports it (v2+).
    if head.version() >= 2 {
        head.make(phys.make.clone());
        head.model(phys.model.clone());
    }

    // Send modes.
    let modes = output.modes();
    let current_mode = output.current_mode();
    let preferred_mode = output.preferred_mode();

    if modes.is_empty() {
        // If no explicit modes were added, advertise the current mode as the
        // only (and preferred) mode.  Virtual outputs often have only one mode.
        if let Some(mode) = current_mode {
            send_mode(dh, &client, manager, &head, output, mode, true, true, data);
        }
    } else {
        for mode in &modes {
            let is_current = current_mode.is_some_and(|cm| cm == *mode);
            let is_preferred = preferred_mode.is_some_and(|pm| pm == *mode);
            send_mode(dh, &client, manager, &head, output, *mode, is_current, is_preferred, data);
        }
    }

    // Enabled / position / transform / scale.
    let is_mapped = state.space.output_geometry(output).is_some();
    head.enabled(i32::from(is_mapped));

    if is_mapped {
        let loc = output.current_location();
        head.position(loc.x, loc.y);

        let transform = output.current_transform();
        head.transform(transform.into());

        head.scale(output.current_scale().fractional_scale());
    }

    data.heads.lock().expect("mutex poisoned").push((output.clone(), head));
}

/// Create and send a mode object with its properties.
#[allow(clippy::too_many_arguments)]
fn send_mode(
    dh: &DisplayHandle,
    client: &Client,
    manager: &ZwlrOutputManagerV1,
    head: &ZwlrOutputHeadV1,
    output: &Output,
    mode: Mode,
    is_current: bool,
    is_preferred: bool,
    data: &OutputManagerData,
) {
    let Ok(mode_obj) = client.create_resource::<ZwlrOutputModeV1, _, State>(
        dh,
        manager.version(),
        OutputModeData { output: output.clone(), mode },
    ) else {
        tracing::warn!("output management: failed to create mode resource (client gone?)");
        return;
    };

    head.mode(&mode_obj);
    mode_obj.size(mode.size.w, mode.size.h);
    mode_obj.refresh(mode.refresh);

    if is_preferred {
        mode_obj.preferred();
    }
    if is_current {
        head.current_mode(&mode_obj);
    }

    data.modes.lock().expect("mutex poisoned").push((output.clone(), mode, mode_obj));
}

// ---------------------------------------------------------------------------
// Global registration
// ---------------------------------------------------------------------------

/// Register the `zwlr_output_manager_v1` global.
pub fn init_output_management(
    dh: &DisplayHandle,
    filter: impl Fn(&Client) -> bool + Send + Sync + 'static,
) -> GlobalId {
    dh.create_global::<State, ZwlrOutputManagerV1, _>(4, OutputManagementGlobalData { filter: Box::new(filter) })
}

/// Notify all bound output management clients that the output configuration changed.
///
/// Sends `finished` events on all old head/mode objects to invalidate them,
/// then re-sends the full output state with a bumped serial so that clients
/// (e.g. `kanshi`) watching for changes can create a fresh configuration.
pub fn notify_output_config_changed(state: &mut State) {
    // GC dead manager references.
    state.output_managers.retain(|weak| weak.upgrade().is_ok());

    let serial = CONFIG_SERIAL.load(Ordering::Relaxed);

    for weak in &state.output_managers {
        let Ok(manager) = weak.upgrade() else {
            continue;
        };
        let Some(data) = manager.data::<OutputManagerData>() else {
            continue;
        };

        // Send `finished` on old mode objects first (modes are children of heads).
        for (_, _, mode_obj) in data.modes.lock().expect("mutex poisoned").drain(..) {
            mode_obj.finished();
        }
        // Send `finished` on old head objects.
        for (_, head) in data.heads.lock().expect("mutex poisoned").drain(..) {
            head.finished();
        }

        // Re-send the full head/mode tree for all current outputs.
        send_all_heads(state, &manager);
        manager.done(serial);
    }

    tracing::debug!(serial, managers = state.output_managers.len(), "notified output management clients");
}
