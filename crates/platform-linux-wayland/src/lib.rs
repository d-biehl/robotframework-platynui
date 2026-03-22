//! Wayland platform backend for `PlatynUI` on Linux.
//!
//! This crate provides the Wayland-specific implementations of all platform
//! traits (`PointerDevice`, `KeyboardDevice`, `ScreenshotProvider`,
//! `HighlightProvider`, `DesktopInfoProvider`, `WindowManager`).
//!
//! It does **not** register itself via `inventory`. Instead, the
//! `platform-linux` mediator crate imports these types and delegates to them
//! when the detected session type is Wayland.
//!
//! # Compositor Detection
//!
//! At `initialize()` time, the crate identifies the running compositor via
//! `SO_PEERCRED` on the Wayland socket and selects appropriate backends
//! (EIS vs Portal for input, Layer-Shell vs D-Bus for highlights, etc.).

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

#[cfg(target_os = "linux")]
pub mod init {
    use platynui_core::platform::{PlatformError, PlatformModule};
    use tracing::info;

    use crate::connection;

    pub struct WaylandModule;

    impl PlatformModule for WaylandModule {
        fn name(&self) -> &'static str {
            "Linux Wayland Platform"
        }

        fn initialize(&self) -> Result<(), PlatformError> {
            let (conn, compositor, mut outputs, session) = connection::connect_and_enumerate()?;
            crate::desktop::display_config::enrich_outputs(compositor, &mut outputs);
            info!(?compositor, output_count = outputs.len(), "Wayland platform initialized");

            crate::desktop::set_outputs(outputs);
            connection::set_global_and_start(conn, compositor, session);

            // Initialize input backends based on detected compositor.
            crate::input::initialize(compositor);

            Ok(())
        }

        fn shutdown(&self) {
            crate::input::shutdown();
            connection::clear_global();
            crate::desktop::clear_outputs();
        }
    }
}

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
pub mod screenshot {
    use platynui_core::platform::{PlatformError, Screenshot, ScreenshotProvider, ScreenshotRequest};

    pub struct WaylandScreenshot;

    impl ScreenshotProvider for WaylandScreenshot {
        fn capture(&self, _request: &ScreenshotRequest) -> Result<Screenshot, PlatformError> {
            Err(PlatformError::CapabilityUnavailable {
                capability: "Wayland screenshot provider",
                details: Some("not yet implemented".into()),
            })
        }
    }
}

// Non-Linux targets keep a tiny marker to allow cross-platform builds.
#[cfg(not(target_os = "linux"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WaylandPlatformStub;
