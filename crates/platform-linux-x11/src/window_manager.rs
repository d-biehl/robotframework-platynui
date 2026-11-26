//! EWMH-based [`WindowManager`] for X11.
//!
//! Migrated from `provider-atspi/src/ewmh.rs`.  Holds the runtime's owned X11
//! connection ([`crate::x11util::X11Connection`]) and acts as a platform-level
//! window manager so any accessibility provider can resolve and manage native
//! windows without a direct `x11rb` dependency.
//!
//! The interned-atom cache (`ATOMS`) stays a process-global `OnceLock`: atoms
//! are stable for the lifetime of the X server, so caching them across runtimes
//! is safe and it is never cleared on shutdown. Only the connection is
//! per-instance.

use crate::x11util::X11Connection;
use platynui_core::platform::{
    PlatformError, PopupGeometry, WindowGeometry, WindowHit, WindowId, WindowManager, WindowState, WindowVisualState,
};
use platynui_core::types::{Point, Rect, Size};
use platynui_core::ui::{Namespace, UiNode};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tracing::{debug, info, trace, warn};
use x11rb::connection::Connection;
use x11rb::protocol::res::{ClientIdMask, ClientIdSpec, ClientIdValue, ConnectionExt as _};
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ClientMessageEvent, ConfigureWindowAux, ConnectionExt, EventMask, InputFocus, MapState, StackMode,
    Window,
};
use x11rb::rust_connection::RustConnection;

// ---------------------------------------------------------------------------
//  Atom cache
// ---------------------------------------------------------------------------

struct EwmhAtoms {
    net_client_list: Atom,
    net_client_list_stacking: Atom,
    net_wm_pid: Atom,
    net_active_window: Atom,
    net_close_window: Atom,
    net_wm_state: Atom,
    net_wm_state_maximized_vert: Atom,
    net_wm_state_maximized_horz: Atom,
    net_wm_state_hidden: Atom,
    net_wm_state_above: Atom,
    wm_state: Atom,
    net_supporting_wm_check: Atom,
    net_supported: Atom,
    net_wm_name: Atom,
    utf8_string: Atom,
    gtk_frame_extents: Atom,
    wm_protocols: Atom,
    wm_delete_window: Atom,
}

static ATOMS: OnceLock<Mutex<EwmhAtoms>> = OnceLock::new();

fn intern(conn: &RustConnection, name: &[u8]) -> Result<Atom, PlatformError> {
    conn.intern_atom(false, name)
        .map_err(|e| PlatformError::OperationFailed {
            operation: "x11 intern_atom request",
            details: Some(e.to_string()),
        })?
        .reply()
        .map(|r| r.atom)
        .map_err(|e| PlatformError::OperationFailed {
            operation: "x11 intern_atom reply",
            details: Some(e.to_string()),
        })
}

fn atoms(x11: &X11Connection) -> Result<std::sync::MutexGuard<'static, EwmhAtoms>, PlatformError> {
    if let Some(cell) = ATOMS.get() {
        return cell.lock().map_err(|_| PlatformError::OperationFailed {
            operation: "ewmh atoms lock",
            details: Some("poisoned".into()),
        });
    }
    let conn = &x11.conn;
    let a = EwmhAtoms {
        net_client_list: intern(conn, b"_NET_CLIENT_LIST")?,
        net_client_list_stacking: intern(conn, b"_NET_CLIENT_LIST_STACKING")?,
        net_wm_pid: intern(conn, b"_NET_WM_PID")?,
        net_active_window: intern(conn, b"_NET_ACTIVE_WINDOW")?,
        net_close_window: intern(conn, b"_NET_CLOSE_WINDOW")?,
        net_wm_state: intern(conn, b"_NET_WM_STATE")?,
        net_wm_state_maximized_vert: intern(conn, b"_NET_WM_STATE_MAXIMIZED_VERT")?,
        net_wm_state_maximized_horz: intern(conn, b"_NET_WM_STATE_MAXIMIZED_HORZ")?,
        net_wm_state_hidden: intern(conn, b"_NET_WM_STATE_HIDDEN")?,
        net_wm_state_above: intern(conn, b"_NET_WM_STATE_ABOVE")?,
        wm_state: intern(conn, b"WM_STATE")?,
        net_supporting_wm_check: intern(conn, b"_NET_SUPPORTING_WM_CHECK")?,
        net_supported: intern(conn, b"_NET_SUPPORTED")?,
        net_wm_name: intern(conn, b"_NET_WM_NAME")?,
        utf8_string: intern(conn, b"UTF8_STRING")?,
        gtk_frame_extents: intern(conn, b"_GTK_FRAME_EXTENTS")?,
        wm_protocols: intern(conn, b"WM_PROTOCOLS")?,
        wm_delete_window: intern(conn, b"WM_DELETE_WINDOW")?,
    };
    let _ = ATOMS.set(Mutex::new(a));
    ATOMS
        .get()
        .expect("just initialised")
        .lock()
        .map_err(|_| PlatformError::OperationFailed { operation: "ewmh atoms lock", details: Some("poisoned".into()) })
}

// ---------------------------------------------------------------------------
//  XID resolution helpers
// ---------------------------------------------------------------------------

/// The X11 window behind `id`. This window manager issues ids by widening a
/// 32-bit XID, so the truncating cast returns exactly that XID.
#[allow(clippy::cast_possible_truncation)]
fn xid_of(id: WindowId) -> Window {
    id.raw() as Window
}

fn get_client_list(conn: &RustConnection, root: Window, net_client_list: Atom) -> Result<Vec<Window>, PlatformError> {
    let reply = conn
        .get_property(false, root, net_client_list, AtomEnum::WINDOW, 0, u32::MAX)
        .map_err(|e| PlatformError::OperationFailed {
            operation: "read _NET_CLIENT_LIST",
            details: Some(e.to_string()),
        })?
        .reply()
        .map_err(|e| PlatformError::OperationFailed {
            operation: "read _NET_CLIENT_LIST reply",
            details: Some(e.to_string()),
        })?;
    Ok(reply.value32().map(Iterator::collect).unwrap_or_default())
}

fn get_window_pid(conn: &RustConnection, win: Window, net_wm_pid: Atom) -> Option<u32> {
    let reply = conn.get_property(false, win, net_wm_pid, AtomEnum::CARDINAL, 0, 1).ok()?.reply().ok()?;
    reply.value32().and_then(|mut iter| iter.next())
}

/// Read the `_NET_WM_NAME` (UTF-8) of a window, falling back to `WM_NAME`.
fn get_window_name(conn: &RustConnection, win: Window, atoms: &EwmhAtoms) -> Option<String> {
    // Try _NET_WM_NAME (UTF-8) first.
    if let Ok(reply) = conn
        .get_property(false, win, atoms.net_wm_name, atoms.utf8_string, 0, 1024)
        .ok()
        .and_then(|c| c.reply().ok())
        .ok_or(())
    {
        let bytes = reply.value;
        if !bytes.is_empty()
            && let Ok(name) = String::from_utf8(bytes)
        {
            return Some(name);
        }
    }
    // Fallback: WM_NAME (Latin-1 / compound text).
    if let Some(reply) =
        conn.get_property(false, win, AtomEnum::WM_NAME, AtomEnum::STRING, 0, 1024).ok().and_then(|c| c.reply().ok())
    {
        let bytes = reply.value;
        if !bytes.is_empty() {
            return Some(String::from_utf8_lossy(&bytes).into_owned());
        }
    }
    None
}

/// Find the X11 window of the given PID that the node stands for.
///
/// A process with one window has it. With several (a main window and its
/// dialogs), [`choose_window`] decides from what the node tells about its
/// window.
fn find_xid_for_pid(
    x11: &X11Connection,
    pid: u32,
    node: &WindowHints,
    mode: WindowMode,
) -> Result<Window, PlatformError> {
    let atoms = atoms(x11)?;
    let windows: Vec<Window> = match mode {
        WindowMode::Managed => get_client_list(&x11.conn, x11.root, atoms.net_client_list)?
            .into_iter()
            .filter(|&win| get_window_pid(&x11.conn, win, atoms.net_wm_pid) == Some(pid))
            .collect(),
        WindowMode::Bare => {
            application_windows(&root_windows(x11, atoms.net_wm_pid)?, pid).iter().map(|window| window.xid).collect()
        }
    };

    let mut candidates = Vec::new();
    for win in windows {
        let Ok(rect) = visible_window_rect(x11, win, atoms.gtk_frame_extents) else { continue };
        candidates.push(CandidateWindow { xid: win, title: get_window_name(&x11.conn, win, &atoms), rect });
    }

    if candidates.is_empty() {
        warn!(pid, "no X11 window found for PID");
        return Err(PlatformError::OperationFailed {
            operation: "resolve X11 window by PID",
            details: Some(format!("no window found for PID {pid}")),
        });
    }
    let xid = choose_window(&candidates, node, pid).inspect_err(|_| {
        warn!(pid, count = candidates.len(), "could not tell the windows of a PID apart");
    })?;
    debug!(pid, xid, candidates = candidates.len(), "resolved XID for PID");
    Ok(xid)
}

/// A window of the node's process, as the lookup sees it.
#[derive(Clone, Debug)]
struct CandidateWindow {
    xid: Window,
    /// `_NET_WM_NAME`, or `WM_NAME` when the window has none.
    title: Option<String>,
    /// The visible window ([`visible_rect`]).
    rect: Rect,
}

/// What the node tells about its window.
#[derive(Clone, Debug, Default)]
struct WindowHints {
    /// The node's accessible name, which toolkits set to the window title.
    name: Option<String>,
    /// The window's size as its toolkit reports it.
    size: Option<Size>,
    /// The window's screen position as its toolkit reports it; `None` when
    /// the toolkit reports none, which GTK 4 does by reporting `(0,0)`.
    position: Option<Point>,
}

