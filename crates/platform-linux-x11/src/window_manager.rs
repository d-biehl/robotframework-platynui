//! EWMH-based [`WindowManager`] for X11.
//!
//! Migrated from `provider-atspi/src/ewmh.rs`.  Uses the shared X11
//! connection from [`crate::x11util`] and registers as a platform-level
//! window manager provider so any accessibility provider can resolve and
//! manage native windows without a direct `x11rb` dependency.

use crate::x11util;
use platynui_core::platform::{PlatformError, PlatformErrorKind, WindowId, WindowManager};
use platynui_core::register_window_manager;
use platynui_core::types::{Point, Rect, Size};
use platynui_core::ui::{Namespace, UiNode};
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
        .map_err(|e| PlatformError::new(PlatformErrorKind::OperationFailed, format!("intern_atom request: {e}")))?
        .reply()
        .map(|r| r.atom)
        .map_err(|e| PlatformError::new(PlatformErrorKind::OperationFailed, format!("intern_atom reply: {e}")))
}

fn atoms() -> Result<std::sync::MutexGuard<'static, EwmhAtoms>, PlatformError> {
    if let Some(cell) = ATOMS.get() {
        return cell
            .lock()
            .map_err(|_| PlatformError::new(PlatformErrorKind::OperationFailed, "EWMH atoms mutex poisoned"));
    }
    let guard = x11util::connection()?;
    let conn = &guard.conn;
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
        .map_err(|_| PlatformError::new(PlatformErrorKind::OperationFailed, "EWMH atoms mutex poisoned"))
}

// ---------------------------------------------------------------------------
//  XID resolution helpers
// ---------------------------------------------------------------------------

fn get_client_list(conn: &RustConnection, root: Window, net_client_list: Atom) -> Result<Vec<Window>, PlatformError> {
    let reply = conn
        .get_property(false, root, net_client_list, AtomEnum::WINDOW, 0, u32::MAX)
        .map_err(|e| PlatformError::new(PlatformErrorKind::OperationFailed, format!("_NET_CLIENT_LIST: {e}")))?
        .reply()
        .map_err(|e| PlatformError::new(PlatformErrorKind::OperationFailed, format!("_NET_CLIENT_LIST reply: {e}")))?;
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
/// exist, disambiguate by comparing the AT-SPI window `name` against
/// `_NET_WM_NAME`.
fn find_xid_for_pid(pid: u32, window_name: Option<&str>) -> Result<Window, PlatformError> {
    let atoms = atoms()?;
    let x11 = x11util::connection()?;
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
            Err(PlatformError::new(PlatformErrorKind::OperationFailed, format!("no X11 window found for PID {pid}")))
        }
        1 => {
            debug!(pid, xid = candidates[0], "resolved XID for PID");
            Ok(candidates[0])
        }
        _ => {
            // Multiple windows for same PID — try to match by window title.
            if let Some(name) = window_name
                && !name.is_empty()
            {
                debug!(pid, count = candidates.len(), name, "multiple X11 windows for PID — matching by name");
                if let Some(xid) = best_name_match(&x11.conn, &candidates, name, &atoms) {
                    debug!(pid, xid, name, "resolved XID by name match");
                    return Ok(xid);
                }
                warn!(pid, name, "name match failed, using first candidate");
            } else {
                debug!(pid, count = candidates.len(), "multiple X11 windows for PID — no name hint, using first");
            }
            // Last resort: return the first candidate.
            Ok(candidates[0])
        }
    }
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
        .map_err(|e| PlatformError::new(PlatformErrorKind::OperationFailed, format!("send_event: {e}")))?;
    Ok(())
}

