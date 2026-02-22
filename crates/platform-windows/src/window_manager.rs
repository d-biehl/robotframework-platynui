//! Win32-based [`WindowManager`] for Windows.
//!
//! Uses native HWND-based window management APIs and registers as a
//! platform-level provider so any accessibility provider (or the runtime
//! itself) can resolve and manage native windows without a direct dependency on
//! UIAutomation patterns.
//!
//! ## Window resolution strategy
//!
//! 1. **Direct HWND** — reads `native:NativeWindowHandle` (UIA property 30005)
//!    from the node.  This is the fast path used when the node comes from the
//!    Windows UIA provider.
//! 2. **PID fallback** — if the node carries `native:ProcessId` (e.g. via an
//!    Application node), we enumerate top-level windows with `EnumWindows` and
//!    match by PID.

use platynui_core::platform::{PlatformError, PlatformErrorKind, WindowId, WindowManager};
use platynui_core::register_window_manager;
use platynui_core::types::{Point, Rect, Size};
use platynui_core::ui::{Namespace, UiNode, UiValue};
use tracing::debug;
use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId, IsIconic,
    IsWindowVisible, PostMessageW, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SetForegroundWindow, SetWindowPos, ShowWindow, WM_CLOSE,
};
use windows::core::BOOL;

// ---------------------------------------------------------------------------
//  HWND ↔ WindowId conversions
// ---------------------------------------------------------------------------

fn hwnd_from_id(id: WindowId) -> HWND {
    HWND(id.raw() as *mut core::ffi::c_void)
}

fn id_from_hwnd(hwnd: HWND) -> WindowId {
    WindowId::new(hwnd.0 as usize as u64)
}

// ---------------------------------------------------------------------------
//  Window resolution helpers
// ---------------------------------------------------------------------------

/// Extract the native window handle (HWND) directly from a UiNode.
///
/// On Windows, UIA publishes property 30005 (`NativeWindowHandle`) which the
/// provider exposes under `native:NativeWindowHandle`.
fn extract_hwnd(node: &dyn UiNode) -> Option<HWND> {
    let attr = node.attribute(Namespace::Native, "NativeWindowHandle")?;
    let raw = match attr.value() {
        UiValue::Integer(v) => v as usize,
        UiValue::Number(v) => v as usize,
        _ => return None,
    };
    if raw == 0 {
        return None;
    }
    Some(HWND(raw as *mut core::ffi::c_void))
}

/// Extract the process ID from a UiNode by walking up to the Application
/// node and reading `ProcessId`.
///
/// Application nodes are the canonical source of PID information across all
/// providers (AT-SPI, UIA, etc.).
fn extract_pid(node: &dyn UiNode) -> Option<u32> {
    pid_from_attr(node).or_else(|| {
        let mut current = node.parent()?.upgrade()?;
        loop {
            if let Some(pid) = pid_from_attr(&*current) {
                return Some(pid);
            }
            current = current.parent()?.upgrade()?;
        }
    })
}

/// Try to read `control:ProcessId` from a single node.
fn pid_from_attr(node: &dyn UiNode) -> Option<u32> {
    let attr = node.attribute(Namespace::Control, "ProcessId")?;
    match attr.value() {
        UiValue::Integer(v) => u32::try_from(v).ok(),
        UiValue::Number(v) => {
            let rounded = v as u32;
            if rounded > 0 { Some(rounded) } else { None }
        }
        UiValue::String(s) => s.parse::<u32>().ok(),
        _ => None,
    }
}

/// Find a top-level visible window belonging to the given process.
///
/// Uses `EnumWindows` to iterate all top-level windows and matches on PID.
/// When multiple windows match, the first visible one wins.
fn find_hwnd_for_pid(pid: u32) -> Result<HWND, PlatformError> {
    struct EnumData {
        target_pid: u32,
        result: Option<HWND>,
    }

    unsafe extern "system" fn callback(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
        let data = unsafe { &mut *(lparam.0 as *mut EnumData) };
        let mut win_pid: u32 = 0;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut win_pid)) };
        if win_pid == data.target_pid && unsafe { IsWindowVisible(hwnd) }.as_bool() {
            data.result = Some(hwnd);
            return BOOL(0); // stop enumeration
        }
        BOOL(1) // continue
    }

    let mut data = EnumData { target_pid: pid, result: None };
    // EnumWindows returns Err when the callback stops enumeration early
    // (by returning FALSE). We ignore that — the result is in `data`.
    let _ = unsafe { EnumWindows(Some(callback), LPARAM(&raw mut data as isize)) };

    data.result.ok_or_else(|| {
        PlatformError::new(PlatformErrorKind::OperationFailed, format!("no visible window found for PID {pid}"))
    })
}

// ---------------------------------------------------------------------------
//  WindowManager implementation
// ---------------------------------------------------------------------------

struct Win32WindowManager;

