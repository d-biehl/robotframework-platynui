//! EWMH (Extended Window Manager Hints) helpers for X11 window management.
//!
//! Provides functions to resolve X11 window IDs (XIDs) from process IDs and
//! screen coordinates, and to perform window actions (activate, close) through
//! standard EWMH client messages.  These work universally across toolkits and
//! window managers that support the EWMH specification.

use once_cell::sync::OnceCell;
use std::env;
use std::sync::Mutex;
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{Atom, AtomEnum, ClientMessageEvent, ConnectionExt, EventMask, Window};
use x11rb::rust_connection::RustConnection;

/// Shared X11 connection for EWMH operations.
struct X11Handle {
    conn: RustConnection,
    root: Window,
    /// Cached atoms — resolved once and reused.
    atoms: Atoms,
}

struct Atoms {
    net_client_list: Atom,
    net_wm_pid: Atom,
    net_active_window: Atom,
    net_close_window: Atom,
}

static X11: OnceCell<Mutex<X11Handle>> = OnceCell::new();

fn x11() -> Result<std::sync::MutexGuard<'static, X11Handle>, String> {
    let display = env::var("DISPLAY").map_err(|_| "DISPLAY environment variable not set".to_string())?;

    let cell = X11.get_or_try_init(|| {
        let (conn, screen_num) = connect_raw(&display)?;
        let root = conn.setup().roots[screen_num].root;

        // Intern frequently used atoms up front.
        let net_client_list = intern(&conn, b"_NET_CLIENT_LIST")?;
        let net_wm_pid = intern(&conn, b"_NET_WM_PID")?;
        let net_active_window = intern(&conn, b"_NET_ACTIVE_WINDOW")?;
        let net_close_window = intern(&conn, b"_NET_CLOSE_WINDOW")?;

        let atoms = Atoms { net_client_list, net_wm_pid, net_active_window, net_close_window };

        Ok::<Mutex<X11Handle>, String>(Mutex::new(X11Handle { conn, root, atoms }))
    })?;

    cell.lock().map_err(|_| "X11 mutex poisoned".to_string())
}

/// Connect to the X11 server with a timeout to avoid hanging when the server
/// is unavailable.
fn connect_raw(display: &str) -> Result<(RustConnection, usize), String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let disp = display.to_owned();
    std::thread::spawn(move || {
        let res = x11rb::connect(Some(&disp)).map_err(|e| format!("x11 connect: {e}"));
        let _ = tx.send(res);
    });
    match rx.recv_timeout(Duration::from_millis(500)) {
        Ok(res) => res,
        Err(_) => Err("x11 connect timed out".to_string()),
    }
}

fn intern(conn: &RustConnection, name: &[u8]) -> Result<Atom, String> {
    conn.intern_atom(false, name)
        .map_err(|e| format!("intern_atom request: {e}"))?
        .reply()
        .map(|r| r.atom)
        .map_err(|e| format!("intern_atom reply: {e}"))
}

// ---------------------------------------------------------------------------
//  XID resolution
// ---------------------------------------------------------------------------

/// Find the X11 window ID for a top-level window belonging to `pid` whose
/// geometry overlaps the given screen rectangle.
///
/// Strategy:
/// 1. Read `_NET_CLIENT_LIST` from the root window.
/// 2. Filter windows by `_NET_WM_PID == pid`.
/// 3. If exactly one candidate remains, return it.
/// 4. Otherwise pick the one whose geometry best matches `(x, y, w, h)`.
pub fn find_xid_for_pid(pid: u32, x: i32, y: i32, w: i32, h: i32) -> Result<Window, String> {
    let handle = x11()?;
    let client_list = get_client_list(&handle)?;

    // Filter by PID.
    let mut candidates: Vec<Window> = Vec::new();
    for &win in &client_list {
        if let Some(win_pid) = get_window_pid(&handle, win)
            && win_pid == pid
        {
            candidates.push(win);
        }
    }

    match candidates.len() {
        0 => {
            tracing::warn!(pid, "no X11 window found for PID");
            Err(format!("no X11 window found for PID {pid}"))
        }
        1 => {
            tracing::debug!(pid, xid = candidates[0], "resolved XID for PID");
            Ok(candidates[0])
        }
        _ => {
            tracing::debug!(pid, count = candidates.len(), "multiple X11 windows for PID — selecting by geometry");
            // Multiple windows — pick the one whose geometry best matches.
            best_geometry_match(&handle, &candidates, x, y, w, h)
        }
    }
}

/// Find the X11 window belonging to `pid`.
/// Simpler variant when we don't have geometry to disambiguate — returns the
/// first match.
pub fn find_xid_for_pid_simple(pid: u32) -> Result<Window, String> {
    let handle = x11()?;
    let client_list = get_client_list(&handle)?;

    for &win in &client_list {
        if let Some(win_pid) = get_window_pid(&handle, win)
            && win_pid == pid
        {
            tracing::debug!(pid, xid = win, "resolved XID for PID (simple)");
            return Ok(win);
        }
    }
    tracing::warn!(pid, "no X11 window found for PID (simple)");
    Err(format!("no X11 window found for PID {pid}"))
}

