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
use platynui_core::platform::{PlatformError, WindowId, WindowManager};
use platynui_core::types::{Point, Rect, Size};
use platynui_core::ui::{Namespace, UiNode};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use tracing::{debug, trace, warn};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ClientMessageEvent, ConfigureWindowAux, ConnectionExt, EventMask, Window,
};
use x11rb::rust_connection::RustConnection;

// ---------------------------------------------------------------------------
//  Atom cache
// ---------------------------------------------------------------------------

struct EwmhAtoms {
    net_client_list: Atom,
    net_wm_pid: Atom,
    net_active_window: Atom,
    net_close_window: Atom,
    net_wm_state: Atom,
    net_wm_state_maximized_vert: Atom,
    net_wm_state_maximized_horz: Atom,
    net_wm_state_hidden: Atom,
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
        net_wm_pid: intern(conn, b"_NET_WM_PID")?,
        net_active_window: intern(conn, b"_NET_ACTIVE_WINDOW")?,
        net_close_window: intern(conn, b"_NET_CLOSE_WINDOW")?,
        net_wm_state: intern(conn, b"_NET_WM_STATE")?,
        net_wm_state_maximized_vert: intern(conn, b"_NET_WM_STATE_MAXIMIZED_VERT")?,
        net_wm_state_maximized_horz: intern(conn, b"_NET_WM_STATE_MAXIMIZED_HORZ")?,
        net_wm_state_hidden: intern(conn, b"_NET_WM_STATE_HIDDEN")?,
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
    Ok(reply.value32().map(|iter| iter.collect()).unwrap_or_default())
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
    let (wx, wy) =
        coords.map(|c| (f64::from(c.dst_x), f64::from(c.dst_y))).unwrap_or((f64::from(geom.x), f64::from(geom.y)));
    Ok(Rect::new(wx, wy, f64::from(geom.width), f64::from(geom.height)))
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

/// Extract the process ID from a UiNode by walking up to the Application
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
            let rounded = v as u32;
            if rounded > 0 { Some(rounded) } else { None }
        }
        platynui_core::ui::UiValue::String(s) => s.parse::<u32>().ok(),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
//  WindowManager implementation
// ---------------------------------------------------------------------------

pub struct X11EwmhWindowManager {
    conn: Arc<X11Connection>,
}

impl X11EwmhWindowManager {
    pub fn new(conn: Arc<X11Connection>) -> Self {
        Self { conn }
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
        client_rect(&self.conn, id.raw() as Window)
    }

    fn is_active(&self, id: WindowId) -> Result<bool, PlatformError> {
        let xid = id.raw() as Window;
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

    fn activate(&self, id: WindowId) -> Result<(), PlatformError> {
        let xid = id.raw() as Window;
        debug!(xid, "EWMH activate");
        let atoms = atoms(&self.conn)?;
        let x11 = &self.conn;
        send_client_message(&x11.conn, x11.root, xid, atoms.net_active_window, [2, 0, 0, 0, 0])?;
        flush(&x11.conn)
    }

    fn close(&self, id: WindowId) -> Result<(), PlatformError> {
        let xid = id.raw() as Window;
        debug!(xid, "EWMH close");
        let atoms = atoms(&self.conn)?;
        let x11 = &self.conn;
        send_client_message(&x11.conn, x11.root, xid, atoms.net_close_window, [0, 2, 0, 0, 0])?;
        flush(&x11.conn)
    }

    fn minimize(&self, id: WindowId) -> Result<(), PlatformError> {
        let xid = id.raw() as Window;
        debug!(xid, "EWMH minimize (iconify)");
        let x11 = &self.conn;
        // XIconifyWindow equivalent: use ClientMessage WM_CHANGE_STATE with IconicState.
        let wm_change_state = intern(&x11.conn, b"WM_CHANGE_STATE")?;
        send_client_message(&x11.conn, x11.root, xid, wm_change_state, [3 /* IconicState */, 0, 0, 0, 0])?;
        flush(&x11.conn)
    }

    fn maximize(&self, id: WindowId) -> Result<(), PlatformError> {
        let xid = id.raw() as Window;
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
        let xid = id.raw() as Window;
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
        let xid = id.raw() as Window;
        debug!(xid, x = position.x(), y = position.y(), "EWMH move_to");
        let x11 = &self.conn;
        let aux = ConfigureWindowAux::new().x(position.x() as i32).y(position.y() as i32);
        x11.conn.configure_window(xid, &aux).map_err(|e| PlatformError::OperationFailed {
            operation: "x11 configure_window move",
            details: Some(e.to_string()),
        })?;
        flush(&x11.conn)
    }

    fn resize(&self, id: WindowId, size: Size) -> Result<(), PlatformError> {
        let xid = id.raw() as Window;
        debug!(xid, w = size.width(), h = size.height(), "EWMH resize");
        let x11 = &self.conn;
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
    let supported_set: Vec<Atom> = supported_reply.and_then(|r| r.value32().map(|it| it.collect())).unwrap_or_default();

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
