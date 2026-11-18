//! Window manager abstraction for native window handle operations.
//!
//! The [`WindowManager`] trait decouples accessibility providers from
//! platform-specific windowing APIs.  Each platform crate implements the trait
//! and exposes it through its [`PlatformBundle`](crate::platform::PlatformBundle).

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
    #[must_use]
    pub fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Return the underlying raw value.
    #[must_use]
    pub fn raw(self) -> u64 {
        self.0
    }
}

impl fmt::Display for WindowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WindowId(0x{:x})", self.0)
    }
}

/// A top-level window resolved at a screen point by [`WindowManager::window_at_point`].
///
/// Carries the fields a hit-test consumer needs to correlate the native window
/// with an accessibility application and translate into window coordinates:
/// the opaque handle, the owning process id (when known), and the window's
/// screen bounds.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowHit {
    /// Opaque native window handle.
    pub id: WindowId,
    /// Owning process id, when the window manager can report it.
    pub pid: Option<u32>,
    /// The window's bounds in desktop (screen) coordinates.
    pub bounds: Rect,
}

/// How a top-level window is shown.
///
/// The three states are exclusive: a minimized window is `Minimized` even when
/// it will come back maximized once it is activated.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum WindowVisualState {
    /// Shown at its regular size and position.
    Normal,
    /// Minimized (iconified); not visible on the desktop.
    Minimized,
    /// Maximized to fill the work area.
    Maximized,
}

/// A window's state as reported by [`WindowManager::state`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct WindowState {
    /// How the window is shown.
    pub visual: WindowVisualState,
    /// Whether the window is kept above other windows. Window managers without
    /// such a concept report `false`.
    pub topmost: bool,
}

impl WindowState {
    /// Whether the window is minimized.
    #[must_use]
    pub fn is_minimized(&self) -> bool {
        self.visual == WindowVisualState::Minimized
    }

    /// Whether the window is maximized (and not minimized).
    #[must_use]
    pub fn is_maximized(&self) -> bool {
        self.visual == WindowVisualState::Maximized
    }
}

/// What a window manager knows about a window's place on the desktop.
///
/// A window's visible frame and the origin of its surface are two facts: a
/// toolkit that draws its own shadow around the window (client-side
/// decorations) places its surface outside the visible frame, so the surface
/// starts above and left of the bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowGeometry {
    /// The visible window in desktop coordinates, which the window's node
    /// reports as its `Bounds`.
    pub bounds: Rect,
    /// The desktop position of the window surface's `(0,0)`, from which a
    /// toolkit measures the positions it reports relative to its surface.
    pub surface_origin: Point,
}

impl WindowGeometry {
    /// The geometry of a window whose surface starts where its visible frame
    /// does.
    #[must_use]
    pub fn from_bounds(bounds: Rect) -> Self {
        Self { bounds, surface_origin: bounds.position() }
    }
}

/// A transient popup (a menu, drop-down or tooltip) as the platform shows it,
/// reported by [`WindowManager::popups`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PopupGeometry {
    /// The visible popup in desktop coordinates.
    pub bounds: Rect,
    /// The popup's whole surface in desktop coordinates, including a shadow
    /// the toolkit draws around the visible popup.
    pub surface: Rect,
}

impl PopupGeometry {
    /// The geometry of a popup whose surface is its visible rectangle.
    #[must_use]
    pub fn from_bounds(bounds: Rect) -> Self {
        Self { bounds, surface: bounds }
    }
}

/// Platform-native window management operations.
///
/// Implementations live in platform crates (e.g. `platform-linux-x11`,
/// `platform-windows`) and are provided via the platform's
/// [`PlatformBundle`](crate::platform::PlatformBundle).  Accessibility
/// providers call [`resolve_window`](WindowManager::resolve_window)
/// to obtain a [`WindowId`] and then invoke the desired operation.
pub trait WindowManager: Send + Sync {
    /// Human-readable name for diagnostics (e.g. `"X11 EWMH"`, `"Win32"`).
    fn name(&self) -> &'static str;

    /// Resolve the native window handle from an accessibility node.
    ///
    /// Each implementation decides which node attributes it inspects (e.g.
    /// `native:NativeWindowHandle` on Windows, PID + geometry on X11).
    ///
    /// # Errors
    ///
    /// Returns a [`PlatformError`] when no native window can be resolved for
    /// `node`.
    fn resolve_window(&self, node: &dyn UiNode) -> Result<WindowId, PlatformError>;

