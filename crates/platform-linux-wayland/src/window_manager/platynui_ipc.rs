//! Control-socket-backed window manager for the `PlatynUI` compositor.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

use platynui_core::platform::{PlatformError, WindowHit, WindowId};
use platynui_core::types::{Point, Rect, Size};
use platynui_core::ui::{Namespace, PatternName, UiNode, pattern_names};
use serde_json::{Value, json};

use super::CompositorBackend;

#[derive(Clone, Debug)]
struct WindowSelector {
    compositor_window_id: u64,
    title: String,
    pid: Option<u32>,
    /// AT-SPI size of the node, used to re-disambiguate on refresh (see
    /// [`match_best_window`]) when the title alone is not enough.
    size: Option<(f64, f64)>,
}

#[derive(Clone, Debug, PartialEq)]
struct ControlWindowInfo {
    window_id: u64,
    title: String,
    app_id: String,
    pid: Option<u32>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    content_x: f64,
    content_y: f64,
    content_width: f64,
    content_height: f64,
    /// CSD geometry offset (shadow inset) within the surface buffer.
    geometry_x: f64,
    geometry_y: f64,
    focused: bool,
    /// Decoration mode reported by the compositor: `"csd"` or `"ssd"`.
    decoration_mode: Option<String>,
    opaque_region: Option<Vec<OpaqueRect>>,
}

/// A rectangle from the surface opaque region reported by the compositor.
#[derive(Clone, Debug, PartialEq)]
struct OpaqueRect {
    kind: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

pub(crate) struct PlatynUiIpcBackend;

static NEXT_WINDOW_ID: AtomicU64 = AtomicU64::new(1);
static RESOLVED_WINDOWS: LazyLock<Mutex<HashMap<u64, WindowSelector>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

impl CompositorBackend for PlatynUiIpcBackend {
    fn name(&self) -> &'static str {
        "Wayland (PlatynUI IPC)"
    }

    fn resolve_window(&self, node: &dyn UiNode) -> Result<WindowId, PlatformError> {
        let pid = extract_pid(node);
        let title = extract_window_title(node);
        // AT-SPI reports a correct size even on Wayland (only the position is
        // unavailable, i.e. 0,0), so size is the usable geometric key to
        // disambiguate windows whose accessible name differs from their
        // window title. Read recursion-safely via the raw native attribute.
        let node_size = extract_screen_size(node);
        let window = match_best_window(&list_windows()?, pid, &title, node_size).ok_or_else(|| {
            PlatformError::OperationFailed {
                operation: "resolve window via control socket",
                details: Some(format!("no matching window found for pid={pid:?}, title={title:?}, size={node_size:?}")),
            }
        })?;

        let local_id = NEXT_WINDOW_ID.fetch_add(1, Ordering::Relaxed);
        RESOLVED_WINDOWS
            .lock()
            .expect("resolved windows mutex poisoned")
            .insert(local_id, WindowSelector { compositor_window_id: window.window_id, title, pid, size: node_size });
        Ok(WindowId::new(local_id))
    }

    fn bounds(&self, id: WindowId, toolkit_hint: Option<&str>) -> Result<Rect, PlatformError> {
        let info = resolve_window_info(id)?;

        // The compositor's element_location (reported as content_x/y) is
        // the buffer origin — where the wl_surface is placed on screen.
        // For CSD windows the buffer includes transparent shadow area, so
        // the actual content starts further right/down by the shadow inset.
        //
        // We derive this offset from the opaque region (surface-local coords
        // from the client) with geometry_offset as fallback.  The opaque
        // region's origin is at least as large as the geometry offset; for
        // the Y axis they typically match, while on X the opaque region may
        // be indented further due to rounded corners.  We use the minimum
        // per axis to get the content origin, not the opaque origin.
        let (offset_x, offset_y) = csd_shadow_offset(&info, toolkit_hint);

        tracing::debug!(
            window_id = info.window_id,
            title = %info.title,
            buffer_x = info.content_x,
            buffer_y = info.content_y,
            content_width = info.content_width,
            content_height = info.content_height,
            geometry_x = info.geometry_x,
            geometry_y = info.geometry_y,
            offset_x,
            offset_y,
            opaque_region = ?info.opaque_region,
            "window bounds computation"
        );

        Ok(Rect::new(info.content_x + offset_x, info.content_y + offset_y, info.content_width, info.content_height))
    }