fn flush(conn: &RustConnection) -> Result<(), PlatformError> {
    conn.flush().map_err(|e| PlatformError::new(PlatformErrorKind::OperationFailed, format!("x11 flush: {e}")))
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

struct X11EwmhWindowManager;

impl WindowManager for X11EwmhWindowManager {
    fn name(&self) -> &'static str {
        "X11 EWMH"
    }

    fn resolve_window(&self, node: &dyn UiNode) -> Result<WindowId, PlatformError> {
        let pid = extract_pid(node)
            .ok_or_else(|| PlatformError::new(PlatformErrorKind::OperationFailed, "cannot extract PID from UiNode"))?;

        // Use the node's accessible name for window title matching when
        // multiple windows share the same PID.
        let node_name = node.name();
        let name_hint = if node_name.is_empty() { None } else { Some(node_name.as_str()) };

        let xid = find_xid_for_pid(pid, name_hint)?;
        trace!(pid, xid, "resolved WindowId");
        Ok(WindowId::new(u64::from(xid)))
    }

    fn bounds(&self, id: WindowId) -> Result<Rect, PlatformError> {
        let xid = id.raw() as Window;
        let x11 = x11util::connection()?;
        let geom = x11
            .conn
            .get_geometry(xid)
            .map_err(|e| PlatformError::new(PlatformErrorKind::OperationFailed, format!("get_geometry: {e}")))?
            .reply()
            .map_err(|e| PlatformError::new(PlatformErrorKind::OperationFailed, format!("get_geometry reply: {e}")))?;
        let coords = x11.conn.translate_coordinates(xid, x11.root, 0, 0).ok().and_then(|c| c.reply().ok());
        let (wx, wy) =
            coords.map(|c| (f64::from(c.dst_x), f64::from(c.dst_y))).unwrap_or((f64::from(geom.x), f64::from(geom.y)));
        Ok(Rect::new(wx, wy, f64::from(geom.width), f64::from(geom.height)))
    }

    fn is_active(&self, id: WindowId) -> Result<bool, PlatformError> {
        let xid = id.raw() as Window;
        let atoms = atoms()?;
        let x11 = x11util::connection()?;
        let reply = x11
            .conn
            .get_property(false, x11.root, atoms.net_active_window, AtomEnum::WINDOW, 0, 1)
            .map_err(|e| PlatformError::new(PlatformErrorKind::OperationFailed, format!("_NET_ACTIVE_WINDOW: {e}")))?
            .reply()
            .map_err(|e| {
                PlatformError::new(PlatformErrorKind::OperationFailed, format!("_NET_ACTIVE_WINDOW reply: {e}"))
            })?;
        let active_xid = reply.value32().and_then(|mut iter| iter.next()).unwrap_or(0);
        Ok(active_xid == xid)
    }

    fn activate(&self, id: WindowId) -> Result<(), PlatformError> {
        let xid = id.raw() as Window;
        debug!(xid, "EWMH activate");
        let atoms = atoms()?;
        let x11 = x11util::connection()?;
        send_client_message(&x11.conn, x11.root, xid, atoms.net_active_window, [2, 0, 0, 0, 0])?;
        flush(&x11.conn)
    }

    fn close(&self, id: WindowId) -> Result<(), PlatformError> {
        let xid = id.raw() as Window;
        debug!(xid, "EWMH close");
        let atoms = atoms()?;
        let x11 = x11util::connection()?;
        send_client_message(&x11.conn, x11.root, xid, atoms.net_close_window, [0, 2, 0, 0, 0])?;
        flush(&x11.conn)
    }

    fn minimize(&self, id: WindowId) -> Result<(), PlatformError> {
        let xid = id.raw() as Window;
        debug!(xid, "EWMH minimize (iconify)");
        let x11 = x11util::connection()?;
        // XIconifyWindow equivalent: use ClientMessage WM_CHANGE_STATE with IconicState.
        let wm_change_state = intern(&x11.conn, b"WM_CHANGE_STATE")?;
        send_client_message(&x11.conn, x11.root, xid, wm_change_state, [3 /* IconicState */, 0, 0, 0, 0])?;
        flush(&x11.conn)
    }

    fn maximize(&self, id: WindowId) -> Result<(), PlatformError> {
        let xid = id.raw() as Window;
        debug!(xid, "EWMH maximize");
        let atoms = atoms()?;
        let x11 = x11util::connection()?;
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
        let atoms = atoms()?;
        let x11 = x11util::connection()?;
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
        let x11 = x11util::connection()?;
        let aux = ConfigureWindowAux::new().x(position.x() as i32).y(position.y() as i32);
        x11.conn
            .configure_window(xid, &aux)
            .map_err(|e| PlatformError::new(PlatformErrorKind::OperationFailed, format!("configure_window: {e}")))?;
        flush(&x11.conn)
    }

    fn resize(&self, id: WindowId, size: Size) -> Result<(), PlatformError> {
        let xid = id.raw() as Window;
        debug!(xid, w = size.width(), h = size.height(), "EWMH resize");
        let x11 = x11util::connection()?;
        let aux = ConfigureWindowAux::new().width(size.width() as u32).height(size.height() as u32);
        x11.conn
            .configure_window(xid, &aux)
            .map_err(|e| PlatformError::new(PlatformErrorKind::OperationFailed, format!("configure_window: {e}")))?;
        flush(&x11.conn)
    }
}

// ---------------------------------------------------------------------------
//  Registration
// ---------------------------------------------------------------------------

static PROVIDER: X11EwmhWindowManager = X11EwmhWindowManager;

register_window_manager!(&PROVIDER);

// ---------------------------------------------------------------------------
//  EWMH WM detection — called during PlatformModule::initialize()
// ---------------------------------------------------------------------------

/// Check whether an EWMH-compatible window manager is running and log the
/// result.  Returns `Ok(true)` when a WM was detected, `Ok(false)` when the
/// check cannot confirm WM presence (non-fatal).
pub fn check_ewmh_wm_support() -> Result<bool, PlatformError> {
    // Initialize atoms first (needs its own connection lock) before acquiring
    // the x11 guard below — otherwise we deadlock on the non-reentrant Mutex.
    let atoms = atoms()?;
    let x11 = x11util::connection()?;

    // 1. _NET_SUPPORTING_WM_CHECK on root → child window
    let child_reply = x11
        .conn
        .get_property(false, x11.root, atoms.net_supporting_wm_check, AtomEnum::WINDOW, 0, 1)
        .map_err(|e| PlatformError::new(PlatformErrorKind::OperationFailed, format!("_NET_SUPPORTING_WM_CHECK: {e}")))?
        .reply()
        .map_err(|e| {
            PlatformError::new(PlatformErrorKind::OperationFailed, format!("_NET_SUPPORTING_WM_CHECK reply: {e}"))
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
