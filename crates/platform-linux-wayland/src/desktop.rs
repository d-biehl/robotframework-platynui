//! Wayland desktop information provider.
//!
//! Reads monitor data collected during initialization (via `wl_output` +
//! `xdg-output-manager`) and returns a [`DesktopInfo`] with real bounds.

use std::env;

use platynui_core::platform::{DesktopInfo, DesktopInfoProvider, MonitorInfo, PlatformError};
use platynui_core::types::Rect;
use platynui_core::ui::{DESKTOP_RUNTIME_ID, RuntimeId, TechnologyId};

use crate::connection;

pub struct WaylandDesktopInfo;

impl DesktopInfoProvider for WaylandDesktopInfo {
    fn desktop_info(&self) -> Result<DesktopInfo, PlatformError> {
        let outputs = connection::outputs();

        let monitors: Vec<MonitorInfo> = outputs
            .iter()
            .enumerate()
            .map(|(i, o)| {
                let x = f64::from(o.effective_x());
                let y = f64::from(o.effective_y());
                let w = f64::from(o.effective_width());
                let h = f64::from(o.effective_height());
                let bounds = Rect::new(x, y, w, h);
                let id = o.name.clone().unwrap_or_else(|| format!("wl-output-{i}"));

                MonitorInfo {
                    id,
                    name: o.description.clone(),
                    bounds,
                    is_primary: i == 0,
                    scale_factor: Some(f64::from(o.scale)),
                }
            })
            .collect();

        let bounds = union_bounds(&monitors);

        Ok(DesktopInfo {
            runtime_id: RuntimeId::from(DESKTOP_RUNTIME_ID),
            name: "Wayland Desktop".into(),
            technology: TechnologyId::from("Wayland"),
            bounds,
            os_name: env::consts::OS.to_string(),
            os_version: env::consts::ARCH.to_string(),
            monitors,
        })
    }
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