    fn is_active(&self, id: WindowId) -> Result<bool, PlatformError> {
        Ok(resolve_window_info(id)?.focused)
    }

    fn activate(&self, id: WindowId) -> Result<(), PlatformError> {
        let selector = current_selector(id)?;
        let _ = send_command(&json!({"command": "focus_window", "window_id": selector.compositor_window_id}))?;
        Ok(())
    }

    fn close(&self, id: WindowId) -> Result<(), PlatformError> {
        let selector = current_selector(id)?;
        let _ = send_command(&json!({"command": "close_window", "window_id": selector.compositor_window_id}))?;
        Ok(())
    }

    fn minimize(&self, id: WindowId) -> Result<(), PlatformError> {
        let selector = current_selector(id)?;
        let _ = send_command(&json!({"command": "minimize_window", "window_id": selector.compositor_window_id}))?;
        Ok(())
    }

    fn maximize(&self, id: WindowId) -> Result<(), PlatformError> {
        let selector = current_selector(id)?;
        let _ = send_command(&json!({"command": "maximize_window", "window_id": selector.compositor_window_id}))?;
        Ok(())
    }

    fn restore(&self, id: WindowId) -> Result<(), PlatformError> {
        let selector = current_selector(id)?;
        let _ = send_command(&json!({"command": "restore_window", "window_id": selector.compositor_window_id}))?;
        Ok(())
    }

    fn move_to(&self, id: WindowId, position: Point) -> Result<(), PlatformError> {
        let selector = current_selector(id)?;
        let _ = send_command(
            &json!({"command": "move_window", "window_id": selector.compositor_window_id, "x": position.x(), "y": position.y()}),
        )?;
        Ok(())
    }

    fn resize(&self, id: WindowId, size: Size) -> Result<(), PlatformError> {
        let selector = current_selector(id)?;
        let _ = send_command(&json!({
            "command": "resize_window",
            "window_id": selector.compositor_window_id,
            "width": size.width(),
            "height": size.height()
        }))?;
        Ok(())
    }

    fn window_at_point(&self, point: Point) -> Result<Option<WindowHit>, PlatformError> {
        let response = send_command(&json!({"command": "window_at_point", "x": point.x(), "y": point.y()}))?;
        // The compositor returns `window: null` when no window covers the point.
        match response.get("window") {
            None | Some(Value::Null) => Ok(None),
            Some(_) => {
                let info = decode_window_response(&response)?;
                let (offset_x, offset_y) = csd_shadow_offset(&info, None);
                let bounds = Rect::new(
                    info.content_x + offset_x,
                    info.content_y + offset_y,
                    info.content_width,
                    info.content_height,
                );
                // Register a selector so the returned WindowId is usable with
                // the other window-manager operations (bounds/activate/...).
                let local_id = NEXT_WINDOW_ID.fetch_add(1, Ordering::Relaxed);
                RESOLVED_WINDOWS.lock().expect("resolved windows mutex poisoned").insert(
                    local_id,
                    WindowSelector {
                        compositor_window_id: info.window_id,
                        title: info.title.clone(),
                        pid: info.pid,
                        size: Some((info.content_width, info.content_height)),
                    },
                );
                Ok(Some(WindowHit { id: WindowId::new(local_id), pid: info.pid, bounds }))
            }
        }
    }
}

fn resolve_window_info(id: WindowId) -> Result<ControlWindowInfo, PlatformError> {
    let selector = current_selector(id)?;

    let direct = send_command(&json!({"command": "get_window", "window_id": selector.compositor_window_id}))
        .and_then(|value| decode_window_response(&value));
    if let Ok(info) = direct {
        return Ok(info);
    }

    let window =
        match_best_window(&list_windows()?, selector.pid, &selector.title, selector.size).ok_or_else(|| {
            PlatformError::OperationFailed {
                operation: "refresh window via control socket",
                details: Some(format!("window {id:?} is no longer present")),
            }
        })?;

    RESOLVED_WINDOWS.lock().expect("resolved windows mutex poisoned").insert(
        id.raw(),
        WindowSelector {
            compositor_window_id: window.window_id,
            title: selector.title,
            pid: selector.pid,
            size: selector.size,
        },
    );

    Ok(window)
}