/// How far, summed over both dimensions, a candidate's size or position may
/// lie from the node's to count as the same window. Toolkits that include the
/// window manager's frame in their window's extents (GTK 3 on X11) stay
/// within it; windows the lookup must tell apart differ by more.
const WINDOW_MATCH_TOLERANCE: f64 = 64.0;

/// Choose the node's window among the windows of its process: by the exact
/// title first, then by a size only one candidate has, and among candidates of
/// equal size by the screen position the toolkit reports. Fails, naming the
/// number of candidates, when none of these singles out one window — taking
/// another window's geometry is worse than an explicit failure.
fn choose_window(candidates: &[CandidateWindow], node: &WindowHints, pid: u32) -> Result<Window, PlatformError> {
    if let [only] = candidates {
        return Ok(only.xid);
    }
    let mut pool: Vec<&CandidateWindow> = candidates.iter().collect();

    if let Some(name) = node.name.as_deref() {
        let titled: Vec<&CandidateWindow> = pool.iter().copied().filter(|c| c.title.as_deref() == Some(name)).collect();
        if let [only] = titled.as_slice() {
            return Ok(only.xid);
        }
        if !titled.is_empty() {
            pool = titled;
        }
    }

    if let Some(size) = node.size {
        let sized: Vec<&CandidateWindow> = pool
            .iter()
            .copied()
            .filter(|c| {
                (c.rect.width() - size.width()).abs() + (c.rect.height() - size.height()).abs()
                    <= WINDOW_MATCH_TOLERANCE
            })
            .collect();
        if let [only] = sized.as_slice() {
            return Ok(only.xid);
        }
        if !sized.is_empty() {
            pool = sized;
        }
    }

    if let Some(position) = node.position {
        let distance = |c: &CandidateWindow| (c.rect.x() - position.x()).abs() + (c.rect.y() - position.y()).abs();
        if let Some(closest) = pool.iter().copied().min_by(|a, b| distance(a).total_cmp(&distance(b)))
            && distance(closest) <= WINDOW_MATCH_TOLERANCE
        {
            return Ok(closest.xid);
        }
    }

    Err(PlatformError::OperationFailed {
        operation: "disambiguate X11 window for PID",
        details: Some(format!(
            "{} candidate windows for PID {pid}, none singled out by title, size or screen position",
            candidates.len()
        )),
    })
}

/// Whether a window manager manages the display, decided once when the
/// runtime connects (`window_mode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowMode {
    /// An EWMH window manager runs: windows are found and operated through it.
    Managed,
    /// No window manager runs: windows are found and operated through the X
    /// server directly.
    Bare,
}

/// The window mode from the window-manager check: managed when an EWMH window
/// manager answered, bare when the check found none. A check that could not be
/// made keeps the window-manager path.
pub(crate) fn window_mode(check: &Result<bool, PlatformError>) -> WindowMode {
    match check {
        Ok(false) => WindowMode::Bare,
        Ok(true) | Err(_) => WindowMode::Managed,
    }
}

/// Whether a toolkit's screen coordinates are desktop coordinates in a window
/// mode. A window manager tells a client where its window went after another
/// client moved it (the synthetic `ConfigureNotify` ICCCM requires); without
/// one, a toolkit may keep counting from where its window opened.
fn screen_coordinates_are_desktop(mode: WindowMode) -> bool {
    mode == WindowMode::Managed
}

/// A child of the root window, as the window lookup without a window manager
/// sees it.
#[derive(Clone, Copy, Debug)]
struct RootWindow {
    xid: Window,
    viewable: bool,
    override_redirect: bool,
    /// Its `_NET_WM_PID`.
    pid: Option<u32>,
}

/// Whether a root child is an application's top-level window: viewable, not
/// override-redirect (which menus and tooltips are), and carrying a process
/// ID, which toolkits set themselves.
fn is_top_level(window: &RootWindow) -> bool {
    window.viewable && !window.override_redirect && window.pid.is_some()
}

/// The windows of process `pid` among the root's children, without a window
/// manager.
fn application_windows(children: &[RootWindow], pid: u32) -> Vec<RootWindow> {
    children.iter().filter(|window| is_top_level(window) && window.pid == Some(pid)).copied().collect()
}

/// The root's children (bottom to top) with what the lookup without a window
/// manager needs of them.
fn root_windows(x11: &X11Connection, pid_atom: Atom) -> Result<Vec<RootWindow>, PlatformError> {
    let tree = x11
        .conn
        .query_tree(x11.root)
        .map_err(|e| PlatformError::OperationFailed { operation: "x11 query_tree", details: Some(e.to_string()) })?
        .reply()
        .map_err(|e| PlatformError::OperationFailed {
            operation: "x11 query_tree reply",
            details: Some(e.to_string()),
        })?;
    Ok(tree
        .children
        .iter()
        .filter_map(|&xid| {
            let attrs = x11.conn.get_window_attributes(xid).ok()?.reply().ok()?;
            Some(RootWindow {
                xid,
                viewable: attrs.map_state == MapState::VIEWABLE,
                override_redirect: attrs.override_redirect,
                pid: get_window_pid(&x11.conn, xid, pid_atom),
            })
        })
        .collect())
}

/// The input focus `None`: no window has it.
const FOCUS_NONE: Window = 0;
/// The input focus `PointerRoot`: it follows the pointer's root window.
const FOCUS_POINTER_ROOT: Window = 1;

/// The root child that holds the input focus: the focused window itself, or
/// the ancestor of it whose parent is the root. `None` when the focus is
/// `None`, `PointerRoot` or the root, or when the walk loses the tree.
fn focused_top_level(focus: Window, root: Window, parent_of: impl Fn(Window) -> Option<Window>) -> Option<Window> {
    if focus == FOCUS_NONE || focus == FOCUS_POINTER_ROOT || focus == root {
        return None;
    }
    let mut current = focus;
    loop {
        let parent = parent_of(current)?;
        if parent == root {
            return Some(current);
        }
        current = parent;
    }
}

/// How activation raises `window` among the top-level windows, which the X
/// server lists bottom to top: `None` when it is the topmost already,
/// otherwise the sibling to place it directly above — the topmost other one.
/// Raising it above that sibling rather than above every root child keeps the
/// override-redirect popups stacked above the windows, an open menu of the
/// window's own process among them.
fn raise_above(top_levels: &[Window], window: Window) -> Option<Window> {
    let topmost = *top_levels.last()?;
    (topmost != window).then_some(topmost)
}

/// Whether a window can be asked to close: its `WM_PROTOCOLS` list
/// `WM_DELETE_WINDOW`.
fn check_delete_protocol(protocols: &[Atom], wm_delete_window: Atom) -> Result<(), PlatformError> {
    if protocols.contains(&wm_delete_window) {
        Ok(())
    } else {
        Err(PlatformError::OperationFailed {
            operation: "close X11 window",
            details: Some("the window does not list WM_DELETE_WINDOW in its WM_PROTOCOLS".into()),
        })
    }
}

/// The error for a window manager's own operation on a display without one.
fn needs_window_manager(capability: &'static str) -> PlatformError {
    PlatformError::CapabilityUnavailable { capability, details: Some("no window manager runs on the display".into()) }
}

/// How long activation waits for a window it mapped to become viewable; the
/// input focus can only go to a viewable window.
const MAP_TIMEOUT: Duration = Duration::from_secs(2);

/// The `_GTK_FRAME_EXTENTS` of a window: the insets of the shadow GTK draws
/// around a client-side-decorated window under a compositing window manager.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FrameExtents {
    left: u32,
    right: u32,
    top: u32,
    bottom: u32,
}

/// A window's visible rectangle: its client rectangle without the frame
/// extents, when it has any.
fn visible_rect(client: Rect, extents: Option<FrameExtents>) -> Rect {
    let Some(e) = extents else { return client };
    Rect::new(
        client.x() + f64::from(e.left),
        client.y() + f64::from(e.top),
        client.width() - f64::from(e.left + e.right),
        client.height() - f64::from(e.top + e.bottom),
    )
}

/// Read a window's `_GTK_FRAME_EXTENTS` (four cardinals: left, right, top,
/// bottom). `None` when the window has no such property.
fn read_frame_extents(conn: &RustConnection, win: Window, atom: Atom) -> Option<FrameExtents> {
    let reply = conn.get_property(false, win, atom, AtomEnum::CARDINAL, 0, 4).ok()?.reply().ok()?;
    let mut values = reply.value32()?;
    Some(FrameExtents { left: values.next()?, right: values.next()?, top: values.next()?, bottom: values.next()? })
}

/// A child of the root window, as the popup listing sees it.
#[derive(Clone, Copy, Debug)]
struct RootChild {
    viewable: bool,
    override_redirect: bool,
    /// Whether its `_NET_WM_WINDOW_TYPE` marks a menu, combo or tooltip.
    popup_type: bool,
    /// Its `_NET_WM_PID`.
    pid: Option<u32>,
    client: Rect,
    frame_extents: Option<FrameExtents>,
}

/// The popups of process `pid` among the root's children, which the X server
/// lists bottom to top: the viewable override-redirect windows of a popup type
/// that carry the process's `_NET_WM_PID`, topmost first. A popup's surface is
/// its window; its visible rectangle leaves out GTK's shadow
/// (`_GTK_FRAME_EXTENTS`).
fn popups_of(children: &[RootChild], pid: u32) -> Vec<PopupGeometry> {
    children
        .iter()
        .rev()
        .filter(|c| c.viewable && c.override_redirect && c.popup_type && c.pid == Some(pid))
        .map(|c| PopupGeometry { bounds: visible_rect(c.client, c.frame_extents), surface: c.client })
        .collect()
}

