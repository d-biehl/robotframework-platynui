use crate::x11util::{connection, root_window_from};
use platynui_core::platform::{DesktopInfo, DesktopInfoProvider, MonitorInfo, PlatformError};
use platynui_core::register_desktop_info_provider;
use platynui_core::types::Rect;
use platynui_core::ui::{DESKTOP_RUNTIME_ID, RuntimeId, TechnologyId};
use std::env;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::xproto::ConnectionExt as _;

pub struct LinuxDesktopInfo;

impl DesktopInfoProvider for LinuxDesktopInfo {
    fn desktop_info(&self) -> Result<DesktopInfo, PlatformError> {
        let guard = match connection() {
            Ok(g) => g,
            Err(_) => {
                tracing::warn!("X11 connection failed — using fallback desktop info");
                // Fallback when no X11 display is available
                let bounds = Rect::new(0.0, 0.0, 1920.0, 1080.0);
                let os_name = env::consts::OS.to_string();
                let os_version = env::consts::ARCH.to_string();
                let technology = TechnologyId::from("Fallback");
                let runtime_id = RuntimeId::from(DESKTOP_RUNTIME_ID);
                let monitors = vec![MonitorInfo {
                    id: "fallback".into(),
                    name: Some("Fallback".into()),
                    bounds,
                    is_primary: true,
                    scale_factor: Some(1.0),
                }];
                return Ok(DesktopInfo {
                    runtime_id,
                    name: "Fallback Desktop".into(),
                    technology,
                    bounds,
                    os_name,
                    os_version,
                    monitors,
                });
            }
        };
        let root = root_window_from(&guard);
        let (bounds, monitors) = match monitors_via_randr(&guard.conn, root) {
            Ok(ms) if !ms.is_empty() => {
                tracing::debug!(monitor_count = ms.len(), "RANDR monitors enumerated");
                // Compute union bounds across monitors
                let mut it = ms.iter();
                // SAFETY: the `!ms.is_empty()` guard above ensures at least one element
                let first = it.next().unwrap_or_else(|| unreachable!());
                let mut union = first.bounds;
                for m in it {
                    let x0 = union.x().min(m.bounds.x());
                    let y0 = union.y().min(m.bounds.y());
                    let x1 = (union.x() + union.width()).max(m.bounds.x() + m.bounds.width());
                    let y1 = (union.y() + union.height()).max(m.bounds.y() + m.bounds.height());
                    union = Rect::new(x0, y0, x1 - x0, y1 - y0);
                }
                (union, ms)
            }
            _ => {
                tracing::debug!("RANDR unavailable or empty — falling back to root window geometry");
                // Fallback to root geometry as a single monitor
                let geom = guard.conn.get_geometry(root).map_err(to_pf)?.reply().map_err(to_pf)?;
                let width = f64::from(geom.width);
                let height = f64::from(geom.height);
                let b = Rect::new(0.0, 0.0, width, height);
                let ms = vec![MonitorInfo {
                    id: "root".into(),
                    name: Some("Root".into()),
                    bounds: b,
                    is_primary: true,
                    scale_factor: Some(1.0),
                }];
                (b, ms)
            }
        };
        let os_name = env::consts::OS.to_string();
        let os_version = env::consts::ARCH.to_string();
        let technology = TechnologyId::from("X11");
        let runtime_id = RuntimeId::from(DESKTOP_RUNTIME_ID);
        Ok(DesktopInfo { runtime_id, name: "X11 Desktop".into(), technology, bounds, os_name, os_version, monitors })
    }
}

fn to_pf<E: std::fmt::Display>(e: E) -> PlatformError {
    // Desktop queries past connect are operational (e.g., RANDR unavailable).
    platynui_core::platform::PlatformError::new(
        platynui_core::platform::PlatformErrorKind::OperationFailed,
        format!("x11: {e}"),
    )
}

static DESKTOP: LinuxDesktopInfo = LinuxDesktopInfo;
register_desktop_info_provider!(&DESKTOP);

fn monitors_via_randr<C: x11rb::connection::Connection>(conn: &C, root: u32) -> Result<Vec<MonitorInfo>, String> {
    // Try RANDR 1.5 get_monitors first
    if let Ok(ver_cookie) = conn.randr_query_version(1, 5)
        && ver_cookie.reply().is_ok()
        && let Ok(mon_cookie) = conn.randr_get_monitors(root, true)
        && let Ok(reply) = mon_cookie.reply()
    {
        let mut out = Vec::new();
        for m in reply.monitors {
            let id = format!("{}x{}@{},{}", m.width, m.height, m.x, m.y);
            let bounds = Rect::new(m.x.into(), m.y.into(), m.width.into(), m.height.into());
            out.push(MonitorInfo { id, name: None, bounds, is_primary: m.primary, scale_factor: None });
        }
        return Ok(out);
    }

    // Fallback: RANDR <=1.4 via screen resources / crtcs
    if let Ok(res_cookie) = conn.randr_get_screen_resources_current(root)
        && let Ok(res) = res_cookie.reply()
    {
        let mut out = Vec::new();
        for crtc in res.crtcs {
            if let Ok(info_cookie) = conn.randr_get_crtc_info(crtc, 0)
                && let Ok(info) = info_cookie.reply()
            {
                if info.width == 0 || info.height == 0 {
                    continue;
                }
                let bounds = Rect::new(info.x.into(), info.y.into(), info.width.into(), info.height.into());
                let id = format!("CRTC-{}:{}x{}@{},{}", crtc, info.width, info.height, info.x, info.y);
                out.push(MonitorInfo { id, name: None, bounds, is_primary: false, scale_factor: None });
            }
        }
        return Ok(out);
    }

    Err("RANDR unavailable".into())
}