    /// The window's visible frame and the origin of its surface, as the window
    /// manager reports them.
    ///
    /// The platform states what it knows; what a toolkit's coordinates mean
    /// relative to it is the accessibility provider's decision. A window
    /// manager whose windows have no separate surface origin returns
    /// [`WindowGeometry::from_bounds`].
    ///
    /// # Errors
    ///
    /// Returns a [`PlatformError`] when the window manager cannot report the
    /// window's geometry (for example because the window no longer exists).
    fn geometry(&self, id: WindowId) -> Result<WindowGeometry, PlatformError>;

    /// Whether screen coordinates a toolkit reports are desktop coordinates.
    ///
    /// They are on X11 and Windows. Under a Wayland compositor a client does
    /// not know where its window is, so a toolkit's screen coordinates are
    /// relative to something the toolkit chose, and the provider re-bases them
    /// on the window. The default answers `true`.
    fn toolkit_screen_coordinates_are_desktop(&self) -> bool {
        true
    }

    /// Whether this window is the currently active (foreground) window.
    ///
    /// # Errors
    ///
    /// Returns a [`PlatformError`] when the window manager cannot determine
    /// the active window.
    fn is_active(&self, id: WindowId) -> Result<bool, PlatformError>;

    /// The window's current state: whether it is minimized, maximized or
    /// shown normally, and whether it is kept on top.
    ///
    /// The default reports the capability as unavailable, so window managers
    /// that cannot read window state never guess one.
    ///
    /// # Errors
    ///
    /// The default implementation always returns
    /// [`PlatformError::CapabilityUnavailable`]. Implementations return a
    /// [`PlatformError`] when the state cannot be read.
    fn state(&self, _id: WindowId) -> Result<WindowState, PlatformError> {
        Err(PlatformError::CapabilityUnavailable { capability: "window_state", details: None })
    }

    /// Bring the window to the foreground and make it the active window.
    ///
    /// A minimized window comes back in the state it was minimized from, so a
    /// window minimized while maximized comes back maximized. Activation never
    /// changes whether a visible window is maximized, and never moves or
    /// resizes it — returning a window to its normal state is
    /// [`restore`](WindowManager::restore)'s job.
    ///
    /// # Errors
    ///
    /// Returns a [`PlatformError`] when the window manager refuses or fails
    /// the activation.
    fn activate(&self, id: WindowId) -> Result<(), PlatformError>;

    /// Request the window manager to close this window.
    ///
    /// # Errors
    ///
    /// Returns a [`PlatformError`] when the close request cannot be delivered.
    fn close(&self, id: WindowId) -> Result<(), PlatformError>;

    /// Minimise the window.
    ///
    /// # Errors
    ///
    /// Returns a [`PlatformError`] when the window cannot be minimized.
    fn minimize(&self, id: WindowId) -> Result<(), PlatformError>;

    /// Maximise the window.
    ///
    /// # Errors
    ///
    /// Returns a [`PlatformError`] when the window cannot be maximized.
    fn maximize(&self, id: WindowId) -> Result<(), PlatformError>;

    /// Restore the window from minimised/maximised state.
    ///
    /// # Errors
    ///
    /// Returns a [`PlatformError`] when the window cannot be restored.
    fn restore(&self, id: WindowId) -> Result<(), PlatformError>;

    /// Move the window to a new position.
    ///
    /// # Errors
    ///
    /// Returns a [`PlatformError`] when the window cannot be moved.
    fn move_to(&self, id: WindowId, position: Point) -> Result<(), PlatformError>;

    /// Resize the window.
    ///
    /// # Errors
    ///
    /// Returns a [`PlatformError`] when the window cannot be resized.
    fn resize(&self, id: WindowId, size: Size) -> Result<(), PlatformError>;

    /// Returns the frontmost top-level window at the given desktop point,
    /// respecting the window manager's stacking order, or `None` if no managed
    /// window covers the point.
    ///
    /// This is the authoritative source of *window-level* z-order — used by
    /// hit-test consumers (e.g. the AT-SPI provider) to pick the right
    /// application window before descending into it. The default reports the
    /// operation as unavailable so window managers that cannot answer it (and
    /// platforms that do not need it, e.g. Windows where UIA `ElementFromPoint`
    /// resolves z-order natively) are unaffected.
    ///
    /// # Errors
    ///
    /// The default implementation always returns
    /// [`PlatformError::CapabilityUnavailable`]. Implementations return a
    /// [`PlatformError`] when the window stack cannot be queried.
    fn window_at_point(&self, _point: Point) -> Result<Option<WindowHit>, PlatformError> {
        Err(PlatformError::CapabilityUnavailable { capability: "window_at_point", details: None })
    }