/// A window's visible rectangle ([`visible_rect`]) on the screen.
fn visible_window_rect(x11: &X11Connection, win: Window, frame_extents_atom: Atom) -> Result<Rect, PlatformError> {
    Ok(visible_rect(client_rect(x11, win)?, read_frame_extents(&x11.conn, win, frame_extents_atom)))
}

/// Compute a window's client rect: screen position of its client origin
/// (`translate_coordinates`) plus its client size (`get_geometry`). Shared by
/// [`visible_window_rect`] and the window hit-test.
fn client_rect(x11: &X11Connection, xid: Window) -> Result<Rect, PlatformError> {
    let geom = x11
        .conn
        .get_geometry(xid)
        .map_err(|e| PlatformError::OperationFailed { operation: "x11 get_geometry", details: Some(e.to_string()) })?
        .reply()
        .map_err(|e| PlatformError::OperationFailed {
            operation: "x11 get_geometry reply",
            details: Some(e.to_string()),
        })?;
    let coords = x11.conn.translate_coordinates(xid, x11.root, 0, 0).ok().and_then(|c| c.reply().ok());
    let (wx, wy) = coords.map_or((f64::from(geom.x), f64::from(geom.y)), |c| (f64::from(c.dst_x), f64::from(c.dst_y)));
    Ok(Rect::new(wx, wy, f64::from(geom.width), f64::from(geom.height)))
}

/// ICCCM `WM_STATE` value of a window the window manager has iconified.
const ICONIC_STATE: u32 = 3;

/// Read a client window's state from `_NET_WM_STATE` and the ICCCM `WM_STATE`.
/// Fails when the window no longer exists (the property request errors).
fn read_window_state(x11: &X11Connection, xid: Window, atoms: &EwmhAtoms) -> Result<WindowState, PlatformError> {
    let net_states: Vec<Atom> = x11
        .conn
        .get_property(false, xid, atoms.net_wm_state, AtomEnum::ATOM, 0, 64)
        .map_err(|e| PlatformError::OperationFailed { operation: "read _NET_WM_STATE", details: Some(e.to_string()) })?
        .reply()
        .map_err(|e| PlatformError::OperationFailed {
            operation: "read _NET_WM_STATE reply",
            details: Some(e.to_string()),
        })?
        .value32()
        .map(Iterator::collect)
        .unwrap_or_default();
    let iconic = x11
        .conn
        .get_property(false, xid, atoms.wm_state, atoms.wm_state, 0, 1)
        .ok()
        .and_then(|cookie| cookie.reply().ok())
        .and_then(|reply| reply.value32().and_then(|mut iter| iter.next()))
        == Some(ICONIC_STATE);
    Ok(decode_window_state(&net_states, iconic, atoms))
}

/// Map `_NET_WM_STATE` atoms plus the ICCCM iconic flag onto a [`WindowState`].
/// Minimized wins: an iconified window keeps its maximized atoms so the window
/// manager can bring it back maximized, but it is not visible as maximized.
fn decode_window_state(net_states: &[Atom], iconic: bool, atoms: &EwmhAtoms) -> WindowState {
    let has = |atom: Atom| net_states.contains(&atom);
    let visual = if iconic || has(atoms.net_wm_state_hidden) {
        WindowVisualState::Minimized
    } else if has(atoms.net_wm_state_maximized_vert) && has(atoms.net_wm_state_maximized_horz) {
        WindowVisualState::Maximized
    } else {
        WindowVisualState::Normal
    };
    WindowState { visual, topmost: has(atoms.net_wm_state_above) }
}

// ---------------------------------------------------------------------------
//  EWMH client messages
// ---------------------------------------------------------------------------

fn send_client_message(
    conn: &RustConnection,
    root: Window,
    win: Window,
    message_type: Atom,
    data: [u32; 5],
) -> Result<(), PlatformError> {
    let event = ClientMessageEvent::new(32, win, message_type, data);
    let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
    conn.send_event(false, root, mask, event)
        .map_err(|e| PlatformError::OperationFailed { operation: "x11 send_event", details: Some(e.to_string()) })?;
    Ok(())
}

fn flush(conn: &RustConnection) -> Result<(), PlatformError> {
    conn.flush().map_err(|e| PlatformError::OperationFailed { operation: "x11 flush", details: Some(e.to_string()) })
}

// ---------------------------------------------------------------------------
//  Node attribute extraction helpers
// ---------------------------------------------------------------------------

/// Extract the process ID from a `UiNode` by walking up to the Application
/// node and reading `ProcessId`.
///
/// Application nodes are the canonical source of PID information across all
/// providers (AT-SPI, UIA, etc.).
fn extract_pid(node: &dyn UiNode) -> Option<u32> {
    if let Some(pid) = pid_from_attr(node) {
        debug!(pid, role = node.role(), "PID found on node");
        return Some(pid);
    }
    debug!(role = node.role(), name = node.name(), "no ProcessId on node, walking parent chain");
    let mut current = node.parent()?.upgrade()?;
    loop {
        trace!(role = current.role(), ns = ?current.namespace(), name = current.name(), "checking ancestor for PID");
        if let Some(pid) = pid_from_attr(&*current) {
            debug!(pid, role = current.role(), "PID found on ancestor");
            return Some(pid);
        }
        current = current.parent()?.upgrade()?;
    }
}

/// What the node tells about its window ([`WindowHints`]): its name, and the
/// size and screen position its toolkit reports. The extents are read through
/// the raw `Component.Extents.*` native attributes, which go straight to the
/// accessibility provider and do **not** route through this window manager,
/// so this is safe to call from [`X11EwmhWindowManager::resolve_window`]
/// without recursing. A toolkit without screen extents (GTK 4 reports `(0,0)`
/// and may report no size there) still gives its size in window coordinates.
fn window_hints(node: &dyn UiNode) -> WindowHints {
    let extents = |name: &str| match node.attribute(Namespace::Native, name).map(|attr| attr.value()) {
        Some(platynui_core::ui::UiValue::Rect(rect)) if rect.width() > 0.0 && rect.height() > 0.0 => Some(rect),
        _ => None,
    };
    let screen = extents("Component.Extents.Screen");
    let size = screen.or_else(|| extents("Component.Extents.Window")).map(|rect| rect.size());
    let position = screen.map(|rect| rect.position()).filter(|p| p.x() != 0.0 || p.y() != 0.0);
    let name = node.name();
    WindowHints { name: (!name.is_empty()).then_some(name), size, position }
}

/// Try to read `control:ProcessId` from a single node. Only a positive number
/// is a process ID: `0` would match a window that declares `_NET_WM_PID` 0.
fn pid_from_attr(node: &dyn UiNode) -> Option<u32> {
    let attr = node.attribute(Namespace::Control, "ProcessId")?;
    let pid = match attr.value() {
        platynui_core::ui::UiValue::Integer(v) => u32::try_from(v).ok(),
        platynui_core::ui::UiValue::Number(v) => {
            // Saturating float-to-int: negatives and NaN become 0, rejected below.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let rounded = v as u32;
            Some(rounded)
        }
        platynui_core::ui::UiValue::String(s) => s.parse::<u32>().ok(),
        _ => None,
    };
    pid.filter(|pid| *pid > 0)
}

// ---------------------------------------------------------------------------
//  Own-window ownership
// ---------------------------------------------------------------------------
//
// The hit-test skips the runtime's own windows so the picker never resolves its
// own UI. A window says which process it belongs to through `_NET_WM_PID`, a
// number the client writes from its own `getpid()` — a number in the client's
// PID namespace — while `std::process::id()` is one in ours. Where the two
// namespaces differ, an unrelated application can carry our number.
//
// So a window counts as ours only when two witnesses agree: it reports our
// PID, and the X server attributes it to the same process as our own
// connection. The server's attribution is X-Resource's `LocalClientPID`,
// derived from each connection's socket credentials rather than reported by a
// client, and both answers are numbers in the server's namespace, so they
// compare wherever the server runs. See `sidecar-deployment` for the
// deployment in which the namespaces differ.

/// What the X server says about the process behind a connection — our own, or
/// the one that created a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServerView {
    /// The server named a process ID — `0` for a client in a PID namespace it
    /// cannot see.
    Reported(u32),
    /// The server answered, but with no process ID.
    Silent,
    /// The server could not be asked: no X-Resource 1.2, or the request failed.
    Unavailable,
}

/// How a connection's own-window check is backed, as the log states it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OwnershipMode {
    /// The server numbers us as we do.
    Verified,
    /// The server numbers us differently or not at all, so a window reporting
    /// our PID is ours only if the server attributes it to our process.
    Foreign,
    /// The server could not be asked; a window reporting our PID is skipped
    /// unverified, as before.
    Unknown,
}

fn ownership_mode(view: ServerView, own_pid: u32) -> OwnershipMode {
    match view {
        ServerView::Reported(pid) if pid != 0 && pid == own_pid => OwnershipMode::Verified,
        ServerView::Reported(_) | ServerView::Silent => OwnershipMode::Foreign,
        ServerView::Unavailable => OwnershipMode::Unknown,
    }
}

/// Whether the hit-test skips a window as the runtime's own.
///
/// The window must report our PID, and the server must attribute it to the
/// same process as our own connection (`ours`). `window_owner` asks the server
/// about the window, and is called only when the first witness holds. A server
/// that cannot be asked at all leaves the reported PID alone to decide, as
/// before; a window the server cannot name an owner for is not ours. A window
/// without a reported PID is never ours.
fn skips_as_own(
    ours: ServerView,
    own_pid: u32,
    window_pid: Option<u32>,
    window_owner: impl FnOnce() -> ServerView,
) -> bool {
    if window_pid != Some(own_pid) {
        return false;
    }
    if ours == ServerView::Unavailable {
        return true;
    }
    match window_owner() {
        ServerView::Unavailable => false,
        owner => owner == ours,
    }
}

