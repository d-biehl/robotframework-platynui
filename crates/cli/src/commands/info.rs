use crate::OutputFormat;
use crate::util::{CliResult, yes_no};
use platynui_core::platform::{DesktopInfo, MonitorInfo};
use platynui_core::types::Rect;
use platynui_runtime::Runtime;
use serde::Serialize;
use std::fmt::Write;

#[derive(Serialize, Debug, PartialEq)]
struct MonitorSummary {
    id: String,
    name: Option<String>,
    bounds: Rect,
    is_primary: bool,
    scale_factor: Option<f64>,
}

#[derive(Serialize, Debug, PartialEq)]
struct DesktopSummary {
    runtime_id: String,
    name: String,
    technology: String,
    bounds: Rect,
    os_name: String,
    os_version: String,
    display_count: usize,
    monitors: Vec<MonitorSummary>,
}

pub fn run(runtime: &Runtime, format: OutputFormat) -> CliResult<String> {
    let summary = DesktopSummary::from_info(runtime.desktop_info());
    let output = match format {
        OutputFormat::Text => render_info_text(&summary),
        OutputFormat::Json => render_info_json(&summary)?,
    };
    Ok(output)
}

impl DesktopSummary {
    fn from_info(info: &DesktopInfo) -> Self {
        Self {
            runtime_id: info.runtime_id.as_str().to_owned(),
            name: info.name.clone(),
            technology: info.technology.as_str().to_owned(),
            bounds: info.bounds,
            os_name: info.os_name.clone(),
            os_version: info.os_version.clone(),
            display_count: info.display_count(),
            monitors: info.monitors.iter().map(MonitorSummary::from_monitor).collect(),
        }
    }
}

impl MonitorSummary {
    fn from_monitor(monitor: &MonitorInfo) -> Self {
        Self {
            id: monitor.id.clone(),
            name: monitor.name.clone(),
            bounds: monitor.bounds,
            is_primary: monitor.is_primary,
            scale_factor: monitor.scale_factor,
        }
    }
}

fn render_info_text(desktop: &DesktopSummary) -> String {
    let mut output = String::new();
    let _ = writeln!(&mut output, "Desktop: {} [{}]", desktop.name, desktop.technology);
    let _ = writeln!(&mut output, "RuntimeId: {}", desktop.runtime_id);
    let _ = writeln!(&mut output, "OS: {} {}", desktop.os_name, desktop.os_version);
    let _ = writeln!(&mut output, "Bounds: {}", desktop.bounds);
    let _ = writeln!(&mut output, "Displays: {}", desktop.display_count);

    if desktop.monitors.is_empty() {
        let _ = writeln!(&mut output, "Monitors: none");
    } else {
        let _ = writeln!(&mut output, "Monitors:");
        for monitor in &desktop.monitors {
            let name = monitor.name.as_deref().unwrap_or("(unnamed)");
            let scale =
                monitor.scale_factor.map(|value| format!(", scale={value:.2}")).unwrap_or_default();
            let _ = writeln!(
                &mut output,
                "  - [{}] {} (primary: {}) bounds: {}{}",
                monitor.id,
                name,
                yes_no(monitor.is_primary),
                monitor.bounds,
                scale
            );
        }
    }

    output.trim_end().to_owned()
}

fn render_info_json(summary: &DesktopSummary) -> CliResult<String> {
    Ok(serde_json::to_string_pretty(summary)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::map_provider_error;
    use platynui_runtime::Runtime;
    use rstest::rstest;

    #[rstest]
    fn desktop_summary_uses_mock_desktop() {
        let mut runtime = Runtime::new().map_err(map_provider_error).expect("runtime");
        let summary = DesktopSummary::from_info(runtime.desktop_info());
        runtime.shutdown();

        assert_eq!(summary.name, "Mock Desktop");
        assert_eq!(summary.display_count, 1);
    }

    #[rstest]
    fn render_info_json_is_valid() {
        let summary = DesktopSummary {
            runtime_id: "mock".into(),
            name: "Mock Desktop".into(),
            technology: "MockTech".into(),
            bounds: Rect::new(0.0, 0.0, 1920.0, 1080.0),
            os_name: "MockOS".into(),
            os_version: "1.0".into(),
            display_count: 1,
            monitors: vec![],
        };

        let json = render_info_json(&summary).expect("json");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");
        assert_eq!(parsed["name"], "Mock Desktop");
    }
}
