use clap::{Parser, Subcommand, ValueEnum};
use platynui_core::platform::{DesktopInfo, MonitorInfo};
use platynui_core::provider::{ProviderError, ProviderKind};
use platynui_core::types::Rect;
use platynui_core::ui::{Namespace, PatternId, UiValue};
use platynui_runtime::{EvaluateError, EvaluationItem, Runtime};
use serde::Serialize;
use std::collections::HashSet;
use std::error::Error;
use std::fmt::Write;
use std::str::FromStr;

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
    #[command(name = "query", about = "Evaluate XPath expressions.")]
    Query {
        #[arg(value_name = "XPATH")]
        expression: String,
        #[arg(long = "namespace")]
        namespaces: Vec<String>,
        #[arg(long = "pattern")]
        patterns: Vec<String>,
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

#[derive(Serialize, Debug, Clone, PartialEq)]
struct AttributeSummary {
    namespace: String,
    name: String,
    value: UiValue,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(tag = "type")]
enum QueryItemSummary {
    Node {
        runtime_id: String,
        namespace: String,
        role: String,
        name: String,
        supported_patterns: Vec<String>,
        attributes: Vec<AttributeSummary>,
    },
    Attribute {
        owner_runtime_id: String,
        namespace: String,
        name: String,
        value: UiValue,
    },
    Value {
        value: UiValue,
    },
}

struct QueryArgs<'a> {
    expression: &'a str,
    format: OutputFormat,
    namespaces: &'a [String],
    patterns: &'a [String],
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
        Commands::Query { expression, namespaces, patterns, format } => {
            let args = QueryArgs {
                expression: &expression,
                format,
                namespaces: &namespaces,
                patterns: &patterns,
            };
            let output = handle_query(&runtime, args)?;
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

fn handle_query(runtime: &Runtime, args: QueryArgs) -> CliResult<String> {
    let namespace_filters = parse_namespace_filters(args.namespaces)?;
    let pattern_filters = if args.patterns.is_empty() {
        None
    } else {
        Some(args.patterns.iter().cloned().collect::<HashSet<_>>())
    };

    let results = runtime.evaluate(None, args.expression).map_err(map_evaluate_error)?;

    let summaries =
        summarize_query_results(results, namespace_filters.as_ref(), pattern_filters.as_ref());

    match args.format {
        OutputFormat::Text => Ok(render_query_text(&summaries)),
        OutputFormat::Json => render_query_json(&summaries),
    }
}

fn parse_namespace_filters(values: &[String]) -> CliResult<Option<HashSet<Namespace>>> {
    if values.is_empty() {
        return Ok(None);
    }

    let mut filters = HashSet::new();
    for value in values {
        let namespace =
            Namespace::from_str(value).map_err(|_| format!("unknown namespace prefix: {value}"))?;
        filters.insert(namespace);
    }
    Ok(Some(filters))
}

fn summarize_query_results(
    results: Vec<EvaluationItem>,
    namespace_filters: Option<&HashSet<Namespace>>,
    pattern_filters: Option<&HashSet<String>>,
) -> Vec<QueryItemSummary> {
    results
        .into_iter()
        .filter_map(|item| match item {
            EvaluationItem::Node(node) => {
                let namespace = node.namespace();
                if let Some(filters) = namespace_filters
                    && !filters.contains(&namespace)
                {
                    return None;
                }

                if let Some(filters) = pattern_filters
                    && !matches_pattern_filter(node.supported_patterns(), filters)
                {
                    return None;
                }

                let supported_patterns =
                    node.supported_patterns().iter().map(|id| id.as_str().to_owned()).collect();

                let mut attributes: Vec<AttributeSummary> = node
                    .attributes()
                    .map(|attribute| AttributeSummary {
                        namespace: attribute.namespace().as_str().to_owned(),
                        name: attribute.name().to_owned(),
                        value: attribute.value(),
                    })
                    .collect();
                attributes.sort_by(|lhs, rhs| {
                    (lhs.namespace.as_str(), lhs.name.as_str())
                        .cmp(&(rhs.namespace.as_str(), rhs.name.as_str()))
                });

                Some(QueryItemSummary::Node {
                    runtime_id: node.runtime_id().as_str().to_owned(),
                    namespace: namespace.as_str().to_owned(),
                    role: node.role().to_owned(),
                    name: node.name().to_owned(),
                    supported_patterns,
                    attributes,
                })
            }
            EvaluationItem::Attribute(attr) => {
                if let Some(filters) = namespace_filters
                    && !filters.contains(&attr.namespace)
                {
                    return None;
                }

                Some(QueryItemSummary::Attribute {
                    owner_runtime_id: attr.owner.runtime_id().as_str().to_owned(),
                    namespace: attr.namespace.as_str().to_owned(),
                    name: attr.name.clone(),
                    value: attr.value.clone(),
                })
            }
            EvaluationItem::Value(value) => {
                if namespace_filters.is_some() {
                    return None;
                }
                Some(QueryItemSummary::Value { value })
            }
        })
        .collect()
}

fn matches_pattern_filter(patterns: &[PatternId], filters: &HashSet<String>) -> bool {
    filters.iter().all(|pattern| patterns.iter().any(|id| id.as_str() == pattern))
}

fn render_query_text(items: &[QueryItemSummary]) -> String {
    let mut output = String::new();
    for item in items {
        match item {
            QueryItemSummary::Node {
                runtime_id,
                namespace,
                role,
                name,
                supported_patterns,
                attributes,
            } => {
                let ns_label = capitalize_namespace(namespace);
                let mut base_attrs = vec![
                    format!("RuntimeId=\"{}\"", runtime_id),
                    format!("Name=\"{}\"", name),
                    format!("Role=\"{}\"", role),
                ];
                if !supported_patterns.is_empty() {
                    base_attrs
                        .push(format!("SupportedPatterns=\"{}\"", supported_patterns.join(", ")));
                }

                let mut extra_attrs: Vec<String> = attributes
                    .iter()
                    .filter(|attr| !should_skip_attribute(attr))
                    .map(|attr| {
                        let key = attribute_display_name(namespace, attr);
                        let value = format_attribute_value(&attr.value);
                        format!("{}=\"{}\"", key, value)
                    })
                    .collect();
                extra_attrs.sort();

                base_attrs.extend(extra_attrs);
                let attr_block = base_attrs.join(" ");
                let _ = writeln!(&mut output, "{} <{} {} />", ns_label, role, attr_block);
            }
            QueryItemSummary::Attribute { owner_runtime_id, namespace, name, value } => {
                let _ = writeln!(
                    &mut output,
                    "Attribute owner={} namespace={} name={} value={}",
                    owner_runtime_id,
                    namespace,
                    name,
                    format_ui_value(value)
                );
            }
            QueryItemSummary::Value { value } => {
                let _ = writeln!(&mut output, "Value   {}", format_ui_value(value));
            }
        }
    }

    output.trim_end().to_owned()
}

fn render_query_json(items: &[QueryItemSummary]) -> CliResult<String> {
    Ok(serde_json::to_string_pretty(items)?)
}

fn capitalize_namespace(ns: &str) -> String {
    let mut chars = ns.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

fn attribute_display_name(node_namespace: &str, attr: &AttributeSummary) -> String {
    if attr.namespace == node_namespace || attr.namespace == "control" {
        attr.name.clone()
    } else {
        format!("{}:{}", attr.namespace, attr.name)
    }
}

fn should_skip_attribute(attr: &AttributeSummary) -> bool {
    let key = attr.name.as_str();
    let ns = attr.namespace.as_str();
    (ns == "control" && matches!(key, "Name" | "Role" | "RuntimeId" | "SupportedPatterns"))
        || (ns == "app" && matches!(key, "Name" | "Role" | "RuntimeId"))
}

fn format_attribute_value(value: &UiValue) -> String {
    match value {
        UiValue::Null => "null".to_string(),
        UiValue::Bool(b) => b.to_string(),
        UiValue::Integer(i) => i.to_string(),
        UiValue::Number(n) => format!("{n}"),
        UiValue::String(text) => text.clone(),
        _ => serde_json::to_string(value).unwrap_or_else(|_| String::from("<value>")),
    }
}

fn format_ui_value(value: &UiValue) -> String {
    match value {
        UiValue::Null => "null".to_string(),
        UiValue::Bool(b) => b.to_string(),
        UiValue::Integer(i) => i.to_string(),
        UiValue::Number(n) => format!("{n}"),
        UiValue::String(text) => format!("\"{}\"", text),
        _ => serde_json::to_string(value).unwrap_or_else(|_| "<value>".to_string()),
    }
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
        "{:<16} {:<12} {:<8} {:<7} Name",
        "ID", "Technology", "Kind", "Active"
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

fn map_evaluate_error(err: EvaluateError) -> Box<dyn Error> {
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
        let summary = DesktopSummary::from_info(runtime.desktop_info());
        runtime.shutdown();

        assert_eq!(summary.os_name, "MockOS");
        assert_eq!(summary.display_count, 1);
    }

    #[rstest]
    fn render_info_json_is_valid() {
        let mut runtime = Runtime::new().map_err(map_provider_error).expect("runtime");
        let summary = DesktopSummary::from_info(runtime.desktop_info());
        runtime.shutdown();

        let rendered = render_info_json(&summary).expect("json");
        let value: serde_json::Value = serde_json::from_str(&rendered).expect("valid json");
        assert_eq!(value.get("name").unwrap(), "Mock Desktop");
    }

    #[rstest]
    fn query_text_returns_nodes() {
        let mut runtime = Runtime::new().map_err(map_provider_error).expect("runtime");
        let namespaces = Vec::new();
        let patterns = Vec::new();
        let args = QueryArgs {
            expression: "//control:Button",
            format: OutputFormat::Text,
            namespaces: &namespaces,
            patterns: &patterns,
        };
        let output = handle_query(&runtime, args).expect("query");
        runtime.shutdown();
        assert!(output.contains("mock://button/ok"));
    }

    #[rstest]
    fn query_json_produces_valid_payload() {
        let mut runtime = Runtime::new().map_err(map_provider_error).expect("runtime");
        let namespaces = Vec::new();
        let patterns = Vec::new();
        let args = QueryArgs {
            expression: "//control:Button",
            format: OutputFormat::Json,
            namespaces: &namespaces,
            patterns: &patterns,
        };
        let output = handle_query(&runtime, args).expect("query");
        runtime.shutdown();

        let value: serde_json::Value = serde_json::from_str(&output).expect("valid json");
        assert!(value.is_array());
        assert_eq!(value[0]["type"], "Node");
    }

    #[rstest]
    fn query_namespace_filter_limits_results() {
        let mut runtime = Runtime::new().map_err(map_provider_error).expect("runtime");
        let namespaces = vec!["item".to_string()];
        let patterns = Vec::new();
        let args = QueryArgs {
            expression: "//control:Button",
            format: OutputFormat::Text,
            namespaces: &namespaces,
            patterns: &patterns,
        };
        let output = handle_query(&runtime, args).expect("query");
        runtime.shutdown();
        assert!(output.trim().is_empty());
    }
}