/// The server's view from a `QueryClientIds` reply: its first `LocalClientPID`.
fn server_view_from_ids(ids: &[ClientIdValue]) -> ServerView {
    ids.iter().find_map(|id| id.value.first().copied()).map_or(ServerView::Silent, ServerView::Reported)
}

/// Ask the X server which process owns the client behind `xid`. Any XID of a
/// client's resource range names that client. Every failure is `Unavailable`,
/// never an error for the hit-test.
fn query_client_view(conn: &RustConnection, xid: u32) -> ServerView {
    let spec = ClientIdSpec { client: xid, mask: ClientIdMask::LOCAL_CLIENT_PID };
    match conn.res_query_client_ids(&[spec]).ok().and_then(|cookie| cookie.reply().ok()) {
        Some(reply) => server_view_from_ids(&reply.ids),
        None => ServerView::Unavailable,
    }
}

/// Ask the X server about our own connection, named by the base of our
/// resource range, which needs no allocation.
fn query_own_view(conn: &RustConnection) -> ServerView {
    let supports_client_ids = conn
        .res_query_version(1, 2)
        .ok()
        .and_then(|cookie| cookie.reply().ok())
        .is_some_and(|version| (version.server_major, version.server_minor) >= (1, 2));
    if !supports_client_ids {
        return ServerView::Unavailable;
    }
    query_client_view(conn, conn.setup().resource_id_base)
}

/// The server's view of our own connection, asked on first use and logged
/// once. It lives on the window manager, not beside the process-global atom
/// cache: a second runtime may connect to another display whose answer differs.
#[derive(Default)]
struct OwnershipCell(OnceLock<ServerView>);

impl OwnershipCell {
    fn get_or_decide(&self, own_pid: u32, display_name: &str, query: impl FnOnce() -> ServerView) -> ServerView {
        *self.0.get_or_init(|| {
            let view = query();
            let mode = ownership_mode(view, own_pid);
            match mode {
                OwnershipMode::Unknown => warn!(
                    display = %display_name,
                    own_pid,
                    "X11 own-window exclusion is unverified on this display: the X server cannot report \
                     which process owns a connection (X-Resource 1.2), so a window reporting our PID is \
                     skipped as before",
                ),
                OwnershipMode::Verified | OwnershipMode::Foreign => info!(
                    display = %display_name,
                    own_pid,
                    server_view = ?view,
                    ?mode,
                    "X11 own-window identification decided",
                ),
            }
            view
        })
    }
}

// ---------------------------------------------------------------------------
//  WindowManager implementation
// ---------------------------------------------------------------------------

pub struct X11EwmhWindowManager {
    conn: Arc<X11Connection>,
    /// Whether a window manager manages the display, decided when the runtime
    /// connected.
    mode: WindowMode,
    /// The X server's view of this connection's process, for the own-window check.
    ownership: OwnershipCell,
}

impl X11EwmhWindowManager {
    pub fn new(conn: Arc<X11Connection>, mode: WindowMode) -> Self {
        Self { conn, mode, ownership: OwnershipCell::default() }
    }

    /// Frontmost top-level window covering `point` on a display without a
    /// window manager: the root's children top to bottom, with the lookup's
    /// filter and the own-window check of [`Self::managed_window_at`].
    fn bare_window_at(&self, point: Point, ours: ServerView, own_pid: u32) -> Option<WindowHit> {
        let (pid_atom, frame_extents_atom) = {
            let atoms = atoms(&self.conn).ok()?;
            (atoms.net_wm_pid, atoms.gtk_frame_extents)
        };
        let windows = root_windows(&self.conn, pid_atom).ok()?;
        for window in windows.iter().rev().filter(|window| is_top_level(window)) {
            let Ok(rect) = visible_window_rect(&self.conn, window.xid, frame_extents_atom) else { continue };
            if !rect.contains(point) {
                continue;
            }
            if skips_as_own(ours, own_pid, window.pid, || query_client_view(&self.conn.conn, window.xid)) {
                continue;
            }
            return Some(WindowHit { id: WindowId::new(u64::from(window.xid)), pid: window.pid, bounds: rect });
        }
        debug!(
            ?point,
            root_children = windows.len(),
            top_levels = windows.iter().filter(|w| is_top_level(w)).count(),
            "no top-level window at the point"
        );
        None
    }

