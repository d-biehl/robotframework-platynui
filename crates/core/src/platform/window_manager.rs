//! Window manager abstraction for native window handle operations.
//!
//! The [`WindowManager`] trait decouples accessibility providers from
//! platform-specific windowing APIs.  Each platform crate implements the trait
//! and registers it via [`register_window_manager!`]; providers
//! discover the implementation through [`window_managers()`] at
//! runtime.

use crate::platform::PlatformError;
use crate::types::{Point, Rect, Size};
use crate::ui::UiNode;
use std::fmt;

/// Opaque native window handle.
///
/// Wraps the platform-native identifier (HWND on Windows, XID on X11, Wayland
/// surface ID, etc.) as a `u64`.  Consumers must treat this as opaque; only
/// the originating [`WindowManager`] understands how to interpret it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct WindowId(pub(crate) u64);

impl WindowId {
    /// Create a new `WindowId` from a raw platform handle.
    pub fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Return the underlying raw value.
    pub fn raw(self) -> u64 {
        self.0
    }
}

impl fmt::Display for WindowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WindowId(0x{:x})", self.0)
    }
}

/// Platform-native window management operations.
///
/// Implementations live in platform crates (e.g. `platform-linux-x11`,
/// `platform-windows`) and are registered via inventory.  Accessibility
/// providers call [`resolve_window`](WindowManager::resolve_window)
/// to obtain a [`WindowId`] and then invoke the desired operation.
pub trait WindowManager: Send + Sync {
    /// Human-readable name for diagnostics (e.g. `"X11 EWMH"`, `"Win32"`).
    fn name(&self) -> &'static str;

    /// Resolve the native window handle from an accessibility node.
    ///
    /// Each implementation decides which node attributes it inspects (e.g.
    /// `native:NativeWindowHandle` on Windows, PID + geometry on X11).
    fn resolve_window(&self, node: &dyn UiNode) -> Result<WindowId, PlatformError>;

    /// Actual screen bounds of the window as reported by the window manager.
    ///
    /// May differ from accessibility-reported bounds (e.g. GTK4 on X11).
    fn bounds(&self, id: WindowId) -> Result<Rect, PlatformError>;

    /// Whether this window is the currently active (foreground) window.
    fn is_active(&self, id: WindowId) -> Result<bool, PlatformError>;

    /// Bring the window to the foreground.
    fn activate(&self, id: WindowId) -> Result<(), PlatformError>;

    /// Request the window manager to close this window.
    fn close(&self, id: WindowId) -> Result<(), PlatformError>;

    /// Minimise the window.
    fn minimize(&self, id: WindowId) -> Result<(), PlatformError>;

    /// Maximise the window.
    fn maximize(&self, id: WindowId) -> Result<(), PlatformError>;

    /// Restore the window from minimised/maximised state.
    fn restore(&self, id: WindowId) -> Result<(), PlatformError>;

    /// Move the window to a new position.
    fn move_to(&self, id: WindowId, position: Point) -> Result<(), PlatformError>;

    /// Resize the window.
    fn resize(&self, id: WindowId, size: Size) -> Result<(), PlatformError>;
}

/// Inventory registration entry for [`WindowManager`].
pub struct WindowManagerRegistration {
    pub provider: &'static dyn WindowManager,
}

inventory::collect!(WindowManagerRegistration);

/// Iterate over all registered [`WindowManager`] implementations.
pub fn window_managers() -> impl Iterator<Item = &'static dyn WindowManager> {
    inventory::iter::<WindowManagerRegistration>.into_iter().map(|entry| entry.provider)
}

/// Register a [`WindowManager`] implementation.
#[macro_export]
macro_rules! register_window_manager {
    ($provider:expr) => {
        inventory::submit! {
            $crate::platform::WindowManagerRegistration { provider: $provider }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{PlatformError, PlatformErrorKind};
    use crate::types::{Point, Rect, Size};
    use crate::ui::{Namespace, PatternId, RuntimeId, UiAttribute, UiNode};
    use once_cell::sync::Lazy;
    use std::sync::{Arc, Weak};

    /// Minimal UiNode stub for unit tests.
    struct MinimalStubNode;

    static STUB_RUNTIME_ID: Lazy<RuntimeId> = Lazy::new(|| RuntimeId::from("stub"));

    impl UiNode for MinimalStubNode {
        fn namespace(&self) -> Namespace {
            Namespace::Control
        }
        fn role(&self) -> &str {
            "Window"
        }
        fn name(&self) -> String {
            String::from("stub")
        }
        fn runtime_id(&self) -> &RuntimeId {
            &STUB_RUNTIME_ID
        }
        fn parent(&self) -> Option<Weak<dyn UiNode>> {
            None
        }
        fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
            Box::new(std::iter::empty())
        }
        fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
            Box::new(std::iter::empty())
        }
        fn supported_patterns(&self) -> Vec<PatternId> {
            Vec::new()
        }
        fn invalidate(&self) {}
    }

    struct StubWindowManager;

    impl WindowManager for StubWindowManager {
        fn name(&self) -> &'static str {
            "stub"
        }

        fn resolve_window(&self, _node: &dyn UiNode) -> Result<WindowId, PlatformError> {
            Ok(WindowId::new(42))
        }

        fn bounds(&self, _id: WindowId) -> Result<Rect, PlatformError> {
            Ok(Rect::new(0.0, 0.0, 800.0, 600.0))
        }

        fn is_active(&self, _id: WindowId) -> Result<bool, PlatformError> {
            Ok(true)
        }

        fn activate(&self, _id: WindowId) -> Result<(), PlatformError> {
            Ok(())
        }

        fn close(&self, _id: WindowId) -> Result<(), PlatformError> {
            Err(PlatformError::new(PlatformErrorKind::OperationFailed, "close not supported"))
        }

        fn minimize(&self, _id: WindowId) -> Result<(), PlatformError> {
            Ok(())
        }

        fn maximize(&self, _id: WindowId) -> Result<(), PlatformError> {
            Ok(())
        }

        fn restore(&self, _id: WindowId) -> Result<(), PlatformError> {
            Ok(())
        }

        fn move_to(&self, _id: WindowId, _position: Point) -> Result<(), PlatformError> {
            Ok(())
        }

        fn resize(&self, _id: WindowId, _size: Size) -> Result<(), PlatformError> {
            Ok(())
        }
    }

    #[test]
    fn window_id_display() {
        let id = WindowId::new(0xFF);
        assert_eq!(format!("{id}"), "WindowId(0xff)");
    }

    #[test]
    fn window_id_raw_roundtrip() {
        let id = WindowId::new(12345);
        assert_eq!(id.raw(), 12345);
    }

    #[test]
    fn stub_resolve_returns_id() {
        let node = MinimalStubNode;
        let wm = StubWindowManager;
        let id = wm.resolve_window(&node).unwrap();
        assert_eq!(id, WindowId::new(42));
    }

    #[test]
    fn stub_bounds_returns_rect() {
        let wm = StubWindowManager;
        let rect = wm.bounds(WindowId::new(1)).unwrap();
        assert!((rect.width() - 800.0).abs() < f64::EPSILON);
    }

    #[test]
    fn stub_close_returns_error() {
        let wm = StubWindowManager;
        let err = wm.close(WindowId::new(1)).unwrap_err();
        assert!(matches!(err, PlatformError::OperationFailed { .. }));
    }
}
