//! Wayland desktop information provider.
//!
//! Reads monitor data collected during initialization (via `wl_output` +
//! `xdg-output-manager`) and returns a [`DesktopInfo`] with real bounds.
//!
//! Monitor bounds use physical pixel coordinates (native resolution) for
//! consistency with X11 and Windows platforms. Physical positions are
//! computed from the compositor's logical layout by propagating physical
//! dimensions along adjacent monitor edges.
//!
//! Submodules:
//! - [`display_config`] — compositor-specific D-Bus enrichment (Mutter, `KWin`)
//! - [`output_info`] — per-output data model

pub mod display_config;
pub mod output_info;

pub use output_info::OutputInfo;

use std::env;
use std::sync::Mutex;

use platynui_core::platform::{DesktopInfo, DesktopInfoProvider, MonitorInfo, PlatformError};
use platynui_core::types::Rect;
use platynui_core::ui::{DESKTOP_RUNTIME_ID, RuntimeId, TechnologyId};

use crate::connection;

// ---------------------------------------------------------------------------
//  Global output state
// ---------------------------------------------------------------------------

static OUTPUTS: Mutex<Option<Vec<OutputInfo>>> = Mutex::new(None);

/// Store the collected output information for later retrieval.
///
/// # Panics
///
/// Panics if the internal mutex is poisoned.
pub fn set_outputs(outputs: Vec<OutputInfo>) {
    let mut guard = OUTPUTS.lock().expect("desktop output mutex poisoned");
    *guard = Some(outputs);
}

/// Retrieve the stored output information.
///
/// # Panics
///
/// Panics if called before [`set_outputs`] or if the mutex is poisoned.
pub fn outputs() -> Vec<OutputInfo> {
    let guard = OUTPUTS.lock().expect("desktop output mutex poisoned");
    guard.as_ref().expect("Desktop outputs not initialized — call set_outputs() first").clone()
}

/// Clear output state during shutdown.
///
/// # Panics
///
/// Panics if the internal mutex is poisoned.
pub fn clear_outputs() {
    let mut guard = OUTPUTS.lock().expect("desktop output mutex poisoned");
    *guard = None;
}

pub struct WaylandDesktopInfo;

impl DesktopInfoProvider for WaylandDesktopInfo {
    fn desktop_info(&self) -> Result<DesktopInfo, PlatformError> {
        let outputs = outputs();
        let physical_positions = compute_physical_positions(&outputs);

        let compositor = connection::compositor();

        let monitors: Vec<MonitorInfo> = outputs
            .iter()
            .enumerate()
            .map(|(i, o)| {
                let (px, py) = physical_positions[i];
                let w = f64::from(o.physical_width());
                let h = f64::from(o.physical_height());
                let bounds = Rect::new(px, py, w, h);
                let id = o.name.clone().unwrap_or_else(|| format!("wl-output-{i}"));

                // Wayland has no "primary" concept — use compositor-specific
                // D-Bus query (Mutter/KWin) or fall back to origin (0, 0).
                let is_primary = o.is_primary;

                MonitorInfo {
                    id,
                    name: o.description.clone(),
                    bounds,
                    is_primary,
                    scale_factor: Some(o.effective_scale()),
                }
            })
            .collect();

        let bounds = union_bounds(&monitors);

        Ok(DesktopInfo {
            runtime_id: RuntimeId::from(DESKTOP_RUNTIME_ID),
            name: format!("Wayland Desktop ({compositor})"),
            technology: TechnologyId::from("Wayland"),
            bounds,
            os_name: env::consts::OS.to_string(),
            os_version: env::consts::ARCH.to_string(),
            monitors,
        })
    }
}

// ---------------------------------------------------------------------------
//  Physical pixel position computation
// ---------------------------------------------------------------------------

/// Check whether two 1D ranges `[a0, a1)` and `[b0, b1)` overlap.
fn ranges_overlap(a0: i32, a1: i32, b0: i32, b1: i32) -> bool {
    a0 < b1 && b0 < a1
}

/// Compute physical pixel positions for each monitor.
///
/// Under Wayland the compositor arranges monitors in a logical coordinate
/// space. With fractional scaling the logical size differs from the native
/// pixel count (e.g. a 3840×2160 panel at 125% has logical size 3072×1728).
///
/// This function maps the logical layout to a unified physical pixel
/// coordinate space by propagating physical dimensions along the edges of
/// adjacent monitors. For each monitor it looks for a neighbor whose
/// logical edge exactly touches this monitor's edge on the same axis, then
/// sets the physical position as `neighbor_physical_pos + neighbor_physical_size`.
///
/// Fallback (no adjacent neighbor on an axis): the logical coordinate is
/// used directly — correct when the effective scale is 1.0.
fn compute_physical_positions(outputs: &[OutputInfo]) -> Vec<(f64, f64)> {
    let n = outputs.len();
    if n == 0 {
        return vec![];
    }

    // Logical rect + physical size per monitor.
    let mons: Vec<(i32, i32, i32, i32, i32, i32)> = outputs
        .iter()
        .map(|o| {
            (
                o.effective_x(),
                o.effective_y(),
                o.effective_width(),
                o.effective_height(),
                o.physical_width(),
                o.physical_height(),
            )
        })
        .collect();

    // Process left-to-right, top-to-bottom.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| (mons[i].0, mons[i].1));

    let mut px: Vec<Option<f64>> = vec![None; n];
    let mut py: Vec<Option<f64>> = vec![None; n];

    for &i in &order {
        let (lx, ly, lw, lh, _pw, _ph) = mons[i];

        // X axis: find a left neighbor whose logical right edge == this left edge.
        if px[i].is_none() {
            for &j in &order {
                if j == i {
                    continue;
                }
                if let Some(pxj) = px[j] {
                    let (jlx, jly, jlw, jlh, jpw, _) = mons[j];
                    if jlx + jlw == lx && ranges_overlap(jly, jly + jlh, ly, ly + lh) {
                        px[i] = Some(pxj + f64::from(jpw));
                        break;
                    }
                }
            }
        }

        // Y axis: find a top neighbor whose logical bottom edge == this top edge.
        if py[i].is_none() {
            for &j in &order {
                if j == i {
                    continue;
                }
                if let Some(pyj) = py[j] {
                    let (jlx, jly, jlw, jlh, _, jph) = mons[j];
                    if jly + jlh == ly && ranges_overlap(jlx, jlx + jlw, lx, lx + lw) {
                        py[i] = Some(pyj + f64::from(jph));
                        break;
                    }
                }
            }
        }

        // Fallback: use logical position (correct when scale ≈ 1.0).
        if px[i].is_none() {
            px[i] = Some(f64::from(lx));
        }
        if py[i].is_none() {
            py[i] = Some(f64::from(ly));
        }
    }

    (0..n).map(|i| (px[i].unwrap_or(0.0), py[i].unwrap_or(0.0))).collect()
}

/// Compute the union bounding rectangle across all monitors.
fn union_bounds(monitors: &[MonitorInfo]) -> Rect {
    let Some(first) = monitors.first() else {
        return Rect::new(0.0, 0.0, 0.0, 0.0);
    };

    let mut union = first.bounds;
    for m in &monitors[1..] {
        let x0 = union.x().min(m.bounds.x());
        let y0 = union.y().min(m.bounds.y());
        let x1 = (union.x() + union.width()).max(m.bounds.x() + m.bounds.width());
        let y1 = (union.y() + union.height()).max(m.bounds.y() + m.bounds.height());
        union = Rect::new(x0, y0, x1 - x0, y1 - y0);
    }
    union
}
