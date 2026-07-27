//! The fallback for a window whose native handle the JVM would not give up
//! (design 5, task 2.4).
//!
//! # Why a fallback at all
//!
//! The in-JVM path is the good one: the agent reads the handle from the AWT peer
//! and the mapping is exact. Measurement says it works on every JDK once the
//! agent opens the needed `java.desktop` packages to itself — so this is a real
//! fallback, not the everyday path. What it covers is a JVM that refuses the
//! module redefinition, or a future peer layout the reflective chain no longer
//! recognises.
//!
//! # Why PID *plus geometry*
//!
//! The platform layer already has a PID-only fallback, and its own comment says
//! what is wrong with it: it "can target the wrong sibling window" — it takes the
//! first visible top-level window of the process. For a Swing application that is
//! a coin flip the moment a dialog is open, and the window patterns would then
//! move, resize or close the wrong window.
//!
//! Geometry is what turns the guess into a match. The agent reported the window's
//! rectangle in the same physical desktop pixels `GetWindowRect` answers in, so a
//! candidate either agrees or it does not. **An ambiguous answer is refused**: two
//! windows of one process at the same rectangle means there is nothing to
//! distinguish them, and acting on either would be acting on a guess.
//!
//! This is Windows-shaped on purpose. It presumes a native window list to match
//! against, which X11 does not offer the same way — `java-provider-linux` decides
//! that separately, and its preferred answer is that the agent supplies the id so
//! no fallback is needed.

use platynui_core::types::Rect;
use tracing::debug;

/// How far a candidate's rectangle may differ from the agent's and still count
/// as the same window, in physical pixels.
///
/// Not zero: the two rectangles are produced by different code paths at
/// different instants, and a window being dragged moves between them. Not
/// generous either — a tolerance wide enough to match a sibling window would
/// defeat the point.
const TOLERANCE: f64 = 4.0;

/// Find the process's top-level window whose rectangle matches `bounds`.
///
/// Returns `None` when nothing matches, when the agent reported no bounds to
/// match against, or when more than one candidate matches — see the module docs
/// on why ambiguity is refused rather than resolved.
pub(crate) fn match_by_pid_and_geometry(pid: u32, bounds: Option<Rect>) -> Option<u64> {
    let Some(target) = bounds else {
        debug!(pid, "no window bounds from the agent; nothing to match a native window against");
        return None;
    };
    let candidates: Vec<(u64, Rect)> =
        top_level_windows_of(pid).into_iter().filter(|(_, rect)| matches(*rect, target)).collect();
    match candidates.as_slice() {
        [(handle, _)] => Some(*handle),
        [] => {
            debug!(pid, ?target, "no top-level window of this process matches the agent's rectangle");
            None
        }
        several => {
            debug!(
                pid,
                ?target,
                count = several.len(),
                "several windows of this process match the same rectangle; refusing to guess"
            );
            None
        }
    }
}

fn matches(candidate: Rect, target: Rect) -> bool {
    (candidate.x() - target.x()).abs() <= TOLERANCE
        && (candidate.y() - target.y()).abs() <= TOLERANCE
        && (candidate.width() - target.width()).abs() <= TOLERANCE
        && (candidate.height() - target.height()).abs() <= TOLERANCE
}

/// Visible top-level windows of `pid`, with their screen rectangles.
#[allow(unsafe_code)]
fn top_level_windows_of(pid: u32) -> Vec<(u64, Rect)> {
    use windows::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowRect, GetWindowThreadProcessId, IsWindowVisible,
    };
    use windows::core::BOOL;

    struct Collector {
        pid: u32,
        found: Vec<(u64, Rect)>,
    }

    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: `lparam` carries the `&mut Collector` passed to `EnumWindows`
        // below; every per-window query here is read-only.
        unsafe {
            let collector = &mut *(lparam.0 as *mut Collector);
            let mut owner = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&raw mut owner));
            if owner == collector.pid && IsWindowVisible(hwnd).as_bool() {
                let mut rect = RECT::default();
                if GetWindowRect(hwnd, &raw mut rect).is_ok() {
                    collector.found.push((
                        hwnd.0 as usize as u64,
                        Rect::new(
                            f64::from(rect.left),
                            f64::from(rect.top),
                            f64::from(rect.right - rect.left),
                            f64::from(rect.bottom - rect.top),
                        ),
                    ));
                }
            }
        }
        BOOL(1)
    }

    let mut collector = Collector { pid, found: Vec::new() };
    // SAFETY: the callback only appends to the Vec passed through `lparam`.
    let _ = unsafe { EnumWindows(Some(collect), LPARAM(std::ptr::addr_of_mut!(collector) as isize)) };
    collector.found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rectangle_matches_within_the_tolerance_and_not_beyond_it() {
        let target = Rect::new(100.0, 200.0, 800.0, 600.0);
        assert!(matches(target, target));
        // A window that moved a pixel between the two reads is the same window.
        assert!(matches(Rect::new(101.0, 201.0, 800.0, 600.0), target));
        // A sibling window somewhere else is not.
        assert!(!matches(Rect::new(140.0, 200.0, 800.0, 600.0), target));
        assert!(!matches(Rect::new(100.0, 200.0, 400.0, 600.0), target));
    }

    /// Without bounds there is nothing to match, and the honest answer is "no
    /// handle" — the alternative would be the platform layer's coin flip.
    #[test]
    fn no_bounds_means_no_match_rather_than_the_first_window() {
        assert_eq!(match_by_pid_and_geometry(std::process::id(), None), None);
    }

    /// A rectangle no window of this process occupies must not resolve to one.
    #[test]
    fn a_rectangle_nothing_occupies_resolves_to_nothing() {
        let absurd = Rect::new(-99_000.0, -99_000.0, 3.0, 3.0);
        assert_eq!(match_by_pid_and_geometry(std::process::id(), Some(absurd)), None);
    }
}
