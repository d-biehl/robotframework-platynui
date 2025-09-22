use clap::{Parser, Subcommand, ValueEnum};
use platynui_core::platform::{DesktopInfo, MonitorInfo};
use platynui_core::provider::{ProviderError, ProviderKind};
use platynui_core::types::Rect;
use platynui_runtime::Runtime;
use serde::Serialize;
use std::collections::HashSet;
use std::error::Error;
use std::fmt::Write;

#[allow(unused_imports)]
use platynui_platform_mock as _;
#[allow(unused_imports)]
use platynui_provider_mock as _;

type CliResult<T> = Result<T, Box<dyn Error>>;

#[derive(Parser)]
#[command(author, version, about = "PlatynUI command line interface", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(name = "list-providers", about = "List registered UI tree providers.")]
    ListProviders {
        #[arg(long = "format", value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    #[command(name = "info", about = "Show desktop and platform metadata.")]
    Info {
        #[arg(long = "format", value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
struct ProviderSummary {
    id: String,
    name: String,
    technology: String,
    kind: String,
    active: bool,
}

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

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

fn run() -> CliResult<()> {
    let cli = Cli::parse();
    let mut runtime = Runtime::new().map_err(map_provider_error)?;

    match cli.command {
        Commands::ListProviders { format } => {
            let output = list_providers(&runtime, format)?;
            println!("{output}");
        }
        Commands::Info { format } => {
            let output = show_info(&runtime, format)?;
            println!("{output}");
        }
    }

    runtime.shutdown();
    Ok(())
}

fn list_providers(runtime: &Runtime, format: OutputFormat) -> CliResult<String> {
    let summaries = collect_provider_summaries(runtime);
    match format {
        OutputFormat::Text => Ok(render_provider_text(&summaries)),
        OutputFormat::Json => render_provider_json(&summaries),
    }
}

fn show_info(runtime: &Runtime, format: OutputFormat) -> CliResult<String> {
    let summary = DesktopSummary::from_info(runtime.desktop_info());
    match format {
        OutputFormat::Text => Ok(render_info_text(&summary)),
        OutputFormat::Json => render_info_json(&summary),
    }
}

fn desktop_summary(runtime: &Runtime) -> DesktopSummary {
    DesktopSummary::from_info(runtime.desktop_info())
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

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn collect_provider_summaries(runtime: &Runtime) -> Vec<ProviderSummary> {
    let active_ids: HashSet<&str> =
        runtime.providers().map(|provider| provider.descriptor().id).collect();

    runtime
        .registry()
        .entries()
        .map(|entry| ProviderSummary {
            id: entry.descriptor.id.to_owned(),
            name: entry.descriptor.display_name.to_owned(),
            technology: entry.descriptor.technology.as_str().to_owned(),
            kind: kind_label(entry.descriptor.kind).to_owned(),
            active: active_ids.contains(entry.descriptor.id),
        })
        .collect()
}

fn render_provider_text(summaries: &[ProviderSummary]) -> String {
    let mut output = String::new();
    let _ = writeln!(
        &mut output,
        "{:<16} {:<12} {:<8} {:<7} {}",
        "ID", "Technology", "Kind", "Active", "Name"
    );

    for summary in summaries {
        let _ = writeln!(
            &mut output,
            "{:<16} {:<12} {:<8} {:<7} {}",
            summary.id,
            summary.technology,
            summary.kind,
            if summary.active { "yes" } else { "no" },
            summary.name
        );
    }

    output.trim_end().to_owned()
}

fn render_provider_json(summaries: &[ProviderSummary]) -> CliResult<String> {
    Ok(serde_json::to_string_pretty(summaries)?)
}

fn kind_label(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Native => "native",
        ProviderKind::External => "external",
    }
}

fn map_provider_error(err: ProviderError) -> Box<dyn Error> {
    Box::new(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use rstest::rstest;

    #[rstest]
    fn summaries_include_mock_provider() {
        let mut runtime = Runtime::new().map_err(map_provider_error).expect("runtime");
        let summaries = collect_provider_summaries(&runtime);
        runtime.shutdown();

        assert!(summaries.iter().any(|summary| summary.id == "mock"));
    }

    #[rstest]
    fn render_text_formats_table() {
        let summaries = vec![ProviderSummary {
            id: "mock".into(),
            name: "Mock Provider".into(),
            technology: "MockTech".into(),
            kind: "native".into(),
            active: true,
        }];

        let rendered = render_provider_text(&summaries);
        let lines: Vec<_> = rendered.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[1].contains("mock"));
        assert!(lines[1].contains("yes"));
    }

    #[rstest]
    fn render_json_produces_valid_json() {
        let summaries = vec![ProviderSummary {
            id: "mock".into(),
            name: "Mock Provider".into(),
            technology: "MockTech".into(),
            kind: "native".into(),
            active: true,
        }];

        let rendered = render_provider_json(&summaries).expect("json");
        let value: serde_json::Value = serde_json::from_str(&rendered).expect("valid json");
        assert!(value.is_array());
        assert_eq!(value.as_array().unwrap().len(), 1);
    }

    #[rstest]
    fn clap_parsing_defaults_to_text() {
        let cli = Cli::parse_from(["platynui-cli", "list-providers"]);
        match cli.command {
            Commands::ListProviders { format } => assert!(matches!(format, OutputFormat::Text)),
            _ => panic!("unexpected command variant"),
        }
    }

    #[rstest]
    fn clap_parsing_info_defaults_to_text() {
        let cli = Cli::parse_from(["platynui-cli", "info"]);
        match cli.command {
            Commands::Info { format } => assert!(matches!(format, OutputFormat::Text)),
            _ => panic!("unexpected command variant"),
        }
    }

    #[rstest]
    fn desktop_summary_uses_mock_desktop() {
        let mut runtime = Runtime::new().map_err(map_provider_error).expect("runtime");
        let summary = desktop_summary(&runtime);
        runtime.shutdown();

        assert_eq!(summary.os_name, "MockOS");
        assert_eq!(summary.display_count, 1);
    }

    #[rstest]
    fn render_info_json_is_valid() {
        let mut runtime = Runtime::new().map_err(map_provider_error).expect("runtime");
        let summary = desktop_summary(&runtime);
        runtime.shutdown();

        let rendered = render_info_json(&summary).expect("json");
        let value: serde_json::Value = serde_json::from_str(&rendered).expect("valid json");
        assert_eq!(value.get("name").unwrap(), "Mock Desktop");
    }
}