fn current_selector(id: WindowId) -> Result<WindowSelector, PlatformError> {
    RESOLVED_WINDOWS.lock().expect("resolved windows mutex poisoned").get(&id.raw()).cloned().ok_or_else(|| {
        PlatformError::OperationFailed {
            operation: "lookup cached Wayland window",
            details: Some(format!("unknown WindowId {id}")),
        }
    })
}

fn list_windows() -> Result<Vec<ControlWindowInfo>, PlatformError> {
    let response = send_command(&json!({"command": "list_windows"}))?;
    let Some(windows) = response.get("windows").and_then(Value::as_array) else {
        return Err(PlatformError::OperationFailed {
            operation: "decode list_windows response",
            details: Some("missing windows array".into()),
        });
    };

    windows.iter().map(decode_window).collect()
}

/// Max per-axis difference (px) between a node's AT-SPI size and a compositor
/// window's content size for the two to be considered the same window. The two
/// describe the same client area, so the match is near-exact; the tolerance only
/// absorbs rounding.
const SIZE_MATCH_TOLERANCE: f64 = 2.0;

fn match_best_window(
    windows: &[ControlWindowInfo],
    pid: Option<u32>,
    title: &str,
    node_size: Option<(f64, f64)>,
) -> Option<ControlWindowInfo> {
    let pid_candidates: Vec<&ControlWindowInfo> = match pid {
        Some(pid) => windows.iter().filter(|window| window.pid == Some(pid)).collect(),
        None => Vec::new(),
    };

    let candidates: &[&ControlWindowInfo] = if pid_candidates.is_empty() { &[] } else { &pid_candidates };

    if !title.is_empty() {
        let exact = if candidates.is_empty() {
            windows.iter().find(|window| window.title == title)
        } else {
            candidates.iter().copied().find(|window| window.title == title)
        };
        if let Some(window) = exact {
            return Some(window.clone());
        }

        let title_lower = title.to_ascii_lowercase();
        let contains = if candidates.is_empty() {
            windows.iter().find(|window| window.title.to_ascii_lowercase().contains(&title_lower))
        } else {
            candidates.iter().copied().find(|window| window.title.to_ascii_lowercase().contains(&title_lower))
        };
        if let Some(window) = contains {
            return Some(window.clone());
        }
    }

    // Title did not disambiguate (e.g. the node's accessible name differs from
    // the window title): fall back to correlating by size. Only accept a UNIQUE
    // size match, so we never guess between equally-sized windows.
    if let Some((w, h)) = node_size {
        let pool: Vec<&ControlWindowInfo> =
            if candidates.is_empty() { windows.iter().collect() } else { candidates.to_vec() };
        let mut size_matches = pool.into_iter().filter(|window| {
            (window.content_width - w).abs() <= SIZE_MATCH_TOLERANCE
                && (window.content_height - h).abs() <= SIZE_MATCH_TOLERANCE
        });
        if let Some(window) = size_matches.next()
            && size_matches.next().is_none()
        {
            return Some(window.clone());
        }
    }

    if candidates.len() == 1 {
        return Some(candidates[0].clone());
    }

    if pid.is_none() && windows.len() == 1 {
        return Some(windows[0].clone());
    }

    None
}

fn extract_window_title(node: &dyn UiNode) -> String {
    if is_window_surface(node) {
        let name = node.name();
        if !name.is_empty() {
            return name;
        }
    }

    let mut current = node.parent().and_then(|parent| parent.upgrade());
    while let Some(parent) = current {
        if is_window_surface(parent.as_ref()) {
            let name = parent.name();
            if !name.is_empty() {
                return name;
            }
        }
        current = parent.parent().and_then(|next| next.upgrade());
    }

    node.name()
}

fn is_window_surface(node: &dyn UiNode) -> bool {
    matches!(node.role(), "Frame" | "Window" | "Dialog")
        || node.supported_patterns().iter().any(|pattern| pattern == &PatternName::from(pattern_names::ACTIVATABLE))
}

