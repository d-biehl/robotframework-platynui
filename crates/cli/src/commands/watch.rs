use crate::OutputFormat;
use crate::commands::query::{QueryItemSummary, render_query_text, summarize_query_results};
use crate::util::{CliResult, map_evaluate_error, parse_namespace_filters};
use clap::Args;
use platynui_core::provider::{ProviderEvent, ProviderEventKind};
use platynui_core::ui::identifiers::RuntimeId;
use platynui_core::ui::{Namespace, UiNode};
use platynui_runtime::Runtime;
use platynui_runtime::provider::event::ProviderEventSink;
use serde::Serialize;
use std::collections::HashSet;
use std::io::{self, Write as IoWrite};
use std::sync::Arc;
use std::sync::mpsc;

#[derive(Args, Debug, Clone)]
pub struct WatchArgs {
    #[arg(long = "format", value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,
    #[arg(long = "namespace")]
    pub namespaces: Vec<String>,
    #[arg(long = "pattern")]
    pub patterns: Vec<String>,
    #[arg(long = "runtime-id")]
    pub runtime_ids: Vec<String>,
    #[arg(long = "expression")]
    pub expression: Option<String>,
    #[arg(long = "limit")]
    pub limit: Option<usize>,
}

pub fn run(runtime: &mut Runtime, args: &WatchArgs) -> CliResult<()> {
    let mut stdout = io::stdout();
    watch_with_writer(runtime, args, &mut stdout)
}

pub fn watch_with_writer<W: IoWrite>(
    runtime: &mut Runtime,
    args: &WatchArgs,
    writer: &mut W,
) -> CliResult<()> {
    watch_with_writer_internal(runtime, args, writer, || {})
}

pub(crate) fn watch_with_writer_internal<W, F>(
    runtime: &mut Runtime,
    args: &WatchArgs,
    writer: &mut W,
    after_register: F,
) -> CliResult<()>
where
    W: IoWrite,
    F: FnOnce(),
{
    let namespace_filters = parse_namespace_filters(&args.namespaces)?;
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

    let expression = args.expression.as_deref();
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

    fn matches(&self, kind: &ProviderEventKind) -> bool {
        match kind {
            ProviderEventKind::TreeInvalidated => true,
            ProviderEventKind::NodeAdded { node, parent } => {
                self.matches_node(node, parent.as_ref())
            }
            ProviderEventKind::NodeUpdated { node } => self.matches_node(node, None),
            ProviderEventKind::NodeRemoved { runtime_id } => self.matches_runtime(runtime_id),
        }
    }

    fn matches_node(&self, node: &Arc<dyn UiNode>, parent_runtime: Option<&RuntimeId>) -> bool {
        if let Some(filters) = &self.namespaces
            && !filters.contains(&node.namespace())
        {
            return false;
        }

        if let Some(filters) = &self.patterns
            && !filters
                .iter()
                .all(|pattern| node.supported_patterns().iter().any(|id| id.as_str() == pattern))
        {
            return false;
        }

        if let Some(filters) = &self.runtime_ids
            && !filters.contains(node.runtime_id().as_str())
            && !parent_runtime.map(|p| filters.contains(p.as_str())).unwrap_or(false)
        {
            return false;
        }

        true
    }

    fn matches_runtime(&self, runtime_id: &RuntimeId) -> bool {
        if self.namespaces.is_some() || self.patterns.is_some() {
            return false;
        }

        match &self.runtime_ids {
            Some(filters) => filters.contains(runtime_id.as_str()),
            None => true,
        }
    }

    fn namespace_filters(&self) -> Option<&HashSet<Namespace>> {
        self.namespaces.as_ref()
    }

    fn pattern_filters(&self) -> Option<&HashSet<String>> {
        self.patterns.as_ref()
    }
}

