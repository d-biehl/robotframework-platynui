//! EWMH-based [`WindowManager`] for X11.
//!
//! Migrated from `provider-atspi/src/ewmh.rs`.  Holds the runtime's owned X11
//! connection ([`crate::x11util::X11Connection`]) and acts as a platform-level
//! window manager so any accessibility provider can resolve and manage native
//! windows without a direct `x11rb` dependency.
//!
//! The interned-atom cache ([`ATOMS`]) stays a process-global `OnceLock`: atoms
//! are stable for the lifetime of the X server, so caching them across runtimes
//! is safe and it is never cleared on shutdown. Only the connection is
//! per-instance.

use crate::x11util::X11Connection;
use platynui_core::platform::{PlatformError, WindowHit, WindowId, WindowManager, WindowState, WindowVisualState};
use platynui_core::types::{Point, Rect, Size};
use platynui_core::ui::{Namespace, UiNode};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use tracing::{debug, info, trace, warn};
use x11rb::connection::Connection;
use x11rb::protocol::res::{ClientIdMask, ClientIdSpec, ClientIdValue, ConnectionExt as _};
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ClientMessageEvent, ConfigureWindowAux, ConnectionExt, EventMask, MapState, Window,
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

/// Find X11 windows belonging to the given PID.  When multiple candidates
/// exist (e.g. a main window plus dialogs, all sharing one process), correlate
/// the node's AT-SPI screen extents with each candidate's client rect
/// (`node_extents`), falling back to matching the accessible `name` against
/// `_NET_WM_NAME`.
///
/// Geometry is the primary key on purpose: the accessible name and the window
/// title frequently diverge (e.g. a Qt dialog whose `accessibleName` differs
/// from its `windowTitle`), which defeats name matching. When neither key
/// disambiguates, this returns an error rather than guessing a candidate —
/// silently picking the wrong window yields another window's bounds (typically
/// the main window's), which is worse than an explicit failure.
fn find_xid_for_pid(
    x11: &X11Connection,
    pid: u32,
    window_name: Option<&str>,
    node_extents: Option<Rect>,
) -> Result<Window, PlatformError> {
    let atoms = atoms(x11)?;
    let client_list = get_client_list(&x11.conn, x11.root, atoms.net_client_list)?;

    let mut candidates: Vec<Window> = Vec::new();
    for &win in &client_list {
        if let Some(win_pid) = get_window_pid(&x11.conn, win, atoms.net_wm_pid)
            && win_pid == pid
        {
            candidates.push(win);
        }
    }

    match candidates.len() {
        0 => {
            warn!(pid, "no X11 window found for PID");
            Err(PlatformError::OperationFailed {
                operation: "resolve X11 window by PID",
                details: Some(format!("no window found for PID {pid}")),
            })
        }
        1 => {
            debug!(pid, xid = candidates[0], "resolved XID for PID");
            Ok(candidates[0])
        }
        _ => {
            // Primary: correlate the node's AT-SPI screen extents with the
            // candidates' client rects. Robust even when names diverge.
            if let Some(target) = node_extents
                && let Some(xid) = best_geometry_match(x11, &candidates, target)
            {
                debug!(pid, xid, "resolved XID by geometry match");
                return Ok(xid);
            }

            // Secondary: match the accessible name against _NET_WM_NAME.
            if let Some(name) = window_name
                && !name.is_empty()
                && let Some(xid) = best_name_match(&x11.conn, &candidates, name, &atoms)
            {
                debug!(pid, xid, name, "resolved XID by name match");
                return Ok(xid);
            }

            warn!(pid, count = candidates.len(), "could not disambiguate window for PID (no geometry or name match)");
            Err(PlatformError::OperationFailed {
                operation: "disambiguate X11 window for PID",
                details: Some(format!(
                    "{} candidate windows for PID {pid}, none matched by geometry or name",
                    candidates.len()
                )),
            })
        }
    }
}

/// Maximum summed absolute difference (in pixels, over x/y/w/h) between a
/// candidate's client rect and the node's AT-SPI screen extents for the two to
/// be considered the same window. On X11 both describe the client area, so a
/// match is near-exact; the tolerance only absorbs rounding / off-by-a-pixel
/// and guards against selecting an unrelated window.
const GEOMETRY_MATCH_TOLERANCE: f64 = 64.0;

/// Pick the candidate whose client rect is closest to `target` (the node's
/// AT-SPI screen extents), provided the closest is within
/// [`GEOMETRY_MATCH_TOLERANCE`]. Returns `None` if no candidate is close enough.
fn best_geometry_match(x11: &X11Connection, candidates: &[Window], target: Rect) -> Option<Window> {
    let mut best: Option<Window> = None;
    let mut best_dist = f64::MAX;
    for &win in candidates {
        let Ok(rect) = client_rect(x11, win) else { continue };
        let dist = (rect.x() - target.x()).abs()
            + (rect.y() - target.y()).abs()
            + (rect.width() - target.width()).abs()
            + (rect.height() - target.height()).abs();
        if dist < best_dist {
            best_dist = dist;
            best = Some(win);
        }
    }
    if best_dist <= GEOMETRY_MATCH_TOLERANCE { best } else { None }
}

/// Compute a window's client rect: screen position of its client origin
/// (`translate_coordinates`) plus its client size (`get_geometry`). Shared by
/// [`X11EwmhWindowManager::bounds`] and [`best_geometry_match`].
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