    /// The transient popups (menus, drop-downs, tooltips) currently shown for
    /// the given process, each with its visible rectangle and its surface,
    /// most recently opened or topmost first.
    ///
    /// A popup is a surface of its own, so the platform knows where it is even
    /// when the toolkit's accessibility tree measures it from the widget that
    /// opened it. The default reports the capability as unavailable, and a
    /// provider then places popups like any other node of their window.
    ///
    /// # Errors
    ///
    /// The default implementation always returns
    /// [`PlatformError::CapabilityUnavailable`]. Implementations return a
    /// [`PlatformError`] when the popup list cannot be queried.
    fn popups(&self, _pid: u32) -> Result<Vec<PopupGeometry>, PlatformError> {
        Err(PlatformError::CapabilityUnavailable { capability: "popups", details: None })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::PlatformError;
    use crate::types::{Point, Rect, Size};
    use crate::ui::{Namespace, PatternName, RuntimeId, UiAttribute, UiNode};
    use std::sync::{Arc, LazyLock, Weak};

    /// Minimal `UiNode` stub for unit tests.
    struct MinimalStubNode;

    static STUB_RUNTIME_ID: LazyLock<RuntimeId> = LazyLock::new(|| RuntimeId::from("stub"));

    impl UiNode for MinimalStubNode {
        fn namespace(&self) -> Namespace {
            Namespace::Control
        }
        fn role(&self) -> &'static str {
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
        fn supported_patterns(&self) -> Vec<PatternName> {
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

        fn geometry(&self, _id: WindowId) -> Result<WindowGeometry, PlatformError> {
            Ok(WindowGeometry::from_bounds(Rect::new(10.0, 20.0, 800.0, 600.0)))
        }

        fn is_active(&self, _id: WindowId) -> Result<bool, PlatformError> {
            Ok(true)
        }

        fn activate(&self, _id: WindowId) -> Result<(), PlatformError> {
            Ok(())
        }

        fn close(&self, _id: WindowId) -> Result<(), PlatformError> {
            Err(PlatformError::OperationFailed {
                operation: "close window",
                details: Some("close not supported".into()),
            })
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
    fn a_window_without_a_separate_surface_origin_has_its_surface_at_its_bounds() {
        let geometry = StubWindowManager.geometry(WindowId::new(1)).unwrap();
        assert_eq!(geometry.bounds, Rect::new(10.0, 20.0, 800.0, 600.0));
        assert_eq!(geometry.surface_origin, geometry.bounds.position());
    }

    #[test]
    fn toolkit_screen_coordinates_are_desktop_coordinates_by_default() {
        assert!(StubWindowManager.toolkit_screen_coordinates_are_desktop());
    }

    #[test]
    fn popups_are_unavailable_unless_implemented() {
        let err = StubWindowManager.popups(1).unwrap_err();
        assert!(matches!(err, PlatformError::CapabilityUnavailable { capability: "popups", .. }));
    }

    #[test]
    fn a_popup_carries_its_visible_rectangle_and_its_surface() {
        // A GTK 4 popover under the PlatynUI compositor: the surface includes the
        // shadow GTK draws around the visible menu (design.md, Context).
        let popup = PopupGeometry {
            bounds: Rect::new(110.0, 60.0, 161.0, 110.0),
            surface: Rect::new(87.0, 39.0, 207.0, 156.0),
        };
        assert_eq!(popup.bounds.size(), Size::new(161.0, 110.0));
        assert_eq!(popup.surface.size(), Size::new(207.0, 156.0));

        let without_shadow = PopupGeometry::from_bounds(Rect::new(5.0, 6.0, 70.0, 80.0));
        assert_eq!(without_shadow.surface, without_shadow.bounds);
    }

    #[test]
    fn state_is_unavailable_unless_implemented() {
        let wm = StubWindowManager;
        let err = wm.state(WindowId::new(1)).unwrap_err();
        assert!(matches!(err, PlatformError::CapabilityUnavailable { capability: "window_state", .. }));
    }

    #[test]
    fn window_state_flags_follow_the_visual_state() {
        let minimized = WindowState { visual: WindowVisualState::Minimized, topmost: false };
        assert!(minimized.is_minimized() && !minimized.is_maximized());
        let maximized = WindowState { visual: WindowVisualState::Maximized, topmost: true };
        assert!(maximized.is_maximized() && !maximized.is_minimized());
        let normal = WindowState { visual: WindowVisualState::Normal, topmost: false };
        assert!(!normal.is_minimized() && !normal.is_maximized());
    }

    #[test]
    fn stub_close_returns_error() {
        let wm = StubWindowManager;
        let err = wm.close(WindowId::new(1)).unwrap_err();
        assert!(matches!(err, PlatformError::OperationFailed { .. }));
    }
}