fn extract_pid(node: &dyn UiNode) -> Option<u32> {
    if let Some(pid) = pid_from_attr(node) {
        return Some(pid);
    }

    let mut current = node.parent().and_then(|parent| parent.upgrade());
    while let Some(parent) = current {
        if let Some(pid) = pid_from_attr(parent.as_ref()) {
            return Some(pid);
        }
        current = parent.parent().and_then(|next| next.upgrade());
    }

    None
}

/// Read the node's AT-SPI screen extents size via the raw
/// `Component.Extents.Screen` native attribute. This goes straight to the
/// accessibility provider (not through this window manager), so it is safe to
/// call from `resolve_window` without recursing. Returns `(width, height)`;
/// `None` when unavailable or degenerate. On Wayland the position is (0,0) but
/// the size is correct, which is why size is the usable geometric key here.
fn extract_screen_size(node: &dyn UiNode) -> Option<(f64, f64)> {
    let attr = node.attribute(Namespace::Native, "Component.Extents.Screen")?;
    match attr.value() {
        platynui_core::ui::UiValue::Rect(rect) if rect.width() > 0.0 && rect.height() > 0.0 => {
            Some((rect.width(), rect.height()))
        }
        _ => None,
    }
}

fn pid_from_attr(node: &dyn UiNode) -> Option<u32> {
    let attr = node.attribute(Namespace::Control, "ProcessId")?;
    match attr.value() {
        platynui_core::ui::UiValue::Integer(value) => u32::try_from(value).ok(),
        platynui_core::ui::UiValue::Number(value) => {
            if value.is_finite() && value.fract() == 0.0 && value >= 0.0 && value <= f64::from(u32::MAX) {
                value.to_string().parse::<u32>().ok()
            } else {
                None
            }
        }
        platynui_core::ui::UiValue::String(value) => value.parse::<u32>().ok(),
        _ => None,
    }
}

fn send_command(command: &Value) -> Result<Value, PlatformError> {
    crate::control_ipc::send_command(command, "control socket request")
}

fn decode_window_response(value: &Value) -> Result<ControlWindowInfo, PlatformError> {
    let window = value.get("window").ok_or_else(|| PlatformError::OperationFailed {
        operation: "decode get_window response",
        details: Some("missing window object".into()),
    })?;
    decode_window(window)
}

fn decode_window(value: &Value) -> Result<ControlWindowInfo, PlatformError> {
    Ok(ControlWindowInfo {
        window_id: value.get("window_id").and_then(Value::as_u64).ok_or_else(|| PlatformError::OperationFailed {
            operation: "decode control window",
            details: Some("missing window_id".into()),
        })?,
        title: value.get("title").and_then(Value::as_str).unwrap_or_default().to_string(),
        app_id: value.get("app_id").and_then(Value::as_str).unwrap_or_default().to_string(),
        pid: value.get("pid").and_then(Value::as_u64).and_then(|pid| u32::try_from(pid).ok()),
        x: value.get("x").and_then(Value::as_f64).unwrap_or(0.0),
        y: value.get("y").and_then(Value::as_f64).unwrap_or(0.0),
        width: value.get("width").and_then(Value::as_f64).unwrap_or(0.0),
        height: value.get("height").and_then(Value::as_f64).unwrap_or(0.0),
        content_x: value
            .get("content_x")
            .and_then(Value::as_f64)
            .or_else(|| value.get("x").and_then(Value::as_f64))
            .unwrap_or(0.0),
        content_y: value
            .get("content_y")
            .and_then(Value::as_f64)
            .or_else(|| value.get("y").and_then(Value::as_f64))
            .unwrap_or(0.0),
        content_width: value
            .get("content_width")
            .and_then(Value::as_f64)
            .or_else(|| value.get("width").and_then(Value::as_f64))
            .unwrap_or(0.0),
        content_height: value
            .get("content_height")
            .and_then(Value::as_f64)
            .or_else(|| value.get("height").and_then(Value::as_f64))
            .unwrap_or(0.0),
        geometry_x: value.get("geometry_x").and_then(Value::as_f64).unwrap_or(0.0),
        geometry_y: value.get("geometry_y").and_then(Value::as_f64).unwrap_or(0.0),
        focused: value.get("focused").and_then(Value::as_bool).unwrap_or(false),
        decoration_mode: value.get("decoration_mode").and_then(Value::as_str).map(String::from),
        opaque_region: decode_opaque_region(value),
    })
}