    /// Activate a window without a window manager: map it if it is unmapped
    /// and wait until it is viewable, raise it above its siblings and give it
    /// the input focus.
    fn activate_bare(&self, xid: Window) -> Result<(), PlatformError> {
        let conn = &self.conn.conn;
        let viewable = || {
            conn.get_window_attributes(xid)
                .ok()
                .and_then(|cookie| cookie.reply().ok())
                .is_some_and(|attrs| attrs.map_state == MapState::VIEWABLE)
        };
        if !viewable() {
            debug!(xid, "activate without a window manager: map the window");
            conn.map_window(xid).map_err(|e| PlatformError::OperationFailed {
                operation: "x11 map_window",
                details: Some(e.to_string()),
            })?;
            flush(conn)?;
            let deadline = Instant::now() + MAP_TIMEOUT;
            while !viewable() {
                if Instant::now() >= deadline {
                    return Err(PlatformError::OperationFailed {
                        operation: "activate X11 window",
                        details: Some(format!("the window did not become viewable within {MAP_TIMEOUT:?}")),
                    });
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        let pid_atom = atoms(&self.conn)?.net_wm_pid;
        let top_levels: Vec<Window> =
            root_windows(&self.conn, pid_atom)?.iter().filter(|window| is_top_level(window)).map(|w| w.xid).collect();
        if let Some(sibling) = raise_above(&top_levels, xid) {
            conn.configure_window(xid, &ConfigureWindowAux::new().sibling(sibling).stack_mode(StackMode::ABOVE))
                .map_err(|e| PlatformError::OperationFailed {
                    operation: "x11 configure_window (raise)",
                    details: Some(e.to_string()),
                })?;
        }
        // Giving the focus to a window that holds it already would take it away
        // from the menu it has open.
        if !self.is_active(WindowId::new(u64::from(xid)))? {
            conn.set_input_focus(InputFocus::PARENT, xid, x11rb::CURRENT_TIME).map_err(|e| {
                PlatformError::OperationFailed { operation: "x11 set_input_focus", details: Some(e.to_string()) }
            })?;
        }
        flush(conn)
    }

    /// Ask a window to close through its `WM_DELETE_WINDOW` protocol.
    fn close_bare(&self, xid: Window) -> Result<(), PlatformError> {
        let (wm_protocols, wm_delete_window) = {
            let atoms = atoms(&self.conn)?;
            (atoms.wm_protocols, atoms.wm_delete_window)
        };
        let conn = &self.conn.conn;
        let protocols: Vec<Atom> = conn
            .get_property(false, xid, wm_protocols, AtomEnum::ATOM, 0, 32)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .and_then(|reply| reply.value32().map(Iterator::collect))
            .unwrap_or_default();
        check_delete_protocol(&protocols, wm_delete_window)?;
        let event = ClientMessageEvent::new(32, xid, wm_protocols, [wm_delete_window, x11rb::CURRENT_TIME, 0, 0, 0]);
        conn.send_event(false, xid, EventMask::NO_EVENT, event).map_err(|e| PlatformError::OperationFailed {
            operation: "x11 send_event",
            details: Some(e.to_string()),
        })?;
        flush(conn)
    }

    /// Topmost viewable override-redirect *popup* (menu/combo/tooltip) covering
    /// `point`, from the raw root window tree. `fallback_pid` supplies the owning
    /// process when the popup has no `_NET_WM_PID` (common for menus).
    fn popup_window_at(
        &self,
        point: Point,
        pid_atom: Atom,
        ours: ServerView,
        own_pid: u32,
        fallback_pid: Option<u32>,
    ) -> Option<WindowHit> {
        let conn = &self.conn.conn;
        let tree = conn.query_tree(self.conn.root).ok()?.reply().ok()?;
        let type_atom = intern(conn, b"_NET_WM_WINDOW_TYPE").ok()?;
        // `children` is bottom-to-top; probe topmost-first.
        for &win in tree.children.iter().rev() {
            let Some(attrs) = conn.get_window_attributes(win).ok().and_then(|cookie| cookie.reply().ok()) else {
                continue;
            };
            if attrs.map_state != MapState::VIEWABLE || !attrs.override_redirect {
                continue;
            }
            if !self.is_popup_type(win, type_atom) {
                continue;
            }
            let Ok(rect) = client_rect(&self.conn, win) else { continue };
            if !rect.contains(point) {
                continue;
            }
            let pid = get_window_pid(conn, win, pid_atom).or(fallback_pid);
            if skips_as_own(ours, own_pid, pid, || query_client_view(conn, win)) {
                continue;
            }
            return Some(WindowHit { id: WindowId::new(u64::from(win)), pid, bounds: rect });
        }
        None
    }

    /// Whether `win`'s `_NET_WM_WINDOW_TYPE` marks it as a menu/popup surface
    /// (dropdown/popup menu, combo, tooltip) — distinguishing real popups from
    /// other override-redirect windows (backing stores, panels, the pointer),
    /// which carry no such type.
    fn is_popup_type(&self, win: Window, type_atom: Atom) -> bool {
        let conn = &self.conn.conn;
        let Some(reply) =
            conn.get_property(false, win, type_atom, AtomEnum::ATOM, 0, 16).ok().and_then(|c| c.reply().ok())
        else {
            return false;
        };
        let Some(atoms) = reply.value32() else {
            return false;
        };
        for atom in atoms {
            if let Some(name) = conn.get_atom_name(atom).ok().and_then(|c| c.reply().ok()) {
                let name = String::from_utf8_lossy(&name.name);
                if name.contains("_MENU")
                    || name.contains("POPUP")
                    || name.contains("COMBO")
                    || name.contains("TOOLTIP")
                {
                    return true;
                }
            }
        }
        false
    }

    /// Frontmost managed (WM-reparented) window covering `point`, from
    /// `_NET_CLIENT_LIST_STACKING` (bottom-to-top → probe topmost-first),
    /// skipping a window this connection identifies as ours so the picker never
    /// resolves its own UI.
    fn managed_window_at(
        &self,
        point: Point,
        stacking_atom: Atom,
        pid_atom: Atom,
        ours: ServerView,
        own_pid: u32,
    ) -> Option<WindowHit> {
        let stacking = get_client_list(&self.conn.conn, self.conn.root, stacking_atom).ok()?;
        let frame_extents_atom = atoms(&self.conn).ok()?.gtk_frame_extents;
        for &win in stacking.iter().rev() {
            let Ok(rect) = visible_window_rect(&self.conn, win, frame_extents_atom) else { continue };
            if !rect.contains(point) {
                continue;
            }
            let pid = get_window_pid(&self.conn.conn, win, pid_atom);
            if skips_as_own(ours, own_pid, pid, || query_client_view(&self.conn.conn, win)) {
                continue;
            }
            return Some(WindowHit { id: WindowId::new(u64::from(win)), pid, bounds: rect });
        }
        None
    }
}

impl WindowManager for X11EwmhWindowManager {
    fn name(&self) -> &'static str {
        "X11 EWMH"
    }

    fn resolve_window(&self, node: &dyn UiNode) -> Result<WindowId, PlatformError> {
        let pid = extract_pid(node)
            .ok_or(PlatformError::OperationFailed { operation: "extract PID from UiNode", details: None })?;
        let xid = find_xid_for_pid(&self.conn, pid, &window_hints(node), self.mode)?;
        trace!(pid, xid, "resolved WindowId");
        Ok(WindowId::new(u64::from(xid)))
    }

    fn toolkit_screen_coordinates_are_desktop(&self) -> bool {
        screen_coordinates_are_desktop(self.mode)
    }

    /// The surface is the client window; its visible part leaves out the
    /// shadow GTK draws inside it under a compositing window manager
    /// (`_GTK_FRAME_EXTENTS`).
    fn geometry(&self, id: WindowId) -> Result<WindowGeometry, PlatformError> {
        let xid = xid_of(id);
        let client = client_rect(&self.conn, xid)?;
        let frame_extents = read_frame_extents(&self.conn.conn, xid, atoms(&self.conn)?.gtk_frame_extents);
        Ok(WindowGeometry { bounds: visible_rect(client, frame_extents), surface_origin: client.position() })
    }

    fn window_at_point(&self, point: Point) -> Result<Option<WindowHit>, PlatformError> {
        let (stacking_atom, pid_atom) = {
            let atoms = atoms(&self.conn)?;
            (atoms.net_client_list_stacking, atoms.net_wm_pid)
        };
        let own_pid = std::process::id();
        let ours = self.ownership.get_or_decide(own_pid, &self.conn.display, || query_own_view(&self.conn.conn));

        // The frontmost top-level window at the point: from EWMH stacking, or
        // from the root's own stacking order without a window manager.
        let managed = match self.mode {
            WindowMode::Managed => self.managed_window_at(point, stacking_atom, pid_atom, ours, own_pid),
            WindowMode::Bare => self.bare_window_at(point, ours, own_pid),
        };

        // Override-redirect popups (menus, combo dropdowns, tooltips) bypass the
        // window manager and are absent from `_NET_CLIENT_LIST_STACKING`, yet
        // stack above managed windows — so a menu must win over the window it
        // popped from. They are filtered by `_NET_WM_WINDOW_TYPE` (menu/popup/
        // combo/tooltip) so unrelated override-redirect windows (backing stores,
        // panels, the pointer) are not mistaken for popups. Such popups often
        // carry no `_NET_WM_PID`, so fall back to the managed window's pid — the
        // menu belongs to that application.
        if let Some(hit) =
            self.popup_window_at(point, pid_atom, ours, own_pid, managed.as_ref().and_then(|managed| managed.pid))
        {
            trace!(xid = hit.id.raw(), ?point, "window_at_point resolved (popup)");
            return Ok(Some(hit));
        }

        Ok(managed)
    }

    /// The process's popup windows from the root's children. Popups are
    /// override-redirect windows, so the X server lists them with and without
    /// a window manager.
    fn popups(&self, pid: u32) -> Result<Vec<PopupGeometry>, PlatformError> {
        let (pid_atom, frame_extents_atom) = {
            let atoms = atoms(&self.conn)?;
            (atoms.net_wm_pid, atoms.gtk_frame_extents)
        };
        let conn = &self.conn.conn;
        let tree = conn
            .query_tree(self.conn.root)
            .map_err(|e| PlatformError::OperationFailed { operation: "x11 query_tree", details: Some(e.to_string()) })?
            .reply()
            .map_err(|e| PlatformError::OperationFailed {
                operation: "x11 query_tree reply",
                details: Some(e.to_string()),
            })?;
        let type_atom = intern(conn, b"_NET_WM_WINDOW_TYPE")?;
        let mut children = Vec::new();
        for &win in &tree.children {
            let Some(attrs) = conn.get_window_attributes(win).ok().and_then(|cookie| cookie.reply().ok()) else {
                continue;
            };
            // Only override-redirect windows can be popups; reading the rest
            // of every managed frame would cost a round trip each.
            if attrs.map_state != MapState::VIEWABLE || !attrs.override_redirect {
                continue;
            }
            let Ok(client) = client_rect(&self.conn, win) else { continue };
            children.push(RootChild {
                viewable: true,
                override_redirect: true,
                popup_type: self.is_popup_type(win, type_atom),
                pid: get_window_pid(conn, win, pid_atom),
                client,
                frame_extents: read_frame_extents(conn, win, frame_extents_atom),
            });
        }
        Ok(popups_of(&children, pid))
    }

    fn is_active(&self, id: WindowId) -> Result<bool, PlatformError> {
        let xid = xid_of(id);
        if self.mode == WindowMode::Bare {
            // Without a window manager, a window is active while it holds the
            // input focus, or one of its descendants does.
            let conn = &self.conn.conn;
            let focus = conn
                .get_input_focus()
                .map_err(|e| PlatformError::OperationFailed {
                    operation: "x11 get_input_focus",
                    details: Some(e.to_string()),
                })?
                .reply()
                .map_err(|e| PlatformError::OperationFailed {
                    operation: "x11 get_input_focus reply",
                    details: Some(e.to_string()),
                })?
                .focus;
            let parent_of = |win| conn.query_tree(win).ok()?.reply().ok().map(|tree| tree.parent);
            return Ok(focused_top_level(focus, self.conn.root, parent_of) == Some(xid));
        }
        let atoms = atoms(&self.conn)?;
        let x11 = &self.conn;
        let reply = x11
            .conn
            .get_property(false, x11.root, atoms.net_active_window, AtomEnum::WINDOW, 0, 1)
            .map_err(|e| PlatformError::OperationFailed {
                operation: "read _NET_ACTIVE_WINDOW",
                details: Some(e.to_string()),
            })?
            .reply()
            .map_err(|e| PlatformError::OperationFailed {
                operation: "read _NET_ACTIVE_WINDOW reply",
                details: Some(e.to_string()),
            })?;
        let active_xid = reply.value32().and_then(|mut iter| iter.next()).unwrap_or(0);
        Ok(active_xid == xid)
    }

    fn state(&self, id: WindowId) -> Result<WindowState, PlatformError> {
        if self.mode == WindowMode::Bare {
            // Nothing minimizes, maximizes or keeps a window on top without a
            // window manager.
            return Ok(WindowState { visual: WindowVisualState::Normal, topmost: false });
        }
        let atoms = atoms(&self.conn)?;
        read_window_state(&self.conn, xid_of(id), &atoms)
    }

    fn activate(&self, id: WindowId) -> Result<(), PlatformError> {
        let xid = xid_of(id);
        if self.mode == WindowMode::Bare {
            return self.activate_bare(xid);
        }
        debug!(xid, "EWMH activate");
        let atoms = atoms(&self.conn)?;
        let x11 = &self.conn;
        // EWMH does not require a window manager to de-iconify on _NET_ACTIVE_WINDOW.
        // ICCCM §4.1.4 does: a client leaves Iconic state by mapping its window. The
        // window manager receives the MapRequest and keeps _NET_WM_STATE untouched,
        // so a window minimized while maximized comes back maximized.
        if read_window_state(x11, xid, &atoms).is_ok_and(|state| state.is_minimized()) {
            debug!(xid, "EWMH activate: de-iconify via MapWindow");
            x11.conn.map_window(xid).map_err(|e| PlatformError::OperationFailed {
                operation: "x11 map_window",
                details: Some(e.to_string()),
            })?;
        }
        send_client_message(&x11.conn, x11.root, xid, atoms.net_active_window, [2, 0, 0, 0, 0])?;
        flush(&x11.conn)
    }

    fn close(&self, id: WindowId) -> Result<(), PlatformError> {
        let xid = xid_of(id);
        if self.mode == WindowMode::Bare {
            return self.close_bare(xid);
        }
        debug!(xid, "EWMH close");
        let atoms = atoms(&self.conn)?;
        let x11 = &self.conn;
        send_client_message(&x11.conn, x11.root, xid, atoms.net_close_window, [0, 2, 0, 0, 0])?;
        flush(&x11.conn)
    }

    fn minimize(&self, id: WindowId) -> Result<(), PlatformError> {
        if self.mode == WindowMode::Bare {
            return Err(needs_window_manager("minimize"));
        }
        let xid = xid_of(id);
        debug!(xid, "EWMH minimize (iconify)");
        let x11 = &self.conn;
        // XIconifyWindow equivalent: use ClientMessage WM_CHANGE_STATE with IconicState.
        let wm_change_state = intern(&x11.conn, b"WM_CHANGE_STATE")?;
        send_client_message(&x11.conn, x11.root, xid, wm_change_state, [3 /* IconicState */, 0, 0, 0, 0])?;
        flush(&x11.conn)
    }

    fn maximize(&self, id: WindowId) -> Result<(), PlatformError> {
        if self.mode == WindowMode::Bare {
            return Err(needs_window_manager("maximize"));
        }
        let xid = xid_of(id);
        debug!(xid, "EWMH maximize");
        let atoms = atoms(&self.conn)?;
        let x11 = &self.conn;
        // _NET_WM_STATE add _NET_WM_STATE_MAXIMIZED_VERT + _NET_WM_STATE_MAXIMIZED_HORZ
        send_client_message(
            &x11.conn,
            x11.root,
            xid,
            atoms.net_wm_state,
            [
                1, // _NET_WM_STATE_ADD
                atoms.net_wm_state_maximized_vert,
                atoms.net_wm_state_maximized_horz,
                2, // source: pager/automation
                0,
            ],
        )?;
        flush(&x11.conn)
    }

    fn restore(&self, id: WindowId) -> Result<(), PlatformError> {
        if self.mode == WindowMode::Bare {
            return Err(needs_window_manager("restore"));
        }
        let xid = xid_of(id);
        debug!(xid, "EWMH restore");
        let atoms = atoms(&self.conn)?;
        let x11 = &self.conn;
        // Remove maximised state.
        send_client_message(
            &x11.conn,
            x11.root,
            xid,
            atoms.net_wm_state,
            [
                0, // _NET_WM_STATE_REMOVE
                atoms.net_wm_state_maximized_vert,
                atoms.net_wm_state_maximized_horz,
                2,
                0,
            ],
        )?;
        // Remove hidden state (de-iconify).
        send_client_message(&x11.conn, x11.root, xid, atoms.net_wm_state, [0, atoms.net_wm_state_hidden, 0, 2, 0])?;
        // Additionally activate the window so it comes to the foreground.
        send_client_message(&x11.conn, x11.root, xid, atoms.net_active_window, [2, 0, 0, 0, 0])?;
        flush(&x11.conn)
    }

    fn move_to(&self, id: WindowId, position: Point) -> Result<(), PlatformError> {
        let xid = xid_of(id);
        debug!(xid, x = position.x(), y = position.y(), "EWMH move_to");
        let x11 = &self.conn;
        // X11 positions are i32; saturating float-to-int is the intended conversion.
        #[allow(clippy::cast_possible_truncation)]
        let aux = ConfigureWindowAux::new().x(position.x() as i32).y(position.y() as i32);
        x11.conn.configure_window(xid, &aux).map_err(|e| PlatformError::OperationFailed {
            operation: "x11 configure_window move",
            details: Some(e.to_string()),
        })?;
        flush(&x11.conn)
    }

    fn resize(&self, id: WindowId, size: Size) -> Result<(), PlatformError> {
        let xid = xid_of(id);
        debug!(xid, w = size.width(), h = size.height(), "EWMH resize");
        let x11 = &self.conn;
        // X11 sizes are u32; saturating float-to-int is the intended conversion.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let aux = ConfigureWindowAux::new().width(size.width() as u32).height(size.height() as u32);
        x11.conn.configure_window(xid, &aux).map_err(|e| PlatformError::OperationFailed {
            operation: "x11 configure_window resize",
            details: Some(e.to_string()),
        })?;
        flush(&x11.conn)
    }
}

// ---------------------------------------------------------------------------
//  EWMH WM detection — called from the platform bundle factory
// ---------------------------------------------------------------------------

/// Check whether an EWMH-compatible window manager is running and log the
/// result.  Returns `Ok(true)` when a WM was detected, `Ok(false)` when the
/// check cannot confirm WM presence (non-fatal).
///
/// Called once from [`crate::create_x11_bundle`] on the runtime's connection.
///
/// # Errors
///
/// Returns [`PlatformError::OperationFailed`] when the EWMH atoms cannot be
/// interned (or their cache lock is poisoned), or when the root window's
/// `_NET_SUPPORTING_WM_CHECK` property cannot be requested or read.
pub fn check_ewmh_wm_support(x11: &X11Connection) -> Result<bool, PlatformError> {
    let atoms = atoms(x11)?;

    // 1. _NET_SUPPORTING_WM_CHECK on root → child window
    let child_reply = x11
        .conn
        .get_property(false, x11.root, atoms.net_supporting_wm_check, AtomEnum::WINDOW, 0, 1)
        .map_err(|e| PlatformError::OperationFailed {
            operation: "read _NET_SUPPORTING_WM_CHECK",
            details: Some(e.to_string()),
        })?
        .reply()
        .map_err(|e| PlatformError::OperationFailed {
            operation: "read _NET_SUPPORTING_WM_CHECK reply",
            details: Some(e.to_string()),
        })?;

    let Some(child_xid) = child_reply.value32().and_then(|mut iter| iter.next()) else {
        warn!("no EWMH-compatible window manager detected (_NET_SUPPORTING_WM_CHECK missing)");
        return Ok(false);
    };

    // 2. Consistency check: the child window must also point back to itself.
    let verify_reply = x11
        .conn
        .get_property(false, child_xid, atoms.net_supporting_wm_check, AtomEnum::WINDOW, 0, 1)
        .ok()
        .and_then(|c| c.reply().ok());
    let consistent = verify_reply.and_then(|r| r.value32().and_then(|mut iter| iter.next())) == Some(child_xid);
    if !consistent {
        warn!(child_xid, "EWMH _NET_SUPPORTING_WM_CHECK consistency check failed");
        return Ok(false);
    }

    // 3. Read WM name from the child window.
    let name_reply = x11
        .conn
        .get_property(false, child_xid, atoms.net_wm_name, atoms.utf8_string, 0, 1024)
        .ok()
        .and_then(|c| c.reply().ok());
    let wm_name = name_reply
        .and_then(|r| {
            let bytes = r.value;
            if bytes.is_empty() { None } else { String::from_utf8(bytes).ok() }
        })
        .unwrap_or_else(|| "<unknown>".to_string());
    tracing::info!(wm = %wm_name, "EWMH window manager detected");

    // 4. Check which atoms are supported.
    let supported_reply = x11
        .conn
        .get_property(false, x11.root, atoms.net_supported, AtomEnum::ATOM, 0, u32::MAX)
        .ok()
        .and_then(|c| c.reply().ok());
    let supported_set: Vec<Atom> = supported_reply.and_then(|r| r.value32().map(Iterator::collect)).unwrap_or_default();

    let required = [
        ("_NET_CLIENT_LIST", atoms.net_client_list),
        ("_NET_ACTIVE_WINDOW", atoms.net_active_window),
        ("_NET_CLOSE_WINDOW", atoms.net_close_window),
        ("_NET_WM_PID", atoms.net_wm_pid),
    ];
    for (name, atom) in required {
        if !supported_set.contains(&atom) {
            warn!(atom_name = name, "EWMH atom not listed in _NET_SUPPORTED — window operations may fail");
        }
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    mod process_id {
        use super::{extract_pid, pid_from_attr};
        use platynui_core::ui::{Namespace, PatternName, RuntimeId, UiAttribute, UiNode, UiValue};
        use std::sync::{Arc, Weak};

        /// A node that carries nothing but `control:ProcessId`, below an
        /// optional parent.
        struct PidNode {
            pid: Option<UiValue>,
            parent: Option<Weak<dyn UiNode>>,
            runtime_id: RuntimeId,
        }

        fn pid_node(pid: Option<UiValue>, parent: Option<&Arc<dyn UiNode>>) -> Arc<dyn UiNode> {
            Arc::new(PidNode { pid, parent: parent.map(Arc::downgrade), runtime_id: RuntimeId::from("pid-node") })
        }

        struct PidAttr(UiValue);

        impl UiAttribute for PidAttr {
            fn namespace(&self) -> Namespace {
                Namespace::Control
            }
            fn name(&self) -> &'static str {
                "ProcessId"
            }
            fn value(&self) -> UiValue {
                self.0.clone()
            }
        }

        impl UiNode for PidNode {
            fn namespace(&self) -> Namespace {
                Namespace::App
            }
            fn role(&self) -> &'static str {
                "Application"
            }
            fn name(&self) -> String {
                String::new()
            }
            fn runtime_id(&self) -> &RuntimeId {
                &self.runtime_id
            }
            fn parent(&self) -> Option<Weak<dyn UiNode>> {
                self.parent.clone()
            }
            fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
                Box::new(std::iter::empty())
            }
            fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
                let attributes: Vec<Arc<dyn UiAttribute>> =
                    self.pid.iter().map(|pid| Arc::new(PidAttr(pid.clone())) as _).collect();
                Box::new(attributes.into_iter())
            }
            fn supported_patterns(&self) -> Vec<PatternName> {
                Vec::new()
            }
            fn invalidate(&self) {}
        }