fn get_client_list(handle: &X11Handle) -> Result<Vec<Window>, String> {
    let reply = handle
        .conn
        .get_property(false, handle.root, handle.atoms.net_client_list, AtomEnum::WINDOW, 0, u32::MAX)
        .map_err(|e| format!("get_property _NET_CLIENT_LIST: {e}"))?
        .reply()
        .map_err(|e| format!("_NET_CLIENT_LIST reply: {e}"))?;

    // Value is array of u32 (Window IDs).
    Ok(reply.value32().map(|iter| iter.collect()).unwrap_or_default())
}

fn get_window_pid(handle: &X11Handle, win: Window) -> Option<u32> {
    let reply =
        handle.conn.get_property(false, win, handle.atoms.net_wm_pid, AtomEnum::CARDINAL, 0, 1).ok()?.reply().ok()?;
    reply.value32().and_then(|mut iter| iter.next())
}

fn best_geometry_match(
    handle: &X11Handle,
    candidates: &[Window],
    target_x: i32,
    target_y: i32,
    target_w: i32,
    target_h: i32,
) -> Result<Window, String> {
    let mut best: Option<(Window, i64)> = None;
    for &win in candidates {
        if let Ok(geom) = handle.conn.get_geometry(win).map(|cookie| cookie.reply())
            && let Ok(geom) = geom
        {
            // Translate to root coordinates.
            let coords = handle.conn.translate_coordinates(win, handle.root, 0, 0).ok().and_then(|c| c.reply().ok());
            let (wx, wy) = coords.map(|c| (c.dst_x as i32, c.dst_y as i32)).unwrap_or((geom.x as i32, geom.y as i32));
            let ww = geom.width as i32;
            let wh = geom.height as i32;

            // Manhattan distance between centres + size difference.
            let dx = (wx + ww / 2) - (target_x + target_w / 2);
            let dy = (wy + wh / 2) - (target_y + target_h / 2);
            let dw = ww - target_w;
            let dh = wh - target_h;
            let score = (dx as i64).abs() + (dy as i64).abs() + (dw as i64).abs() + (dh as i64).abs();

            if best.is_none() || score < best.unwrap().1 {
                best = Some((win, score));
            }
        }
    }
    best.map(|(w, _)| w).ok_or_else(|| "could not determine geometry for any candidate window".to_string())
}

// ---------------------------------------------------------------------------
//  EWMH window actions
// ---------------------------------------------------------------------------

/// Activate (raise + focus) a window via `_NET_ACTIVE_WINDOW`.
pub fn activate_window(xid: Window) -> Result<(), String> {
    tracing::debug!(xid, "EWMH activate_window");
    let handle = x11()?;
    send_client_message(
        &handle,
        xid,
        handle.atoms.net_active_window,
        [
            2, // source indication: pager/automation tool (bypasses focus-stealing prevention)
            0, // timestamp (0 = current)
            0, // currently active window (0 = none)
            0, 0,
        ],
    )?;
    handle.conn.flush().map_err(|e| format!("x11 flush: {e}"))?;
    Ok(())
}

/// Request the window manager to close a window via `_NET_CLOSE_WINDOW`.
pub fn close_window(xid: Window) -> Result<(), String> {
    tracing::debug!(xid, "EWMH close_window");
    let handle = x11()?;
    send_client_message(
        &handle,
        xid,
        handle.atoms.net_close_window,
        [
            0, // timestamp
            2, // source indication: pager/automation tool
            0, 0, 0,
        ],
    )?;
    handle.conn.flush().map_err(|e| format!("x11 flush: {e}"))?;
    Ok(())
}

/// Check if a window is the currently active (foreground) window.
pub fn is_active_window(xid: Window) -> Result<bool, String> {
    let handle = x11()?;
    let atom_active = handle.atoms.net_active_window;
    let reply = handle
        .conn
        .get_property(false, handle.root, atom_active, AtomEnum::WINDOW, 0, 1)
        .map_err(|e| format!("get_property _NET_ACTIVE_WINDOW: {e}"))?
        .reply()
        .map_err(|e| format!("_NET_ACTIVE_WINDOW reply: {e}"))?;
    let active_xid = reply.value32().and_then(|mut iter| iter.next()).unwrap_or(0);
    Ok(active_xid == xid)
}

fn send_client_message(handle: &X11Handle, win: Window, message_type: Atom, data: [u32; 5]) -> Result<(), String> {
    let event = ClientMessageEvent::new(32, win, message_type, data);
    let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
    handle.conn.send_event(false, handle.root, mask, event).map_err(|e| format!("send_event: {e}"))?;
    Ok(())
}
