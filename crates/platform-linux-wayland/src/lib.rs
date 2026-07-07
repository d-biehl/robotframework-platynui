//! Wayland platform backend for `PlatynUI` on Linux.
//!
//! This crate provides the Wayland-specific implementations of all platform
//! traits (`PointerDevice`, `KeyboardDevice`, `ScreenshotProvider`,
//! `HighlightProvider`, `DesktopInfoProvider`, `WindowManager`).
//!
//! It does **not** register itself via `inventory`. Instead it exposes
//! [`create_wayland_bundle`], which the `platform-linux` mediator crate calls
//! when the detected session type is Wayland.
//!
//! # Compositor Detection
//!
//! When [`create_wayland_bundle`] runs, the crate identifies the running
//! compositor via `SO_PEERCRED` on the Wayland socket and selects appropriate
//! backends (EIS vs Portal for input, Layer-Shell vs D-Bus for highlights, etc.).

#[cfg(target_os = "linux")]
pub mod capabilities;

#[cfg(target_os = "linux")]
pub mod connection;

#[cfg(target_os = "linux")]
pub mod control_ipc;

#[cfg(target_os = "linux")]
pub mod desktop;

#[cfg(target_os = "linux")]
pub mod highlight;

#[cfg(target_os = "linux")]
pub mod input;

#[cfg(target_os = "linux")]
pub mod window_manager;

// Protocols used in later phases — suppress unused-crate-dependencies for now.
#[cfg(test)]
use rstest as _;

// Stub modules — full implementations come in Phase 4c.

/// Keyboard device backed by the active input backend (EIS, Portal, or virtual-input).
#[cfg(target_os = "linux")]
pub mod keyboard {
    pub use crate::input::WaylandKeyboardDevice;
}

/// Pointer device backed by the active input backend (EIS, Portal, or virtual-input).
#[cfg(target_os = "linux")]
pub mod pointer {
    pub use crate::input::WaylandPointerDevice;
}

#[cfg(target_os = "linux")]
pub mod screenshot;

/// Build a per-runtime Wayland [`PlatformBundle`](platynui_core::platform::PlatformBundle)
/// for the session described by `config`.
///
/// Connects to the compositor (`$WAYLAND_DISPLAY`), detects the compositor type
/// via `SO_PEERCRED`, enumerates and enriches outputs, starts the background
/// output-monitoring event loop, and selects an input backend — the same setup
/// the former `WaylandModule::initialize` performed — then returns the six
/// Wayland devices.
///
/// # Process-global backing (this phase)
///
/// Unlike the X11 backend, the Wayland devices do **not** own a per-instance
/// connection. They read this crate's process-global session state: the
/// compositor and event-loop handle installed by
/// [`connection::set_global_and_start`], the active input backend, and the
/// enumerated outputs. Building a bundle for a new runtime re-runs this
/// initialization; `set_global_and_start` stops the previous session's event
/// loop before installing the new one, which is sufficient for the
/// sequential-runtime model this phase targets. Internalizing those globals
/// into per-instance state is a deliberate non-goal for now.
///
/// `config` carries no Wayland-specific settings this phase, so it is unused;
/// the parameter is kept for signature symmetry with `create_x11_bundle`.
///
/// # Errors
///
/// Returns [`PlatformError`](platynui_core::platform::PlatformError) if the
/// Wayland display connection, registry initialization, or an output roundtrip
/// fails.
#[cfg(target_os = "linux")]
pub fn create_wayland_bundle(
    _config: &platynui_core::config::RuntimeConfig,
) -> Result<platynui_core::platform::PlatformBundle, platynui_core::platform::PlatformError> {
    use std::sync::Arc;

    use platynui_core::platform::{
        DesktopInfoProvider, HighlightProvider, KeyboardDevice, PlatformBundle, PointerDevice, ScreenshotProvider,
        WindowManager,
    };
    use tracing::info;

    // TODO(wayland-internalization): per-instance teardown on bundle drop.
    // Wayland stays process-global-backed this phase, so the bundle does not
    // arrange `connection::clear_global` / `input::shutdown` / `desktop::clear_outputs`
    // on drop; `set_global_and_start` stopping the previous session on re-init
    // is sufficient for sequential runtimes.

    // Same initialization the former `WaylandModule::initialize` performed:
    // connect, enumerate + enrich outputs, install the global session, and
    // select the input backend for the detected compositor.
    let (conn, compositor, mut outputs, session) = crate::connection::connect_and_enumerate()?;
    crate::desktop::display_config::enrich_outputs(compositor, &mut outputs);
    info!(?compositor, output_count = outputs.len(), "Wayland platform bundle created");

    crate::desktop::set_outputs(outputs);
    crate::connection::set_global_and_start(conn, compositor, session);
    crate::input::initialize(compositor);

    // The six devices are unit structs that read the process-global session
    // state installed above.
    Ok(PlatformBundle {
        pointer: Arc::new(crate::pointer::WaylandPointerDevice) as Arc<dyn PointerDevice>,
        keyboard: Arc::new(crate::keyboard::WaylandKeyboardDevice) as Arc<dyn KeyboardDevice>,
        screenshot: Arc::new(crate::screenshot::WaylandScreenshot) as Arc<dyn ScreenshotProvider>,
        highlight: Arc::new(crate::highlight::WaylandHighlightProvider) as Arc<dyn HighlightProvider>,
        window_manager: Arc::new(crate::window_manager::WaylandWindowManager) as Arc<dyn WindowManager>,
        desktop_info: Arc::new(crate::desktop::WaylandDesktopInfo) as Arc<dyn DesktopInfoProvider>,
    })
}

// Non-Linux targets keep a tiny marker to allow cross-platform builds.
#[cfg(not(target_os = "linux"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WaylandPlatformStub;