impl WindowManager for Win32WindowManager {
    fn name(&self) -> &'static str {
        "Win32"
    }

    fn resolve_window(&self, node: &dyn UiNode) -> Result<WindowId, PlatformError> {
        // Fast path: direct HWND from UIA native attribute.
        if let Some(hwnd) = extract_hwnd(node) {
            debug!(hwnd = hwnd.0 as usize, "resolved WindowId from NativeWindowHandle");
            return Ok(id_from_hwnd(hwnd));
        }

        // Fallback: enumerate windows by PID.
        if let Some(pid) = extract_pid(node) {
            let hwnd = find_hwnd_for_pid(pid)?;
            debug!(pid, hwnd = hwnd.0 as usize, "resolved WindowId via PID enumeration");
            return Ok(id_from_hwnd(hwnd));
        }

        Err(PlatformError::new(
            PlatformErrorKind::OperationFailed,
            "cannot resolve window: no NativeWindowHandle or ProcessId available on node",
        ))
    }

    fn bounds(&self, id: WindowId) -> Result<Rect, PlatformError> {
        let hwnd = hwnd_from_id(id);
        let mut rect = RECT::default();
        unsafe { GetWindowRect(hwnd, &mut rect) }
            .map_err(|e| PlatformError::new(PlatformErrorKind::OperationFailed, format!("GetWindowRect: {e}")))?;
        Ok(Rect::new(
            f64::from(rect.left),
            f64::from(rect.top),
            f64::from(rect.right - rect.left),
            f64::from(rect.bottom - rect.top),
        ))
    }

    fn is_active(&self, id: WindowId) -> Result<bool, PlatformError> {
        let hwnd = hwnd_from_id(id);
        let fg = unsafe { GetForegroundWindow() };
        Ok(fg == hwnd)
    }

    fn activate(&self, id: WindowId) -> Result<(), PlatformError> {
        let hwnd = hwnd_from_id(id);

        // Restore from minimised state before bringing to foreground.
        if unsafe { IsIconic(hwnd) }.as_bool() {
            let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
        }

        // Windows enforces a foreground lock: only the thread that owns
        // the current foreground window may call SetForegroundWindow
        // successfully.  All other callers merely flash the taskbar
        // button instead of raising the window.
        //
        // To work around this we attach our thread to the *foreground*
        // window's input queue (inheriting its foreground privilege),
        // and optionally also to the *target* window's queue.  This is
        // the standard pattern used by UI automation frameworks (FlaUI,
        // pywinauto, WinAppDriver, etc.).
        let our_tid = unsafe { GetCurrentThreadId() };

        let fg_hwnd = unsafe { GetForegroundWindow() };
        let fg_tid = unsafe { GetWindowThreadProcessId(fg_hwnd, None) };
        let attached_fg =
            fg_tid != 0 && fg_tid != our_tid && unsafe { AttachThreadInput(our_tid, fg_tid, true) }.as_bool();

        let target_tid = unsafe { GetWindowThreadProcessId(hwnd, None) };
        let attached_target = target_tid != 0
            && target_tid != our_tid
            && target_tid != fg_tid
            && unsafe { AttachThreadInput(our_tid, target_tid, true) }.as_bool();

        let _ = unsafe { BringWindowToTop(hwnd) };
        let ok = unsafe { SetForegroundWindow(hwnd) };

        if attached_target {
            let _ = unsafe { AttachThreadInput(our_tid, target_tid, false) };
        }
        if attached_fg {
            let _ = unsafe { AttachThreadInput(our_tid, fg_tid, false) };
        }

        if !ok.as_bool() {
            tracing::warn!(
                hwnd = hwnd.0 as usize,
                "SetForegroundWindow returned FALSE — caller may lack foreground rights"
            );
        }
        Ok(())
    }

    fn close(&self, id: WindowId) -> Result<(), PlatformError> {
        let hwnd = hwnd_from_id(id);
        unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) }
            .map_err(|e| PlatformError::new(PlatformErrorKind::OperationFailed, format!("PostMessageW(WM_CLOSE): {e}")))
    }

    fn minimize(&self, id: WindowId) -> Result<(), PlatformError> {
        let hwnd = hwnd_from_id(id);
        let _ = unsafe { ShowWindow(hwnd, SW_MINIMIZE) };
        Ok(())
    }

    fn maximize(&self, id: WindowId) -> Result<(), PlatformError> {
        let hwnd = hwnd_from_id(id);
        let _ = unsafe { ShowWindow(hwnd, SW_MAXIMIZE) };
        Ok(())
    }

    fn restore(&self, id: WindowId) -> Result<(), PlatformError> {
        let hwnd = hwnd_from_id(id);

        // SW_RESTORE handles both minimised and maximised states.
        let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };

        // Also bring the window to the foreground so it is visible and active.
        let _ = unsafe { SetForegroundWindow(hwnd) };
        Ok(())
    }

    fn move_to(&self, id: WindowId, position: Point) -> Result<(), PlatformError> {
        let hwnd = hwnd_from_id(id);
        debug!(hwnd = hwnd.0 as usize, x = position.x(), y = position.y(), "Win32 move_to");
        unsafe {
            SetWindowPos(
                hwnd,
                None,
                position.x() as i32,
                position.y() as i32,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        }
        .map_err(|e| PlatformError::new(PlatformErrorKind::OperationFailed, format!("SetWindowPos (move): {e}")))
    }

    fn resize(&self, id: WindowId, size: Size) -> Result<(), PlatformError> {
        let hwnd = hwnd_from_id(id);
        debug!(hwnd = hwnd.0 as usize, w = size.width(), h = size.height(), "Win32 resize");
        unsafe {
            SetWindowPos(
                hwnd,
                None,
                0,
                0,
                size.width() as i32,
                size.height() as i32,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        }
        .map_err(|e| PlatformError::new(PlatformErrorKind::OperationFailed, format!("SetWindowPos (resize): {e}")))
    }
}

// ---------------------------------------------------------------------------
//  Registration
// ---------------------------------------------------------------------------

static PROVIDER: Win32WindowManager = Win32WindowManager;

register_window_manager!(&PROVIDER);
