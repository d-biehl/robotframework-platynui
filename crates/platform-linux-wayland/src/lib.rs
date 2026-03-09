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
pub mod desktop;

// Protocols used in later phases — suppress unused-crate-dependencies for now.
#[cfg(test)]
use rstest as _;
#[cfg(target_os = "linux")]
use wayland_protocols_wlr as _;

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
            let (conn, outputs) = connection::connect_and_enumerate()?;
            let compositor = crate::capabilities::detect_compositor(&conn);
            info!(?compositor, output_count = outputs.len(), "Wayland platform initialized");

            connection::set_global(conn, compositor, outputs);

            Ok(())
        }

        fn shutdown(&self) {
            connection::clear_global();
        }
    }
}

// Stub modules — full implementations come in Phase 4b–4e.

#[cfg(target_os = "linux")]
pub mod highlight {
    use platynui_core::platform::{HighlightProvider, HighlightRequest, PlatformError};

    pub struct WaylandHighlightProvider;

    impl HighlightProvider for WaylandHighlightProvider {
        fn highlight(&self, _request: &HighlightRequest) -> Result<(), PlatformError> {
            tracing::warn!("Wayland highlight not yet implemented");
            Ok(())
        }

        fn clear(&self) -> Result<(), PlatformError> {
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
pub mod keyboard {
    use platynui_core::platform::{KeyCode, KeyboardDevice, KeyboardError, KeyboardEvent};

    pub struct WaylandKeyboardDevice;

    impl KeyboardDevice for WaylandKeyboardDevice {
        fn key_to_code(&self, name: &str) -> Result<KeyCode, KeyboardError> {
            Err(KeyboardError::UnsupportedKey(name.to_string()))
        }

        fn send_key_event(&self, _event: KeyboardEvent) -> Result<(), KeyboardError> {
            Err(KeyboardError::NotReady)
        }
    }
}

#[cfg(target_os = "linux")]
pub mod pointer {
    use platynui_core::platform::{PlatformError, PlatformErrorKind, PointerButton, PointerDevice, ScrollDelta};
    use platynui_core::types::Point;

    pub struct WaylandPointerDevice;

    impl PointerDevice for WaylandPointerDevice {
        fn position(&self) -> Result<Point, PlatformError> {
            Err(PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "Wayland pointer not yet implemented"))
        }

        fn move_to(&self, _point: Point) -> Result<(), PlatformError> {
            Err(PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "Wayland pointer not yet implemented"))
        }

        fn press(&self, _button: PointerButton) -> Result<(), PlatformError> {
            Err(PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "Wayland pointer not yet implemented"))
        }

        fn release(&self, _button: PointerButton) -> Result<(), PlatformError> {
            Err(PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "Wayland pointer not yet implemented"))
        }

        fn scroll(&self, _delta: ScrollDelta) -> Result<(), PlatformError> {
            Err(PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "Wayland pointer not yet implemented"))
        }
    }
}

#[cfg(target_os = "linux")]
pub mod screenshot {
    use platynui_core::platform::{
        PlatformError, PlatformErrorKind, Screenshot, ScreenshotProvider, ScreenshotRequest,
    };

    pub struct WaylandScreenshot;

    impl ScreenshotProvider for WaylandScreenshot {
        fn capture(&self, _request: &ScreenshotRequest) -> Result<Screenshot, PlatformError> {
            Err(PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "Wayland screenshot not yet implemented"))
        }
    }
}

#[cfg(target_os = "linux")]
pub mod window_manager {
    use platynui_core::platform::{PlatformError, PlatformErrorKind, WindowId, WindowManager};
    use platynui_core::types::{Point, Rect, Size};
    use platynui_core::ui::UiNode;

    pub struct WaylandWindowManager;

    impl WindowManager for WaylandWindowManager {
        fn name(&self) -> &'static str {
            "Wayland"
        }

        fn resolve_window(&self, _node: &dyn UiNode) -> Result<WindowId, PlatformError> {
            Err(PlatformError::new(
                PlatformErrorKind::CapabilityUnavailable,
                "Wayland window manager not yet implemented",
            ))
        }

        fn bounds(&self, _id: WindowId) -> Result<Rect, PlatformError> {
            Err(PlatformError::new(
                PlatformErrorKind::CapabilityUnavailable,
                "Wayland window manager not yet implemented",
            ))
        }

        fn is_active(&self, _id: WindowId) -> Result<bool, PlatformError> {
            Err(PlatformError::new(
                PlatformErrorKind::CapabilityUnavailable,
                "Wayland window manager not yet implemented",
            ))
        }

        fn activate(&self, _id: WindowId) -> Result<(), PlatformError> {
            Err(PlatformError::new(
                PlatformErrorKind::CapabilityUnavailable,
                "Wayland window manager not yet implemented",
            ))
        }

        fn close(&self, _id: WindowId) -> Result<(), PlatformError> {
            Err(PlatformError::new(
                PlatformErrorKind::CapabilityUnavailable,
                "Wayland window manager not yet implemented",
            ))
        }

        fn minimize(&self, _id: WindowId) -> Result<(), PlatformError> {
            Err(PlatformError::new(
                PlatformErrorKind::CapabilityUnavailable,
                "Wayland window manager not yet implemented",
            ))
        }

        fn maximize(&self, _id: WindowId) -> Result<(), PlatformError> {
            Err(PlatformError::new(
                PlatformErrorKind::CapabilityUnavailable,
                "Wayland window manager not yet implemented",
            ))
        }

        fn restore(&self, _id: WindowId) -> Result<(), PlatformError> {
            Err(PlatformError::new(
                PlatformErrorKind::CapabilityUnavailable,
                "Wayland window manager not yet implemented",
            ))
        }

        fn move_to(&self, _id: WindowId, _position: Point) -> Result<(), PlatformError> {
            Err(PlatformError::new(
                PlatformErrorKind::CapabilityUnavailable,
                "Wayland window manager not yet implemented",
            ))
        }

        fn resize(&self, _id: WindowId, _size: Size) -> Result<(), PlatformError> {
            Err(PlatformError::new(
                PlatformErrorKind::CapabilityUnavailable,
                "Wayland window manager not yet implemented",
            ))
        }
    }
}

// Non-Linux targets keep a tiny marker to allow cross-platform builds.
#[cfg(not(target_os = "linux"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WaylandPlatformStub;
