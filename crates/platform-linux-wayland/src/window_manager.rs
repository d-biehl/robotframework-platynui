use platynui_core::platform::{PlatformError, PlatformErrorKind, WindowId, WindowManager};
use platynui_core::register_window_manager;
use platynui_core::types::{Point, Rect, Size};
use platynui_core::ui::UiNode;
use std::os::unix::net::UnixStream;
use std::sync::OnceLock;

/// Wayland window manager with compositor-specific IPC backends.
///
/// Operations not covered by standard Wayland protocols (window bounds,
/// move, resize) require compositor-specific IPC:
/// - **`KWin`**: D-Bus `org.kde.KWin`
/// - **Mutter**: limited — bounds/move/resize not exposed via public API
/// - **`PlatynUI` compositor**: test-control socket
///
/// Standard protocol operations (`activate`, `close`, minimize, maximize)
/// will use `ext-foreign-toplevel-list` / `wlr-foreign-toplevel-management`
/// once implemented.
struct WaylandWindowManager;

impl WindowManager for WaylandWindowManager {
    fn name(&self) -> &'static str {
        "Wayland"
    }

    fn resolve_window(&self, node: &dyn UiNode) -> Result<WindowId, PlatformError> {
        // TODO: Walk the accessibility tree upward to find a Window node,
        // then match against known toplevels via ext-foreign-toplevel-list.
        let _ = node;
        Err(not_yet("resolve_window"))
    }

    fn bounds(&self, id: WindowId) -> Result<Rect, PlatformError> {
        match compositor() {
            CompositorKind::KWin => kwin_window_bounds(id),
            CompositorKind::PlatynUI => platynui_window_bounds(id),
            CompositorKind::Mutter | CompositorKind::Unknown => Err(PlatformError::new(
                PlatformErrorKind::CapabilityUnavailable,
                "window bounds not available on this compositor",
            )),
        }
    }

    fn is_active(&self, id: WindowId) -> Result<bool, PlatformError> {
        // TODO: Implement via ext-foreign-toplevel-list state events.
        let _ = id;
        Err(not_yet("is_active"))
    }

    fn activate(&self, id: WindowId) -> Result<(), PlatformError> {
        // TODO: Implement via ext-foreign-toplevel-list / wlr-foreign-toplevel activate.
        let _ = id;
        Err(not_yet("activate"))
    }

    fn close(&self, id: WindowId) -> Result<(), PlatformError> {
        // TODO: Implement via wlr-foreign-toplevel-management close.
        let _ = id;
        Err(not_yet("close"))
    }

    fn minimize(&self, id: WindowId) -> Result<(), PlatformError> {
        // TODO: Implement via wlr-foreign-toplevel-management set_minimized.
        let _ = id;
        Err(not_yet("minimize"))
    }

    fn maximize(&self, id: WindowId) -> Result<(), PlatformError> {
        // TODO: Implement via wlr-foreign-toplevel-management set_maximized.
        let _ = id;
        Err(not_yet("maximize"))
    }

    fn restore(&self, id: WindowId) -> Result<(), PlatformError> {
        // TODO: Implement via wlr-foreign-toplevel-management unset_minimized / unset_maximized.
        let _ = id;
        Err(not_yet("restore"))
    }

    fn move_to(&self, id: WindowId, position: Point) -> Result<(), PlatformError> {
        match compositor() {
            CompositorKind::KWin => kwin_window_move(id, position),
            CompositorKind::PlatynUI => platynui_window_move(id, position),
            CompositorKind::Mutter | CompositorKind::Unknown => Err(PlatformError::new(
                PlatformErrorKind::CapabilityUnavailable,
                "window move not available on this compositor",
            )),
        }
    }

    fn resize(&self, id: WindowId, size: Size) -> Result<(), PlatformError> {
        match compositor() {
            CompositorKind::KWin => kwin_window_resize(id, size),
            CompositorKind::PlatynUI => platynui_window_resize(id, size),
            CompositorKind::Mutter | CompositorKind::Unknown => Err(PlatformError::new(
                PlatformErrorKind::CapabilityUnavailable,
                "window resize not available on this compositor",
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Compositor detection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompositorKind {
    KWin,
    Mutter,
    PlatynUI,
    Unknown,
}

/// Cached compositor kind, detected once on first access.
static COMPOSITOR: OnceLock<CompositorKind> = OnceLock::new();

fn compositor() -> CompositorKind {
    *COMPOSITOR.get_or_init(detect_compositor)
}

/// Detect the running compositor.
///
/// Primary method (same approach as waycheck): connect to the Wayland display
/// socket, use `SO_PEERCRED` to get the compositor PID, then read
/// `/proc/<pid>/comm` for the process name.
///
/// Fallback: environment variables (`PLATYNUI_CONTROL_SOCKET`,
/// `XDG_CURRENT_DESKTOP`, `KDE_FULL_SESSION`).
fn detect_compositor() -> CompositorKind {
    if let Some(kind) = detect_via_peercred() {
        tracing::info!(compositor = ?kind, "detected compositor via SO_PEERCRED");
        return kind;
    }
    let kind = detect_via_env();
    tracing::info!(compositor = ?kind, "detected compositor via environment variables");
    kind
}

/// Identify the compositor by connecting to the Wayland display socket and
/// reading the peer process name via `SO_PEERCRED` + `/proc/<pid>/comm`.
fn detect_via_peercred() -> Option<CompositorKind> {
    use std::os::unix::io::AsFd;

    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").ok()?;
    let display = std::env::var("WAYLAND_DISPLAY").ok()?;
    let path = if display.starts_with('/') { display } else { format!("{runtime_dir}/{display}") };

    let stream = UnixStream::connect(&path).ok()?;
    let cred = rustix::net::sockopt::socket_peercred(stream.as_fd()).ok()?;
    let pid = cred.pid;

    let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    let name = comm.trim();

    tracing::debug!(pid = pid.as_raw_nonzero().get(), name, "wayland compositor process");

    Some(match_compositor_name(name))
}

/// Map a process name from `/proc/<pid>/comm` to a [`CompositorKind`].
fn match_compositor_name(name: &str) -> CompositorKind {
    match name {
        "kwin_wayland" => CompositorKind::KWin,
        "gnome-shell" | "mutter" => CompositorKind::Mutter,
        // /proc/pid/comm truncates to 15 chars, so our binary
        // "platynui-wayland-compositor" becomes "platynui-waylan".
        n if n.starts_with("platynui") => CompositorKind::PlatynUI,
        _ => CompositorKind::Unknown,
    }
}

/// Fallback compositor detection via environment variables.
fn detect_via_env() -> CompositorKind {
    // PlatynUI compositor exports its control socket.
    if std::env::var("PLATYNUI_CONTROL_SOCKET").is_ok() {
        return CompositorKind::PlatynUI;
    }
    // XDG_CURRENT_DESKTOP is set by the display manager.
    if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
        let lower = desktop.to_lowercase();
        if lower.contains("kde") {
            return CompositorKind::KWin;
        }
        if lower.contains("gnome") {
            return CompositorKind::Mutter;
        }
    }
    if std::env::var("KDE_FULL_SESSION").is_ok() {
        return CompositorKind::KWin;
    }
    if std::env::var("GNOME_DESKTOP_SESSION_ID").is_ok() {
        return CompositorKind::Mutter;
    }
    CompositorKind::Unknown
}

// ---------------------------------------------------------------------------
// Compositor IPC stubs — KWin (D-Bus org.kde.KWin)
// ---------------------------------------------------------------------------

fn kwin_window_bounds(id: WindowId) -> Result<Rect, PlatformError> {
    // TODO: Implement via D-Bus `org.kde.KWin` scripting API.
    let _ = id;
    Err(not_yet("kwin window bounds"))
}

fn kwin_window_move(id: WindowId, position: Point) -> Result<(), PlatformError> {
    let _ = (id, position);
    Err(not_yet("kwin window move"))
}

fn kwin_window_resize(id: WindowId, size: Size) -> Result<(), PlatformError> {
    let _ = (id, size);
    Err(not_yet("kwin window resize"))
}

// ---------------------------------------------------------------------------
// Compositor IPC stubs — PlatynUI compositor (control socket)
// ---------------------------------------------------------------------------

fn platynui_window_bounds(id: WindowId) -> Result<Rect, PlatformError> {
    // TODO: Implement via PLATYNUI_CONTROL_SOCKET IPC.
    let _ = id;
    Err(not_yet("platynui compositor window bounds"))
}

fn platynui_window_move(id: WindowId, position: Point) -> Result<(), PlatformError> {
    let _ = (id, position);
    Err(not_yet("platynui compositor window move"))
}

fn platynui_window_resize(id: WindowId, size: Size) -> Result<(), PlatformError> {
    let _ = (id, size);
    Err(not_yet("platynui compositor window resize"))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn not_yet(operation: &str) -> PlatformError {
    PlatformError::new(
        PlatformErrorKind::CapabilityUnavailable,
        format!("wayland window manager: {operation} not yet implemented"),
    )
}

static PROVIDER: WaylandWindowManager = WaylandWindowManager;

register_window_manager!(&PROVIDER);
