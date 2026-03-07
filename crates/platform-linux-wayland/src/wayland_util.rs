use platynui_core::platform::{PlatformError, PlatformErrorKind};
use std::env;
use std::sync::Mutex;
use std::sync::OnceLock;
use wayland_client::protocol::wl_output;
use wayland_client::protocol::wl_registry;
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, delegate_noop};

/// Discovered Wayland globals relevant for UI automation.
#[derive(Debug, Default, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct ProtocolCapabilities {
    /// `wlr-foreign-toplevel-management-unstable-v1` (window lifecycle)
    pub wlr_foreign_toplevel: bool,
    /// `ext-foreign-toplevel-list-v1` (read-only toplevel list)
    pub ext_foreign_toplevel_list: bool,
    /// `wlr-layer-shell-unstable-v1` (overlay surfaces)
    pub wlr_layer_shell: bool,
    /// `ext-layer-shell-v1` (standardized overlay surfaces)
    pub ext_layer_shell: bool,
    /// `zwlr_virtual_pointer_manager_v1` (wlr virtual pointer)
    pub wlr_virtual_pointer: bool,
    /// `zwp_virtual_keyboard_manager_v1` (wlr virtual keyboard)
    pub wlr_virtual_keyboard: bool,
    /// `ext-image-copy-capture-v1` (standard screenshot)
    pub ext_image_copy_capture: bool,
    /// `wlr-screencopy-unstable-v1` (legacy screenshot)
    pub wlr_screencopy: bool,
    /// `xdg_output_manager_v1` (logical monitor info)
    pub xdg_output_manager: bool,
    /// `wl_output` count
    pub output_count: u32,
}

/// State object used during `wl_registry` global enumeration.
struct RegistryState {
    capabilities: ProtocolCapabilities,
}

impl Dispatch<wl_registry::WlRegistry, ()> for RegistryState {
    fn event(
        state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { interface, .. } = event {
            match interface.as_str() {
                "zwlr_foreign_toplevel_manager_v1" => state.capabilities.wlr_foreign_toplevel = true,
                "ext_foreign_toplevel_list_v1" => state.capabilities.ext_foreign_toplevel_list = true,
                "zwlr_layer_shell_v1" => state.capabilities.wlr_layer_shell = true,
                "ext_layer_shell_v1" => state.capabilities.ext_layer_shell = true,
                "zwlr_virtual_pointer_manager_v1" => state.capabilities.wlr_virtual_pointer = true,
                "zwp_virtual_keyboard_manager_v1" => state.capabilities.wlr_virtual_keyboard = true,
                "ext_image_copy_capture_manager_v1" => state.capabilities.ext_image_copy_capture = true,
                "zwlr_screencopy_manager_v1" => state.capabilities.wlr_screencopy = true,
                "zxdg_output_manager_v1" => state.capabilities.xdg_output_manager = true,
                "wl_output" => state.capabilities.output_count += 1,
                _ => {}
            }
        }
    }
}

delegate_noop!(RegistryState: ignore wl_output::WlOutput);

/// Shared Wayland connection state.
pub struct WaylandHandle {
    #[allow(dead_code)]
    pub connection: Connection,
    pub capabilities: ProtocolCapabilities,
}

static WAYLAND: OnceLock<Mutex<Option<WaylandHandle>>> = OnceLock::new();

/// Acquire a guard to the shared Wayland connection. The connection is
/// established lazily on the first call and reused afterwards.
pub fn connection() -> Result<WaylandGuard, PlatformError> {
    let wayland_display = env::var("WAYLAND_DISPLAY")
        .map_err(|_| PlatformError::new(PlatformErrorKind::UnsupportedPlatform, "WAYLAND_DISPLAY not set"))?;

    let cell = WAYLAND.get_or_init(|| {
        tracing::debug!(display = %wayland_display, "establishing Wayland connection");
        match connect_and_probe(&wayland_display) {
            Ok(handle) => {
                tracing::info!(display = %wayland_display, "Wayland connection established");
                Mutex::new(Some(handle))
            }
            Err(err) => {
                tracing::error!(display = %wayland_display, %err, "Wayland connection failed");
                Mutex::new(None)
            }
        }
    });

    let guard = cell
        .lock()
        .map_err(|_| PlatformError::new(PlatformErrorKind::InitializationFailed, "wayland mutex poisoned"))?;

    if guard.is_none() {
        return Err(PlatformError::new(
            PlatformErrorKind::InitializationFailed,
            "Wayland connection not available (shutdown or failed to connect)",
        ));
    }

    Ok(WaylandGuard(guard))
}

/// RAII guard that dereferences to [`WaylandHandle`]. Returned by [`connection()`].
pub struct WaylandGuard(std::sync::MutexGuard<'static, Option<WaylandHandle>>);

impl std::ops::Deref for WaylandGuard {
    type Target = WaylandHandle;
    fn deref(&self) -> &WaylandHandle {
        self.0.as_ref().expect("WaylandGuard created with None")
    }
}

impl std::ops::DerefMut for WaylandGuard {
    fn deref_mut(&mut self) -> &mut WaylandHandle {
        self.0.as_mut().expect("WaylandGuard created with None")
    }
}

/// Drops the shared Wayland connection. Subsequent calls to [`connection()`]
/// will return an error.
pub fn shutdown_connection() {
    if let Some(cell) = WAYLAND.get()
        && let Ok(mut guard) = cell.lock()
        && let Some(handle) = guard.take()
    {
        tracing::debug!("Wayland connection closed");
        drop(handle);
    }
}

/// Connect to the Wayland display and probe `wl_registry` for available
/// globals to determine which protocols the compositor supports.
fn connect_and_probe(_display: &str) -> Result<WaylandHandle, String> {
    let connection = Connection::connect_to_env().map_err(|e| format!("wayland connect: {e}"))?;

    let display_proxy = connection.display();
    let mut event_queue: EventQueue<RegistryState> = connection.new_event_queue();
    let qh = event_queue.handle();

    let _registry = display_proxy.get_registry(&qh, ());
    let mut state = RegistryState { capabilities: ProtocolCapabilities::default() };

    // Perform a blocking roundtrip to collect all globals.
    event_queue.blocking_dispatch(&mut state).map_err(|e| format!("registry roundtrip: {e}"))?;

    Ok(WaylandHandle { connection, capabilities: state.capabilities })
}