#[derive(Debug, Clone, Serialize)]
struct WatchEventSummary {
    event: String,
    runtime_id: Option<String>,
    parent_runtime_id: Option<String>,
    node: Option<NodeSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
struct NodeSnapshot {
    namespace: String,
    role: String,
    name: String,
    runtime_id: String,
}

fn watch_event_summary(event: &ProviderEvent) -> WatchEventSummary {
    match &event.kind {
        ProviderEventKind::TreeInvalidated => WatchEventSummary {
            event: "TreeInvalidated".to_string(),
            runtime_id: None,
            parent_runtime_id: None,
            node: None,
        },
        ProviderEventKind::NodeAdded { node, parent } => WatchEventSummary {
            event: "NodeAdded".to_string(),
            runtime_id: Some(node.runtime_id().as_str().to_owned()),
            parent_runtime_id: parent.as_ref().map(|id| id.as_str().to_owned()),
            node: Some(NodeSnapshot::from_node(node)),
        },
        ProviderEventKind::NodeUpdated { node } => WatchEventSummary {
            event: "NodeUpdated".to_string(),
            runtime_id: Some(node.runtime_id().as_str().to_owned()),
            parent_runtime_id: None,
            node: Some(NodeSnapshot::from_node(node)),
        },
        ProviderEventKind::NodeRemoved { runtime_id } => WatchEventSummary {
            event: "NodeRemoved".to_string(),
            runtime_id: Some(runtime_id.as_str().to_owned()),
            parent_runtime_id: None,
            node: None,
        },
    }
}

impl NodeSnapshot {
    fn from_node(node: &Arc<dyn UiNode>) -> Self {
        Self {
            namespace: node.namespace().as_str().to_owned(),
            role: node.role().to_owned(),
            name: node.name().to_owned(),
            runtime_id: node.runtime_id().as_str().to_owned(),
        }
    }
}

fn render_watch_text(
    summary: &WatchEventSummary,
    query_results: Option<&[QueryItemSummary]>,
) -> String {
    let mut sections = Vec::new();
    sections.push(format!("Event: {}", summary.event));
    if let Some(id) = &summary.runtime_id {
        sections.push(format!("RuntimeId: {id}"));
    }
    if let Some(parent) = &summary.parent_runtime_id {
        sections.push(format!("Parent: {parent}"));
    }
    if let Some(node) = &summary.node {
        sections.push(format!(
            "Node: {namespace}:{role} name=\"{name}\"",
            namespace = node.namespace,
            role = node.role,
            name = node.name
        ));
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

#[derive(Serialize)]
struct WatchEventJson {
    event: String,
    runtime_id: Option<String>,
    parent_runtime_id: Option<String>,
    node: Option<NodeSnapshot>,
    query_results: Option<Vec<QueryItemSummary>>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::map_provider_error;
    use platynui_runtime::Runtime;
    use rstest::rstest;
    use std::io::Cursor;

    #[cfg(any(test, feature = "mock-provider"))]
    use platynui_provider_mock;

    #[rstest]
    fn watch_text_streams_events() {
        let mut runtime = Runtime::new().map_err(map_provider_error).expect("runtime");
        let args = WatchArgs {
            format: OutputFormat::Text,
            namespaces: vec![],
            patterns: vec![],
            runtime_ids: vec![],
            expression: None,
            limit: Some(1),
        };

        let mut buffer = Cursor::new(Vec::new());
        watch_with_writer_internal(&mut runtime, &args, &mut buffer, || {
            platynui_provider_mock::emit_node_updated("mock://button/ok");
        })
        .expect("watch");
        runtime.shutdown();

        let output = String::from_utf8(buffer.into_inner()).expect("utf8");
        assert!(output.contains("NodeUpdated"));
        assert!(output.contains("mock://button/ok"));
    }

    #[rstest]
    fn watch_json_produces_serializable_payload() {
        let mut runtime = Runtime::new().map_err(map_provider_error).expect("runtime");
        let args = WatchArgs {
            format: OutputFormat::Json,
            namespaces: vec![],
            patterns: vec![],
            runtime_ids: vec![],
            expression: Some("//control:Button".into()),
            limit: Some(1),
        };

        let mut buffer = Cursor::new(Vec::new());
        watch_with_writer_internal(&mut runtime, &args, &mut buffer, || {
            platynui_provider_mock::emit_node_updated("mock://button/ok");
        })
        .expect("watch");
        runtime.shutdown();

        let line = String::from_utf8(buffer.into_inner()).expect("utf8");
        let value: serde_json::Value = serde_json::from_str(line.trim()).expect("json");
        assert_eq!(value["event"], "NodeUpdated");
    }
}
