use crate::wayland_util::connection;
use platynui_core::platform::{DesktopInfo, DesktopInfoProvider, MonitorInfo, PlatformError, PlatformErrorKind};
use platynui_core::register_desktop_info_provider;
use platynui_core::types::Rect;
use platynui_core::ui::{DESKTOP_RUNTIME_ID, RuntimeId, TechnologyId};
use std::env;
use wayland_client::protocol::{wl_output, wl_registry};
use wayland_client::{Connection as WlConnection, Dispatch, EventQueue, QueueHandle};

pub struct WaylandDesktopInfo;

impl DesktopInfoProvider for WaylandDesktopInfo {
    fn desktop_info(&self) -> Result<DesktopInfo, PlatformError> {
        let monitors = match enumerate_monitors() {
            Ok(ms) if !ms.is_empty() => {
                tracing::debug!(monitor_count = ms.len(), "wl_output monitors enumerated");
                ms
            }
            Ok(_) | Err(_) => {
                tracing::warn!("wl_output enumeration failed — using fallback desktop info");
                return Ok(fallback_desktop_info());
            }
        };

        // Compute union bounds across all monitors.
        let mut iter = monitors.iter();
        let first = iter.next().expect("monitors is non-empty");
        let mut union = first.bounds;
        for m in iter {
            let x0 = union.x().min(m.bounds.x());
            let y0 = union.y().min(m.bounds.y());
            let x1 = (union.x() + union.width()).max(m.bounds.x() + m.bounds.width());
            let y1 = (union.y() + union.height()).max(m.bounds.y() + m.bounds.height());
            union = Rect::new(x0, y0, x1 - x0, y1 - y0);
        }

        let os_name = env::consts::OS.to_string();
        let os_version = env::consts::ARCH.to_string();
        let technology = TechnologyId::from("Wayland");
        let runtime_id = RuntimeId::from(DESKTOP_RUNTIME_ID);

        Ok(DesktopInfo {
            runtime_id,
            name: "Wayland Desktop".into(),
            technology,
            bounds: union,
            os_name,
            os_version,
            monitors,
        })
    }
}

fn fallback_desktop_info() -> DesktopInfo {
    let bounds = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    DesktopInfo {
        runtime_id: RuntimeId::from(DESKTOP_RUNTIME_ID),
        name: "Fallback Desktop".into(),
        technology: TechnologyId::from("Fallback"),
        bounds,
        os_name: env::consts::OS.to_string(),
        os_version: env::consts::ARCH.to_string(),
        monitors: vec![MonitorInfo {
            id: "fallback".into(),
            name: Some("Fallback".into()),
            bounds,
            is_primary: true,
            scale_factor: Some(1.0),
        }],
    }
}

/// State for collecting `wl_output` geometry events during monitor enumeration.
struct OutputState {
    monitors: Vec<MonitorCollector>,
}

struct MonitorCollector {
    id: u32,
    name: Option<String>,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    scale: i32,
    done: bool,
}

impl Dispatch<wl_registry::WlRegistry, ()> for OutputState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _data: &(),
        _conn: &WlConnection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, version } = event
            && interface == "wl_output"
        {
            let _output = registry.bind::<wl_output::WlOutput, _, _>(name, version.min(4), qh, name);
            state.monitors.push(MonitorCollector {
                id: name,
                name: None,
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                scale: 1,
                done: false,
            });
        }
    }
}

impl Dispatch<wl_output::WlOutput, u32> for OutputState {
    fn event(
        state: &mut Self,
        _proxy: &wl_output::WlOutput,
        event: wl_output::Event,
        data: &u32,
        _conn: &WlConnection,
        _qh: &QueueHandle<Self>,
    ) {
        let global_name = *data;
        let Some(monitor) = state.monitors.iter_mut().find(|m| m.id == global_name) else {
            return;
        };

        match event {
            wl_output::Event::Geometry { x, y, .. } => {
                monitor.x = x;
                monitor.y = y;
            }
            wl_output::Event::Mode { flags, width, height, .. } => {
                // Only apply current mode.
                if let wayland_client::WEnum::Value(mode_flags) = flags
                    && mode_flags.contains(wl_output::Mode::Current)
                {
                    monitor.width = width;
                    monitor.height = height;
                }
            }
            wl_output::Event::Scale { factor } => {
                monitor.scale = factor;
            }
            wl_output::Event::Name { name } => {
                monitor.name = Some(name);
            }
            wl_output::Event::Done => {
                monitor.done = true;
            }
            _ => {}
        }
    }
}

/// Enumerate monitors by connecting to the Wayland display and collecting
/// `wl_output` events.
fn enumerate_monitors() -> Result<Vec<MonitorInfo>, PlatformError> {
    // Use the existing shared connection to verify availability, but create
    // a fresh connection for the event queue to avoid locking contention.
    let _guard = connection()?;

    let conn = WlConnection::connect_to_env().map_err(|e| {
        PlatformError::new(PlatformErrorKind::OperationFailed, format!("wayland connect for monitor enum: {e}"))
    })?;
    let display = conn.display();
    let mut event_queue: EventQueue<OutputState> = conn.new_event_queue();
    let qh = event_queue.handle();

    let _registry = display.get_registry(&qh, ());
    let mut state = OutputState { monitors: Vec::new() };

    // First roundtrip: discover globals and bind wl_output objects.
    event_queue
        .blocking_dispatch(&mut state)
        .map_err(|e| PlatformError::new(PlatformErrorKind::OperationFailed, format!("registry roundtrip: {e}")))?;

    // Second roundtrip: collect geometry/mode/done events from bound outputs.
    event_queue
        .blocking_dispatch(&mut state)
        .map_err(|e| PlatformError::new(PlatformErrorKind::OperationFailed, format!("output roundtrip: {e}")))?;

    let monitors: Vec<MonitorInfo> = state
        .monitors
        .into_iter()
        .filter(|m| m.done && m.width > 0 && m.height > 0)
        .enumerate()
        .map(|(i, m)| {
            let scale = if m.scale > 0 { m.scale } else { 1 };
            // wl_output geometry reports physical pixel size; logical = physical / scale.
            let logical_width = f64::from(m.width) / f64::from(scale);
            let logical_height = f64::from(m.height) / f64::from(scale);
            let bounds = Rect::new(f64::from(m.x), f64::from(m.y), logical_width, logical_height);
            let id = m.name.clone().unwrap_or_else(|| format!("{}x{}@{},{}", m.width, m.height, m.x, m.y));
            MonitorInfo {
                id,
                name: m.name,
                bounds,
                is_primary: i == 0, // First output is treated as primary.
                scale_factor: Some(f64::from(scale)),
            }
        })
        .collect();

    Ok(monitors)
}

static DESKTOP: WaylandDesktopInfo = WaylandDesktopInfo;
register_desktop_info_provider!(&DESKTOP);
