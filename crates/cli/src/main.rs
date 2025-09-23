use clap::{Parser, Subcommand, ValueEnum};
use platynui_core::platform::{DesktopInfo, MonitorInfo};
use platynui_core::provider::{ProviderError, ProviderEvent, ProviderEventKind, ProviderKind};
use platynui_core::types::Rect;
use platynui_core::ui::{Namespace, PatternId, UiNode, UiValue};
use platynui_runtime::provider::event::ProviderEventSink;
use platynui_runtime::{EvaluateError, EvaluationItem, Runtime};
use serde::Serialize;
use std::collections::HashSet;
use std::error::Error;
use std::fmt::Write;
use std::io::{self, Write as IoWrite};
use std::str::FromStr;
use std::sync::Arc;
use std::sync::mpsc;

#[cfg(any(test, feature = "mock-provider"))]
use platynui_platform_mock as _;
#[cfg(any(test, feature = "mock-provider"))]
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
    #[command(name = "watch", about = "Watch provider events.")]
    Watch {
        #[arg(long = "format", value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
        #[arg(long = "namespace")]
        namespaces: Vec<String>,
        #[arg(long = "pattern")]
        patterns: Vec<String>,
        #[arg(long = "runtime-id")]
        runtime_ids: Vec<String>,
        #[arg(long = "expression")]
        expression: Option<String>,
        #[arg(long = "limit")]
        limit: Option<usize>,
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

#[derive(Serialize, Debug, Clone, PartialEq)]
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

struct WatchArgs<'a> {
    format: OutputFormat,
    namespaces: &'a [String],
    patterns: &'a [String],
    runtime_ids: &'a [String],
    expression: Option<&'a str>,
    limit: Option<usize>,
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
        Commands::Watch { format, namespaces, patterns, runtime_ids, expression, limit } => {
            let args = WatchArgs {
                format,
                namespaces: &namespaces,
                patterns: &patterns,
                runtime_ids: &runtime_ids,
                expression: expression.as_deref(),
                limit,
            };
            handle_watch(&mut runtime, args)?;
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

                Some(node_to_query_summary(node))
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

fn node_to_query_summary(node: Arc<dyn UiNode>) -> QueryItemSummary {
    let namespace = node.namespace();
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

    QueryItemSummary::Node {
        runtime_id: node.runtime_id().as_str().to_owned(),
        namespace: namespace.as_str().to_owned(),
        role: node.role().to_owned(),
        name: node.name().to_owned(),
        supported_patterns,
        attributes,
    }
}

fn matches_pattern_filter(patterns: &[PatternId], filters: &HashSet<String>) -> bool {
    filters.iter().all(|pattern| patterns.iter().any(|id| id.as_str() == pattern))
}

fn handle_watch(runtime: &mut Runtime, args: WatchArgs) -> CliResult<()> {
    let mut stdout = io::stdout();
    handle_watch_with_writer(runtime, args, &mut stdout)
}

fn handle_watch_with_writer<W: IoWrite>(
    runtime: &mut Runtime,
    args: WatchArgs,
    writer: &mut W,
) -> CliResult<()> {
    handle_watch_with_writer_internal(runtime, args, writer, || {})
}

fn handle_watch_with_writer_internal<W, F>(
    runtime: &mut Runtime,
    args: WatchArgs,
    writer: &mut W,
    after_register: F,
) -> CliResult<()>
where
    W: IoWrite,
    F: FnOnce(),
{
    let namespace_filters = parse_namespace_filters(args.namespaces)?;
    let pattern_filters = if args.patterns.is_empty() {
        None
    } else {
        Some(args.patterns.iter().cloned().collect::<HashSet<_>>())
    };
    let runtime_filters = if args.runtime_ids.is_empty() {
        None
    } else {
        Some(args.runtime_ids.iter().cloned().collect::<HashSet<_>>())
    };

    let filters = WatchFilters::new(namespace_filters, pattern_filters, runtime_filters);
    let (sender, receiver) = mpsc::channel::<ProviderEvent>();
    let sink = Arc::new(ChannelSink::new(sender));
    runtime.register_event_sink(sink);
    after_register();

    let limit = args.limit.unwrap_or(usize::MAX);
    if limit == 0 {
        return Ok(());
    }

    let expression = args.expression;
    let mut processed = 0usize;

    while processed < limit {
        let event =
            receiver.recv().map_err(|err| format!("failed to receive provider event: {err}"))?;

        if !filters.matches(&event.kind) {
            continue;
        }

        let summary = watch_event_summary(&event);
        let query_results = if let Some(expr) = expression {
            let results = runtime.evaluate(None, expr).map_err(map_evaluate_error)?;
            Some(summarize_query_results(
                results,
                filters.namespace_filters(),
                filters.pattern_filters(),
            ))
        } else {
            None
        };

        match args.format {
            OutputFormat::Text => {
                let text = render_watch_text(&summary, query_results.as_deref());
                writeln!(writer, "{}", text)
                    .map_err(|err| format!("failed to write output: {err}"))?;
            }
            OutputFormat::Json => {
                let json = render_watch_json(&summary, query_results.as_deref())?;
                writeln!(writer, "{}", json)
                    .map_err(|err| format!("failed to write output: {err}"))?;
            }
        }

        processed += 1;
    }

    Ok(())
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

#[derive(Clone)]
struct WatchFilters {
    namespaces: Option<HashSet<Namespace>>,
    patterns: Option<HashSet<String>>,
    runtime_ids: Option<HashSet<String>>,
}

impl WatchFilters {
    fn new(
        namespaces: Option<HashSet<Namespace>>,
        patterns: Option<HashSet<String>>,
        runtime_ids: Option<HashSet<String>>,
    ) -> Self {
        Self { namespaces, patterns, runtime_ids }
    }

    fn matches(&self, event: &ProviderEventKind) -> bool {
        match event {
            ProviderEventKind::NodeAdded { parent, node } => {
                if let Some(filters) = &self.runtime_ids {
                    let runtime_id = node.runtime_id().as_str();
                    let parent_match =
                        parent.as_ref().map(|id| filters.contains(id.as_str())).unwrap_or(false);
                    if !filters.contains(runtime_id) && !parent_match {
                        return false;
                    }
                }
                self.matches_node(node)
            }
            ProviderEventKind::NodeUpdated { node } => self.matches_node(node),
            ProviderEventKind::NodeRemoved { runtime_id } => {
                if let Some(filters) = &self.runtime_ids {
                    filters.contains(runtime_id.as_str())
                } else {
                    true
                }
            }
            ProviderEventKind::TreeInvalidated => true,
        }
    }

    fn matches_node(&self, node: &Arc<dyn UiNode>) -> bool {
        if let Some(filters) = &self.runtime_ids
            && !filters.contains(node.runtime_id().as_str())
        {
            return false;
        }

        if let Some(filters) = &self.namespaces
            && !filters.contains(&node.namespace())
        {
            return false;
        }

        if let Some(filters) = &self.patterns
            && !matches_pattern_filter(node.supported_patterns(), filters)
        {
            return false;
        }

        true
    }

    fn namespace_filters(&self) -> Option<&HashSet<Namespace>> {
        self.namespaces.as_ref()
    }

    fn pattern_filters(&self) -> Option<&HashSet<String>> {
        self.patterns.as_ref()
    }
}

struct ChannelSink {
    sender: mpsc::Sender<ProviderEvent>,
}

impl ChannelSink {
    fn new(sender: mpsc::Sender<ProviderEvent>) -> Self {
        Self { sender }
    }
}

impl ProviderEventSink for ChannelSink {
    fn dispatch(&self, event: ProviderEvent) {
        let _ = self.sender.send(event);
    }
}

#[derive(Clone, Debug, Serialize)]
struct WatchEventSummary {
    event: String,
    runtime_id: Option<String>,
    parent_runtime_id: Option<String>,
    node: Option<QueryItemSummary>,
}

#[derive(Serialize)]
struct WatchEventJson {
    event: String,
    runtime_id: Option<String>,
    parent_runtime_id: Option<String>,
    node: Option<QueryItemSummary>,
    query_results: Option<Vec<QueryItemSummary>>,
}

fn watch_event_summary(event: &ProviderEvent) -> WatchEventSummary {
    match &event.kind {
        ProviderEventKind::NodeAdded { parent, node } => WatchEventSummary {
            event: "NodeAdded".to_string(),
            runtime_id: Some(node.runtime_id().as_str().to_owned()),
            parent_runtime_id: parent.as_ref().map(|id| id.as_str().to_owned()),
            node: Some(node_to_query_summary(Arc::clone(node))),
        },
        ProviderEventKind::NodeUpdated { node } => WatchEventSummary {
            event: "NodeUpdated".to_string(),
            runtime_id: Some(node.runtime_id().as_str().to_owned()),
            parent_runtime_id: None,
            node: Some(node_to_query_summary(Arc::clone(node))),
        },
        ProviderEventKind::NodeRemoved { runtime_id } => WatchEventSummary {
            event: "NodeRemoved".to_string(),
            runtime_id: Some(runtime_id.as_str().to_owned()),
            parent_runtime_id: None,
            node: None,
        },
        ProviderEventKind::TreeInvalidated => WatchEventSummary {
            event: "TreeInvalidated".to_string(),
            runtime_id: None,
            parent_runtime_id: None,
            node: None,
        },
    }
}

fn render_watch_text(
    summary: &WatchEventSummary,
    query_results: Option<&[QueryItemSummary]>,
) -> String {
    let mut sections: Vec<String> = Vec::new();

    match (&summary.event[..], &summary.runtime_id, &summary.parent_runtime_id, &summary.node) {
        ("NodeAdded", runtime_id, parent, node) => {
            let mut header = String::from("Event NodeAdded");
            if let Some(id) = runtime_id {
                header.push_str(&format!(" runtime_id=\"{}\"", id));
            }
            if let Some(parent_id) = parent {
                header.push_str(&format!(" parent=\"{}\"", parent_id));
            }
            sections.push(header);
            if let Some(node_summary) = node {
                let node_text = render_query_text(std::slice::from_ref(node_summary));
                if !node_text.is_empty() {
                    sections.push(node_text);
                }
            }
        }
        ("NodeUpdated", runtime_id, _, node) => {
            let mut header = String::from("Event NodeUpdated");
            if let Some(id) = runtime_id {
                header.push_str(&format!(" runtime_id=\"{}\"", id));
            }
            sections.push(header);
            if let Some(node_summary) = node {
                let node_text = render_query_text(std::slice::from_ref(node_summary));
                if !node_text.is_empty() {
                    sections.push(node_text);
                }
            }
        }
        ("NodeRemoved", runtime_id, _, _) => {
            let mut header = String::from("Event NodeRemoved");
            if let Some(id) = runtime_id {
                header.push_str(&format!(" runtime_id=\"{}\"", id));
            }
            sections.push(header);
        }
        ("TreeInvalidated", _, _, _) => {
            sections.push("Event TreeInvalidated".to_string());
        }
        _ => {}
    }

    if let Some(results) = query_results {
        sections.push("-- Query Results --".to_string());
        if results.is_empty() {
            sections.push("(empty)".to_string());
        } else {
            sections.push(render_query_text(results));
        }
    }

    sections.join("\n")
}

fn render_watch_json(
    summary: &WatchEventSummary,
    query_results: Option<&[QueryItemSummary]>,
) -> CliResult<String> {
    let payload = WatchEventJson {
        event: summary.event.clone(),
        runtime_id: summary.runtime_id.clone(),
        parent_runtime_id: summary.parent_runtime_id.clone(),
        node: summary.node.clone(),
        query_results: query_results.map(|items| items.to_vec()),
    };
    Ok(serde_json::to_string(&payload)?)
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

    #[rstest]
    fn watch_text_streams_events() {
        let mut runtime = Runtime::new().map_err(map_provider_error).expect("runtime");
        let args = WatchArgs {
            format: OutputFormat::Text,
            namespaces: &[],
            patterns: &[],
            runtime_ids: &[],
            expression: None,
            limit: Some(1),
        };
        let mut buffer: Vec<u8> = Vec::new();

        handle_watch_with_writer_internal(&mut runtime, args, &mut buffer, || {
            platynui_provider_mock::emit_node_updated("mock://button/ok");
        })
        .expect("watch");
        runtime.shutdown();

        let output = String::from_utf8(buffer).expect("utf8");
        assert!(output.contains("NodeUpdated"));
        assert!(output.contains("mock://button/ok"));
    }

    #[rstest]
    fn watch_json_produces_serializable_payload() {
        let mut runtime = Runtime::new().map_err(map_provider_error).expect("runtime");
        let args = WatchArgs {
            format: OutputFormat::Json,
            namespaces: &[],
            patterns: &[],
            runtime_ids: &[],
            expression: None,
            limit: Some(1),
        };
        let mut buffer: Vec<u8> = Vec::new();

        handle_watch_with_writer_internal(&mut runtime, args, &mut buffer, || {
            platynui_provider_mock::emit_event(ProviderEventKind::TreeInvalidated);
        })
        .expect("watch");
        runtime.shutdown();

        let output = String::from_utf8(buffer).expect("utf8");
        let line = output.lines().next().expect("line");
        let value: serde_json::Value = serde_json::from_str(line).expect("json");
        assert_eq!(value["event"], "TreeInvalidated");
    }
}