        /// Spec (`application-process-attributes`): *A window is never looked
        /// up by process ID 0*, in any form a provider may report it.
        #[test]
        fn process_id_0_counts_as_no_process_id() {
            for zero in [UiValue::Integer(0), UiValue::Number(0.0), UiValue::String("0".into())] {
                assert_eq!(pid_from_attr(pid_node(Some(zero.clone()), None).as_ref()), None, "{zero:?}");
            }
            assert_eq!(pid_from_attr(pid_node(Some(UiValue::Integer(4711)), None).as_ref()), Some(4711));
        }

        #[test]
        fn a_node_with_process_id_0_takes_the_process_id_of_its_ancestors() {
            let application = pid_node(Some(UiValue::Integer(4711)), None);
            let window = pid_node(Some(UiValue::Integer(0)), Some(&application));
            assert_eq!(extract_pid(window.as_ref()), Some(4711));
        }
    }

    mod window_choice {
        use super::super::{CandidateWindow, FrameExtents, WindowHints, choose_window, visible_rect};
        use platynui_core::types::{Point, Rect, Size};

        fn candidate(xid: u32, title: &str, rect: Rect) -> CandidateWindow {
            CandidateWindow { xid, title: Some(title.into()), rect }
        }

        fn hints(name: &str, size: Option<Size>, position: Option<Point>) -> WindowHints {
            WindowHints { name: (!name.is_empty()).then(|| name.into()), size, position }
        }

        /// A GTK 4 main window and its modeless dialog, as on the X11 lane.
        fn gtk4_windows() -> Vec<CandidateWindow> {
            vec![
                candidate(0x40_0001, "PlatynUI GTK4 TestApp", Rect::new(0.0, 0.0, 900.0, 760.0)),
                candidate(0x40_0002, "dialog-modeless", Rect::new(905.0, 24.0, 420.0, 300.0)),
            ]
        }

        #[test]
        fn frame_extents_are_taken_off_the_client_rectangle() {
            let extents = FrameExtents { left: 23, right: 23, top: 21, bottom: 25 };
            assert_eq!(
                visible_rect(Rect::new(200.0, 100.0, 210.0, 130.0), Some(extents)),
                Rect::new(223.0, 121.0, 164.0, 84.0)
            );
        }

        #[test]
        fn without_frame_extents_the_client_rectangle_is_visible() {
            let client = Rect::new(200.0, 100.0, 210.0, 130.0);
            assert_eq!(visible_rect(client, None), client);
        }

        #[test]
        fn the_exact_title_decides_first() {
            // The node's size points at the main window; its title wins.
            let node = hints("dialog-modeless", Some(Size::new(900.0, 760.0)), None);
            assert_eq!(choose_window(&gtk4_windows(), &node, 4242).unwrap(), 0x40_0002);
        }

        #[test]
        fn a_title_that_only_contains_the_name_does_not_count() {
            let windows = vec![
                candidate(1, "Catalog - Extra", Rect::new(0.0, 0.0, 640.0, 480.0)),
                candidate(2, "Other", Rect::new(700.0, 0.0, 300.0, 200.0)),
            ];
            let node = hints("Catalog", Some(Size::new(300.0, 200.0)), None);
            assert_eq!(choose_window(&windows, &node, 4242).unwrap(), 2);
        }

        #[test]
        fn two_gtk4_windows_of_one_process_are_resolved_by_size() {
            // GTK 4 reports (0,0) as every screen position and names nothing
            // the lookup can match here; the sizes differ.
            let windows = gtk4_windows();
            assert_eq!(
                choose_window(&windows, &hints("", Some(Size::new(420.0, 300.0)), None), 4242).unwrap(),
                0x40_0002
            );
            assert_eq!(
                choose_window(&windows, &hints("", Some(Size::new(900.0, 760.0)), None), 4242).unwrap(),
                0x40_0001
            );
        }

        #[test]
        fn equal_sizes_are_resolved_by_the_screen_position_the_toolkit_reports() {
            let windows = vec![
                candidate(1, "Untitled", Rect::new(10.0, 10.0, 420.0, 300.0)),
                candidate(2, "Untitled", Rect::new(500.0, 10.0, 420.0, 300.0)),
            ];
            let node = hints("Document", Some(Size::new(420.0, 300.0)), Some(Point::new(500.0, 10.0)));
            assert_eq!(choose_window(&windows, &node, 4242).unwrap(), 2);
        }

        #[test]
        fn two_indistinguishable_windows_fail_the_lookup() {
            // Equal title and size, and no screen position: a window at the
            // origin must not be taken for the node.
            let windows = vec![
                candidate(1, "Untitled", Rect::new(0.0, 0.0, 420.0, 300.0)),
                candidate(2, "Untitled", Rect::new(500.0, 10.0, 420.0, 300.0)),
            ];
            let node = hints("Untitled", Some(Size::new(420.0, 300.0)), None);
            let error = choose_window(&windows, &node, 4242).unwrap_err().to_string();
            assert!(error.contains("2 candidate windows"), "{error}");
        }
    }

    mod popup_list {
        use super::super::{FrameExtents, RootChild, popups_of};
        use platynui_core::platform::PopupGeometry;
        use platynui_core::types::Rect;

        /// A viewable override-redirect root child, as a menu window is.
        fn child(pid: Option<u32>, popup_type: bool, client: Rect) -> RootChild {
            RootChild { viewable: true, override_redirect: true, popup_type, pid, client, frame_extents: None }
        }

        #[test]
        fn popups_of_other_processes_and_windows_without_a_process_are_not_listed() {
            let ours = Rect::new(10.0, 10.0, 100.0, 50.0);
            // Bottom to top, as the X server lists the root's children.
            let children = vec![
                child(Some(7), true, Rect::new(0.0, 0.0, 80.0, 40.0)),
                child(None, true, Rect::new(0.0, 0.0, 80.0, 40.0)),
                child(Some(4242), false, Rect::new(0.0, 0.0, 80.0, 40.0)),
                RootChild { viewable: false, ..child(Some(4242), true, Rect::new(0.0, 0.0, 80.0, 40.0)) },
                RootChild { override_redirect: false, ..child(Some(4242), true, Rect::new(0.0, 0.0, 900.0, 700.0)) },
                child(Some(4242), true, ours),
            ];
            assert_eq!(popups_of(&children, 4242), vec![PopupGeometry::from_bounds(ours)]);
        }

        #[test]
        fn popups_are_listed_topmost_first() {
            let menu = Rect::new(90.0, 40.0, 165.0, 108.0);
            let submenu = Rect::new(250.0, 120.0, 162.0, 82.0);
            let children = vec![child(Some(4242), true, menu), child(Some(4242), true, submenu)];
            assert_eq!(
                popups_of(&children, 4242),
                vec![PopupGeometry::from_bounds(submenu), PopupGeometry::from_bounds(menu)]
            );
        }

        #[test]
        fn a_popup_window_with_frame_extents_has_a_smaller_visible_rectangle() {
            let client = Rect::new(200.0, 100.0, 210.0, 130.0);
            let shadowed = RootChild {
                frame_extents: Some(FrameExtents { left: 23, right: 23, top: 21, bottom: 25 }),
                ..child(Some(4242), true, client)
            };
            assert_eq!(
                popups_of(&[shadowed], 4242),
                vec![PopupGeometry { bounds: Rect::new(223.0, 121.0, 164.0, 84.0), surface: client }]
            );
        }
    }

    mod bare_mode {
        use super::super::{
            RootWindow, WindowMode, application_windows, check_delete_protocol, focused_top_level, raise_above,
            screen_coordinates_are_desktop, window_mode,
        };
        use platynui_core::platform::PlatformError;

        const ROOT: u32 = 0x100;

        fn window(xid: u32, pid: Option<u32>, viewable: bool, override_redirect: bool) -> RootWindow {
            RootWindow { xid, viewable, override_redirect, pid }
        }

        #[test]
        fn a_runtime_created_after_a_window_manager_started_uses_it() {
            assert_eq!(window_mode(&Ok(false)), WindowMode::Bare);
            assert_eq!(window_mode(&Ok(true)), WindowMode::Managed);
        }

        /// Without a window manager a client is not told where its window was
        /// moved, so its screen positions may count from where it opened.
        #[test]
        fn screen_coordinates_are_desktop_coordinates_only_with_a_window_manager() {
            assert!(screen_coordinates_are_desktop(WindowMode::Managed));
            assert!(!screen_coordinates_are_desktop(WindowMode::Bare));
        }

        #[test]
        fn a_check_that_cannot_answer_keeps_the_window_manager_path() {
            let failed =
                Err(PlatformError::OperationFailed { operation: "read _NET_SUPPORTING_WM_CHECK", details: None });
            assert_eq!(window_mode(&failed), WindowMode::Managed);
        }

        #[test]
        fn a_popup_is_not_taken_for_the_applications_window() {
            let children = vec![
                window(1, Some(7), true, false),     // another process
                window(2, Some(4242), false, false), // unmapped
                window(3, Some(4242), true, true),   // its open menu
                window(4, None, true, false),        // no _NET_WM_PID
                window(5, Some(4242), true, false),  // its window
            ];
            let found: Vec<u32> = application_windows(&children, 4242).iter().map(|c| c.xid).collect();
            assert_eq!(found, vec![5]);
        }

        /// A tree of root → 0x10 → 0x11 → 0x12 and root → 0x20.
        fn parent_of(window: u32) -> Option<u32> {
            match window {
                0x10 | 0x20 => Some(ROOT),
                0x11 => Some(0x10),
                0x12 => Some(0x11),
                _ => None,
            }
        }

        #[test]
        fn focus_inside_a_window_makes_it_active() {
            assert_eq!(focused_top_level(0x12, ROOT, parent_of), Some(0x10));
            assert_eq!(focused_top_level(0x20, ROOT, parent_of), Some(0x20));
        }

        #[test]
        fn no_focused_window_means_no_active_window() {
            // PointerRoot (1), None (0) and the root itself focus no window.
            for focus in [1, 0, ROOT] {
                assert_eq!(focused_top_level(focus, ROOT, parent_of), None, "focus {focus:#x}");
            }
        }

        /// The top-level windows bottom to top; override-redirect popups above
        /// them are not among them and stay above the raised window.
        #[test]
        fn activation_raises_a_window_just_above_the_topmost_other_one() {
            assert_eq!(raise_above(&[0x10, 0x20, 0x30], 0x10), Some(0x30));
            assert_eq!(raise_above(&[0x10, 0x20, 0x30], 0x30), None, "the topmost window stays where it is");
            assert_eq!(raise_above(&[], 0x10), None);
        }

        #[test]
        fn a_window_without_the_delete_protocol_fails_by_name() {
            const WM_DELETE_WINDOW: u32 = 0x55;
            assert!(check_delete_protocol(&[0x51, WM_DELETE_WINDOW], WM_DELETE_WINDOW).is_ok());
            let error = check_delete_protocol(&[0x51], WM_DELETE_WINDOW).unwrap_err().to_string();
            assert!(error.contains("WM_DELETE_WINDOW"), "{error}");
        }
    }

    fn test_atoms() -> EwmhAtoms {
        EwmhAtoms {
            net_client_list: 1,
            net_client_list_stacking: 2,
            net_wm_pid: 3,
            net_active_window: 4,
            net_close_window: 5,
            net_wm_state: 6,
            net_wm_state_maximized_vert: 7,
            net_wm_state_maximized_horz: 8,
            net_wm_state_hidden: 9,
            net_wm_state_above: 10,
            wm_state: 11,
            net_supporting_wm_check: 12,
            net_supported: 13,
            net_wm_name: 14,
            utf8_string: 15,
            gtk_frame_extents: 16,
            wm_protocols: 17,
            wm_delete_window: 18,
        }
    }

    #[test]
    fn decode_window_state_reports_maximized_only_for_both_axes() {
        let atoms = test_atoms();
        let both =
            decode_window_state(&[atoms.net_wm_state_maximized_vert, atoms.net_wm_state_maximized_horz], false, &atoms);
        assert_eq!(both, WindowState { visual: WindowVisualState::Maximized, topmost: false });
        let vertical = decode_window_state(&[atoms.net_wm_state_maximized_vert], false, &atoms);
        assert_eq!(vertical.visual, WindowVisualState::Normal);
    }

    #[test]
    fn decode_window_state_reports_minimized_over_maximized() {
        let atoms = test_atoms();
        let maximized = [atoms.net_wm_state_maximized_vert, atoms.net_wm_state_maximized_horz];
        let hidden = [maximized[0], maximized[1], atoms.net_wm_state_hidden];
        assert_eq!(decode_window_state(&hidden, false, &atoms).visual, WindowVisualState::Minimized);
        assert_eq!(decode_window_state(&maximized, true, &atoms).visual, WindowVisualState::Minimized);
    }

    #[test]
    fn decode_window_state_reports_topmost_from_above() {
        let atoms = test_atoms();
        assert!(decode_window_state(&[atoms.net_wm_state_above], false, &atoms).topmost);
        assert!(!decode_window_state(&[], false, &atoms).topmost);
    }

    // ── Own-window ownership (spec: Hit-test excludes the host process's own UI) ──

    const OURS: u32 = 4242;
    const SOMEBODY_ELSE: u32 = 7;
    /// The runtime's PID as a server in an ancestor namespace numbers it.
    const OURS_ON_THE_HOST: u32 = 90_001;
    /// An application's PID as the server numbers it.
    const APP_ON_THE_SERVER: u32 = 50;

    fn skips(ours: ServerView, window_pid: Option<u32>, owner: ServerView) -> bool {
        skips_as_own(ours, OURS, window_pid, || owner)
    }

    #[test]
    fn an_ordinary_desktop_skips_only_the_runtimes_own_window() {
        let ours = ServerView::Reported(OURS);
        assert!(skips(ours, Some(OURS), ServerView::Reported(OURS)));
        assert!(!skips(ours, Some(SOMEBODY_ELSE), ServerView::Reported(SOMEBODY_ELSE)));
        // Another process's window that claims our number.
        assert!(!skips(ours, Some(OURS), ServerView::Reported(SOMEBODY_ELSE)));
    }

    #[test]
    fn an_application_reusing_our_pid_is_resolved_when_the_server_attributes_it_elsewhere() {
        // The sidecar: the server cannot see us, but sees the application.
        assert!(!skips(ServerView::Reported(0), Some(OURS), ServerView::Reported(APP_ON_THE_SERVER)));
        // A child namespace of the server's: an application on the host reuses
        // our in-namespace number.
        assert!(!skips(ServerView::Reported(OURS_ON_THE_HOST), Some(OURS), ServerView::Reported(APP_ON_THE_SERVER)));
        // A window reporting another number, wherever the server runs.
        for ours in [ServerView::Reported(0), ServerView::Reported(OURS_ON_THE_HOST), ServerView::Silent] {
            assert!(!skips(ours, Some(SOMEBODY_ELSE), ours), "{ours:?}");
        }
    }

    #[test]
    fn our_own_window_is_skipped_where_the_server_numbers_us_differently() {
        // A server that sees neither us nor our window (WSLg, a sibling namespace).
        assert!(skips(ServerView::Reported(0), Some(OURS), ServerView::Reported(0)));
        // A server in an ancestor namespace (a container on the host's display).
        assert!(skips(ServerView::Reported(OURS_ON_THE_HOST), Some(OURS), ServerView::Reported(OURS_ON_THE_HOST)));
        // A server that reports no process for any client (a TCP connection).
        assert!(skips(ServerView::Silent, Some(OURS), ServerView::Silent));
    }

    #[test]
    fn a_server_that_cannot_be_asked_keeps_the_previous_comparison() {
        let never =
            || -> ServerView { panic!("the server is not asked about a window when it cannot be asked at all") };
        assert!(skips_as_own(ServerView::Unavailable, OURS, Some(OURS), never));
        assert!(!skips_as_own(ServerView::Unavailable, OURS, Some(SOMEBODY_ELSE), never));
    }

    #[test]
    fn a_window_whose_owner_cannot_be_named_is_not_ours() {
        // The second witness is missing, and nothing is excluded on a guess.
        assert!(!skips(ServerView::Reported(0), Some(OURS), ServerView::Unavailable));
        assert!(!skips(ServerView::Reported(OURS), Some(OURS), ServerView::Unavailable));
    }

    #[test]
    fn a_window_without_a_reported_pid_is_never_skipped() {
        // Override-redirect popups often carry no _NET_WM_PID; "no number" is
        // never a match, whatever the server says.
        for ours in [ServerView::Reported(OURS), ServerView::Reported(0), ServerView::Silent, ServerView::Unavailable] {
            assert!(!skips(ours, None, ours), "{ours:?}");
        }
    }

    #[test]
    fn the_server_is_asked_about_a_window_only_when_it_reports_our_pid() {
        let asked = std::cell::Cell::new(0);
        let owner = || {
            asked.set(asked.get() + 1);
            ServerView::Reported(0)
        };
        skips_as_own(ServerView::Reported(0), OURS, Some(SOMEBODY_ELSE), owner);
        skips_as_own(ServerView::Reported(0), OURS, None, owner);
        assert_eq!(asked.get(), 0);
        skips_as_own(ServerView::Reported(0), OURS, Some(OURS), owner);
        assert_eq!(asked.get(), 1);
    }

    #[test]
    fn the_logged_mode_names_how_the_check_is_backed() {
        assert_eq!(ownership_mode(ServerView::Reported(OURS), OURS), OwnershipMode::Verified);
        assert_eq!(ownership_mode(ServerView::Reported(OURS_ON_THE_HOST), OURS), OwnershipMode::Foreign);
        assert_eq!(ownership_mode(ServerView::Reported(0), OURS), OwnershipMode::Foreign);
        assert_eq!(ownership_mode(ServerView::Silent, OURS), OwnershipMode::Foreign);
        assert_eq!(ownership_mode(ServerView::Unavailable, OURS), OwnershipMode::Unknown);
        // Defensive: a zero on both sides is no identity.
        assert_eq!(ownership_mode(ServerView::Reported(0), 0), OwnershipMode::Foreign);
    }

    #[test]
    fn the_server_view_is_the_first_local_client_pid_of_the_reply() {
        use x11rb::protocol::res::{ClientIdMask, ClientIdSpec, ClientIdValue};
        let value = |pids: &[u32]| ClientIdValue {
            spec: ClientIdSpec { client: 0x0020_0000, mask: ClientIdMask::LOCAL_CLIENT_PID },
            value: pids.to_vec(),
        };
        assert_eq!(server_view_from_ids(&[value(&[OURS])]), ServerView::Reported(OURS));
        assert_eq!(server_view_from_ids(&[value(&[0])]), ServerView::Reported(0));
        assert_eq!(server_view_from_ids(&[value(&[])]), ServerView::Silent);
        assert_eq!(server_view_from_ids(&[]), ServerView::Silent);
    }

    #[test]
    fn the_server_is_asked_about_our_own_connection_once_per_connection() {
        let cell = OwnershipCell::default();
        let queries = std::cell::Cell::new(0);
        let query = || {
            queries.set(queries.get() + 1);
            ServerView::Reported(OURS)
        };
        for _ in 0..3 {
            assert_eq!(cell.get_or_decide(OURS, ":test", query), ServerView::Reported(OURS));
        }
        assert_eq!(queries.get(), 1, "the server is asked once per connection");

        // A second window manager — another runtime, possibly another display —
        // decides for itself.
        let other = OwnershipCell::default();
        assert_eq!(other.get_or_decide(OURS, ":other", || ServerView::Unavailable), ServerView::Unavailable);
        assert_eq!(cell.get_or_decide(OURS, ":test", || ServerView::Unavailable), ServerView::Reported(OURS));
    }
}