fn decode_opaque_region(value: &Value) -> Option<Vec<OpaqueRect>> {
    let arr = value.get("opaque_region")?.as_array()?;
    let rects: Vec<OpaqueRect> = arr
        .iter()
        .filter_map(|r| {
            Some(OpaqueRect {
                kind: r.get("kind")?.as_str()?.to_string(),
                x: r.get("x")?.as_f64()?,
                y: r.get("y")?.as_f64()?,
                width: r.get("width")?.as_f64()?,
                height: r.get("height")?.as_f64()?,
            })
        })
        .collect();
    if rects.is_empty() { None } else { Some(rects) }
}

/// Compute the CSD shadow offset to add to the buffer origin to find
/// the actual content origin.
///
/// For CSD windows, the compositor places the surface buffer on screen
/// at `element_location`. The content area (xdg geometry) starts further
/// inside the buffer by the shadow inset. This function returns that inset.
///
/// Primary source: the opaque region's origin (surface-local coords from
/// the client). Using `min(opaque, geometry)` per axis avoids over-correcting
/// when the opaque rectangle is further indented by rounded corners.
///
/// Fallback: the geometry offset reported by the compositor.
/// Returns `(0.0, 0.0)` for SSD windows.
fn csd_shadow_offset(info: &ControlWindowInfo, toolkit_hint: Option<&str>) -> (f64, f64) {
    // Only apply the shadow offset for GTK4 CSD windows.  GTK4 includes a
    // transparent shadow in the surface buffer and AT-SPI coordinates don't
    // account for it.  Other toolkits (Qt, etc.) report correct AT-SPI
    // extents even with CSD, so we must not apply the offset for them.
    if info.decoration_mode.as_deref() != Some("csd") || toolkit_hint != Some("gtk4") {
        return (0.0, 0.0);
    }

    if let Some(ref rects) = info.opaque_region
        && let Some(first_add) = rects.iter().find(|r| r.kind == "add")
    {
        // Opaque origin may exceed geometry (e.g. rounded corners add
        // extra horizontal inset). Take the minimum per axis so we land
        // at the geometry/content origin, not the opaque-interior origin.
        let ox = if info.geometry_x > 0.0 { info.geometry_x.min(first_add.x) } else { first_add.x };
        let oy = if info.geometry_y > 0.0 { info.geometry_y.min(first_add.y) } else { first_add.y };
        return (ox, oy);
    }
    (info.geometry_x, info.geometry_y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_best_window_prefers_pid_and_exact_title() {
        let windows = vec![
            ControlWindowInfo {
                window_id: 1,
                title: "First".into(),
                app_id: "demo".into(),
                pid: Some(10),
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
                content_x: 0.0,
                content_y: 0.0,
                content_width: 100.0,
                content_height: 100.0,
                geometry_x: 0.0,
                geometry_y: 0.0,
                focused: false,
                decoration_mode: None,
                opaque_region: None,
            },
            ControlWindowInfo {
                window_id: 2,
                title: "Target".into(),
                app_id: "demo".into(),
                pid: Some(11),
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
                content_x: 0.0,
                content_y: 0.0,
                content_width: 100.0,
                content_height: 100.0,
                geometry_x: 0.0,
                geometry_y: 0.0,
                focused: true,
                decoration_mode: None,
                opaque_region: None,
            },
        ];

        let matched = match_best_window(&windows, Some(11), "Target", None).expect("expected match");
        assert_eq!(matched.window_id, 2);
    }

    #[test]
    fn match_best_window_falls_back_to_size_when_title_diverges() {
        // Two windows share a PID; the node's accessible name ("child-dialog-1")
        // matches neither window title. Size then uniquely selects the dialog.
        let windows = vec![
            ControlWindowInfo {
                window_id: 1,
                title: "PlatynUI Test App (Qt)".into(),
                app_id: "org.platynui.test.qt".into(),
                pid: Some(42),
                x: 0.0,
                y: 0.0,
                width: 900.0,
                height: 640.0,
                content_x: 0.0,
                content_y: 0.0,
                content_width: 900.0,
                content_height: 640.0,
                geometry_x: 0.0,
                geometry_y: 0.0,
                focused: false,
                decoration_mode: None,
                opaque_region: None,
            },
            ControlWindowInfo {
                window_id: 2,
                title: "Dialog 1".into(),
                app_id: "org.platynui.test.qt".into(),
                pid: Some(42),
                x: 0.0,
                y: 0.0,
                width: 260.0,
                height: 180.0,
                content_x: 0.0,
                content_y: 0.0,
                content_width: 260.0,
                content_height: 180.0,
                geometry_x: 0.0,
                geometry_y: 0.0,
                focused: false,
                decoration_mode: None,
                opaque_region: None,
            },
        ];

        let matched = match_best_window(&windows, Some(42), "child-dialog-1", Some((260.0, 180.0)))
            .expect("expected size-based match");
        assert_eq!(matched.window_id, 2);

        // Two windows of the SAME size must NOT be guessed by size alone.
        let mut same_size = windows.clone();
        same_size[0].content_width = 260.0;
        same_size[0].content_height = 180.0;
        assert!(
            match_best_window(&same_size, Some(42), "child-dialog-1", Some((260.0, 180.0))).is_none(),
            "must not guess between equally-sized windows"
        );
    }

    #[test]
    fn decode_window_parses_stable_window_id() {
        let value = json!({
            "window_id": 42,
            "title": "Example",
            "app_id": "demo.app",
            "pid": 1234,
            "x": 10,
            "y": 20,
            "width": 800,
            "height": 600,
            "content_x": 14,
            "content_y": 26,
            "content_width": 790,
            "content_height": 590,
            "focused": true
        });

        let decoded = decode_window(&value).expect("expected decoded window");
        assert_eq!(decoded.window_id, 42);
        assert_eq!(decoded.pid, Some(1234));
        assert_eq!(decoded.title, "Example");
        assert!((decoded.content_x - 14.0).abs() < f64::EPSILON);
        assert!((decoded.content_y - 26.0).abs() < f64::EPSILON);
        assert!((decoded.content_width - 790.0).abs() < f64::EPSILON);
        assert!((decoded.content_height - 590.0).abs() < f64::EPSILON);
        assert!((decoded.geometry_x - 0.0).abs() < f64::EPSILON);
        assert!((decoded.geometry_y - 0.0).abs() < f64::EPSILON);
        assert!(decoded.decoration_mode.is_none());
    }

    #[test]
    fn decode_window_uses_frame_bounds_as_content_fallback() {
        let value = json!({
            "window_id": 7,
            "title": "Fallback",
            "x": 30,
            "y": 40,
            "width": 500,
            "height": 300
        });

        let decoded = decode_window(&value).expect("expected decoded window");
        assert!((decoded.content_x - 30.0).abs() < f64::EPSILON);
        assert!((decoded.content_y - 40.0).abs() < f64::EPSILON);
        assert!((decoded.content_width - 500.0).abs() < f64::EPSILON);
        assert!((decoded.content_height - 300.0).abs() < f64::EPSILON);
        assert!((decoded.geometry_x - 0.0).abs() < f64::EPSILON);
        assert!((decoded.geometry_y - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn decode_window_parses_geometry_offset() {
        let value = json!({
            "window_id": 99,
            "title": "CSD Window",
            "app_id": "gtk4.app",
            "x": 106,
            "y": 108,
            "width": 800,
            "height": 600,
            "content_x": 100,
            "content_y": 100,
            "content_width": 800,
            "content_height": 600,
            "geometry_x": 6,
            "geometry_y": 8,
            "focused": false
        });

        let decoded = decode_window(&value).expect("expected decoded window");
        assert!((decoded.geometry_x - 6.0).abs() < f64::EPSILON);
        assert!((decoded.geometry_y - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn decode_window_parses_decoration_mode() {
        let value = json!({
            "window_id": 50,
            "title": "CSD",
            "x": 0,
            "y": 0,
            "width": 800,
            "height": 600,
            "decoration_mode": "csd"
        });

        let decoded = decode_window(&value).expect("expected decoded window");
        assert_eq!(decoded.decoration_mode.as_deref(), Some("csd"));
    }

    #[test]
    fn csd_shadow_offset_uses_min_of_opaque_and_geometry() {
        // GTK4 CSD: opaque_x > geometry_x due to rounded corners
        let info = ControlWindowInfo {
            window_id: 1,
            title: "test".into(),
            app_id: "test".into(),
            pid: None,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            content_x: 761.0,
            content_y: 88.0,
            content_width: 1360.0,
            content_height: 346.0,
            geometry_x: 25.0,
            geometry_y: 25.0,
            focused: false,
            decoration_mode: Some("csd".into()),
            opaque_region: Some(vec![OpaqueRect {
                kind: "add".into(),
                x: 40.0,
                y: 25.0,
                width: 1330.0,
                height: 346.0,
            }]),
        };
        let (ox, oy) = csd_shadow_offset(&info, Some("gtk4"));
        // X: min(geometry=25, opaque=40) = 25  (geometry wins, avoids rounded-corner overcount)
        assert!((ox - 25.0).abs() < f64::EPSILON);
        // Y: min(geometry=25, opaque=25) = 25
        assert!((oy - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn csd_shadow_offset_uses_opaque_when_geometry_is_zero() {
        // Toolkit that doesn't set geometry offset but does set opaque region
        let info = ControlWindowInfo {
            window_id: 1,
            title: "test".into(),
            app_id: "test".into(),
            pid: None,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            content_x: 100.0,
            content_y: 100.0,
            content_width: 800.0,
            content_height: 600.0,
            geometry_x: 0.0,
            geometry_y: 0.0,
            focused: false,
            decoration_mode: Some("csd".into()),
            opaque_region: Some(vec![OpaqueRect { kind: "add".into(), x: 10.0, y: 12.0, width: 780.0, height: 576.0 }]),
        };
        let (ox, oy) = csd_shadow_offset(&info, Some("gtk4"));
        assert!((ox - 10.0).abs() < f64::EPSILON);
        assert!((oy - 12.0).abs() < f64::EPSILON);
    }

    #[test]
    fn csd_shadow_offset_falls_back_to_geometry_without_opaque() {
        let info = ControlWindowInfo {
            window_id: 1,
            title: "test".into(),
            app_id: "test".into(),
            pid: None,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            content_x: 100.0,
            content_y: 100.0,
            content_width: 800.0,
            content_height: 600.0,
            geometry_x: 6.0,
            geometry_y: 8.0,
            focused: false,
            decoration_mode: Some("csd".into()),
            opaque_region: None,
        };
        let (ox, oy) = csd_shadow_offset(&info, Some("gtk4"));
        assert!((ox - 6.0).abs() < f64::EPSILON);
        assert!((oy - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn csd_shadow_offset_returns_zero_for_ssd() {
        let info = ControlWindowInfo {
            window_id: 1,
            title: "test".into(),
            app_id: "test".into(),
            pid: None,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            content_x: 100.0,
            content_y: 100.0,
            content_width: 800.0,
            content_height: 600.0,
            geometry_x: 0.0,
            geometry_y: 0.0,
            focused: false,
            decoration_mode: Some("ssd".into()),
            opaque_region: None,
        };
        let (ox, oy) = csd_shadow_offset(&info, Some("gtk4"));
        assert!((ox - 0.0).abs() < f64::EPSILON);
        assert!((oy - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn csd_shadow_offset_returns_zero_for_non_gtk4_csd() {
        // Qt6 CSD window with geometry offset — should NOT get the shadow correction
        let info = ControlWindowInfo {
            window_id: 1,
            title: "test".into(),
            app_id: "test".into(),
            pid: None,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            content_x: 100.0,
            content_y: 100.0,
            content_width: 800.0,
            content_height: 600.0,
            geometry_x: 10.0,
            geometry_y: 10.0,
            focused: false,
            decoration_mode: Some("csd".into()),
            opaque_region: Some(vec![OpaqueRect { kind: "add".into(), x: 10.0, y: 10.0, width: 780.0, height: 580.0 }]),
        };
        let (ox, oy) = csd_shadow_offset(&info, Some("qt6"));
        assert!((ox - 0.0).abs() < f64::EPSILON);
        assert!((oy - 0.0).abs() < f64::EPSILON);
    }
}