/// Find the candidate whose `_NET_WM_NAME` best matches the AT-SPI name.
fn best_name_match(
    conn: &RustConnection,
    candidates: &[Window],
    target_name: &str,
    atoms: &EwmhAtoms,
) -> Option<Window> {
    // Exact match first.
    for &win in candidates {
        if let Some(wm_name) = get_window_name(conn, win, atoms)
            && wm_name == target_name
        {
            return Some(win);
        }
    }
    // Substring / contains match (window titles often include extra text
    // like " — Application Name").
    for &win in candidates {
        if let Some(wm_name) = get_window_name(conn, win, atoms)
            && (wm_name.contains(target_name) || target_name.contains(&wm_name))
        {
            return Some(win);
        }
    }
    None
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

/// Read the node's AT-SPI screen extents via the raw `Component.Extents.Screen`
/// native attribute. This query goes straight to the accessibility provider and
/// does **not** route through this window manager, so it is safe to call from
/// [`X11EwmhWindowManager::resolve_window`] without recursing.
///
/// Returns `None` when the extents are unavailable or degenerate (zero-sized) —
/// e.g. on Wayland, where AT-SPI reports `0,0,0,0` — so geometry matching is
/// simply skipped and name matching takes over.
fn extract_screen_extents(node: &dyn UiNode) -> Option<Rect> {
    let attr = node.attribute(Namespace::Native, "Component.Extents.Screen")?;
    match attr.value() {
        platynui_core::ui::UiValue::Rect(rect) if rect.width() > 0.0 && rect.height() > 0.0 => Some(rect),
        _ => None,
    }
}

/// Try to read `control:ProcessId` from a single node.
fn pid_from_attr(node: &dyn UiNode) -> Option<u32> {
    let attr = node.attribute(Namespace::Control, "ProcessId")?;
    match attr.value() {
        platynui_core::ui::UiValue::Integer(v) => u32::try_from(v).ok(),
        platynui_core::ui::UiValue::Number(v) => {
            // Saturating float-to-int: negatives and NaN become 0, rejected below.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let rounded = v as u32;
            if rounded > 0 { Some(rounded) } else { None }
        }
        platynui_core::ui::UiValue::String(s) => s.parse::<u32>().ok(),
        _ => None,
    }
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
    /// The X server's view of this connection's process, for the own-window check.
    ownership: OwnershipCell,
}

impl X11EwmhWindowManager {
    pub fn new(conn: Arc<X11Connection>) -> Self {
        Self { conn, ownership: OwnershipCell::default() }
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
        for &win in stacking.iter().rev() {
            let Ok(rect) = client_rect(&self.conn, win) else { continue };
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

        // Primary disambiguation key when multiple windows share the PID: the
        // node's AT-SPI screen extents. This is a raw Component query that does
        // NOT go through this window manager, so it is recursion-safe here.
        let node_extents = extract_screen_extents(node);

        // Secondary key: the node's accessible name, matched against _NET_WM_NAME.
        let node_name = node.name();
        let name_hint = if node_name.is_empty() { None } else { Some(node_name.as_str()) };

        let xid = find_xid_for_pid(&self.conn, pid, name_hint, node_extents)?;
        trace!(pid, xid, "resolved WindowId");
        Ok(WindowId::new(u64::from(xid)))
    }

    fn bounds(&self, id: WindowId, _toolkit_hint: Option<&str>) -> Result<Rect, PlatformError> {
        client_rect(&self.conn, xid_of(id))
    }

    fn window_at_point(&self, point: Point) -> Result<Option<WindowHit>, PlatformError> {
        let (stacking_atom, pid_atom) = {
            let atoms = atoms(&self.conn)?;
            (atoms.net_client_list_stacking, atoms.net_wm_pid)
        };
        let own_pid = std::process::id();
        let ours = self.ownership.get_or_decide(own_pid, &self.conn.display, || query_own_view(&self.conn.conn));

        // Managed (WM-reparented) window at the point, from EWMH stacking.
        let managed = self.managed_window_at(point, stacking_atom, pid_atom, ours, own_pid);

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

    fn is_active(&self, id: WindowId) -> Result<bool, PlatformError> {
        let xid = xid_of(id);
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
        let atoms = atoms(&self.conn)?;
        read_window_state(&self.conn, xid_of(id), &atoms)
    }

    fn activate(&self, id: WindowId) -> Result<(), PlatformError> {
        let xid = xid_of(id);
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
        debug!(xid, "EWMH close");
        let atoms = atoms(&self.conn)?;
        let x11 = &self.conn;
        send_client_message(&x11.conn, x11.root, xid, atoms.net_close_window, [0, 2, 0, 0, 0])?;
        flush(&x11.conn)
    }

    fn minimize(&self, id: WindowId) -> Result<(), PlatformError> {
        let xid = xid_of(id);
        debug!(xid, "EWMH minimize (iconify)");
        let x11 = &self.conn;
        // XIconifyWindow equivalent: use ClientMessage WM_CHANGE_STATE with IconicState.
        let wm_change_state = intern(&x11.conn, b"WM_CHANGE_STATE")?;
        send_client_message(&x11.conn, x11.root, xid, wm_change_state, [3 /* IconicState */, 0, 0, 0, 0])?;
        flush(&x11.conn)
    }

    fn maximize(&self, id: WindowId) -> Result<(), PlatformError> {
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
