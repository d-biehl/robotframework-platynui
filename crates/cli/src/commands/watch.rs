use crate::OutputFormat;
use crate::commands::query::{QueryItemSummary, render_query_text, summarize_query_results};
use crate::util::{CliResult, map_evaluate_error};
use anyhow::anyhow;
use clap::Args;
use platynui_core::provider::{ProviderDescriptor, ProviderEvent, ProviderEventKind};
use platynui_core::ui::UiNode;
use platynui_runtime::Runtime;
use platynui_runtime::provider::event::ProviderEventSink;
use serde::Serialize;
use std::io::{self, Write as IoWrite};
use std::sync::Arc;
use std::sync::mpsc;

#[derive(Args, Debug, Clone)]
pub struct WatchArgs {
    #[arg(long = "format", value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,
    #[arg(long = "expression")]
    pub expression: Option<String>,
    #[arg(long = "limit")]
    pub limit: Option<usize>,
}

pub fn run(runtime: &mut Runtime, args: &WatchArgs) -> CliResult<()> {
    let mut stdout = io::stdout();
    watch_with_writer(runtime, args, &mut stdout)
}

pub fn watch_with_writer<W: IoWrite>(runtime: &mut Runtime, args: &WatchArgs, writer: &mut W) -> CliResult<()> {
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
    let (sender, receiver) = mpsc::channel::<ProviderEvent>();
    let sink = Arc::new(ChannelSink::new(sender));
    runtime.register_event_sink(sink);
    after_register();

    let limit = args.limit.unwrap_or(usize::MAX);
    if limit == 0 {
        return Ok(());
    }
    report_event_sources(runtime.providers().map(|provider| provider.descriptor()));

    let expression = args.expression.as_deref();
    let cache = runtime.create_cache();
    let mut processed = 0usize;

    while processed < limit {
        let event = receiver.recv().map_err(|err| anyhow!("failed to receive provider event: {err}"))?;

        let summary = watch_event_summary(&event);
        let query_results = if let Some(expr) = expression {
            let results = runtime.evaluate_cached(None, expr, &cache).map_err(map_evaluate_error)?;
            Some(summarize_query_results(results))
        } else {
            None
        };

        match args.format {
            OutputFormat::Text => {
                let text = render_watch_text(&summary, query_results.as_deref());
                writeln!(writer, "{text}").map_err(|err| anyhow!("failed to write output: {err}"))?;
            }
            OutputFormat::Json => {
                let json = render_watch_json(&summary, query_results.as_deref())?;
                writeln!(writer, "{json}").map_err(|err| anyhow!("failed to write output: {err}"))?;
            }
        }

        processed += 1;
    }

    Ok(())
}

/// Say whether any of the active `providers` emits UI events: a warning when
/// none does, because the command then waits without output, and a debug
/// record naming the providers that do otherwise.
fn report_event_sources<'a>(providers: impl IntoIterator<Item = &'a ProviderDescriptor>) {
    let active: Vec<&ProviderDescriptor> = providers.into_iter().collect();
    let sources: Vec<&str> =
        active.iter().filter(|descriptor| !descriptor.event_capabilities().is_empty()).map(|d| d.id).collect();
    if sources.is_empty() {
        let providers: Vec<&str> = active.iter().map(|descriptor| descriptor.id).collect();
        tracing::warn!(?providers, "no active provider emits UI events; the command waits without output");
    } else {
        tracing::debug!(providers = ?sources, "watching UI events");
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
            name: node.name().clone(),
            runtime_id: node.runtime_id().as_str().to_owned(),
        }
    }
}

fn render_watch_text(summary: &WatchEventSummary, query_results: Option<&[QueryItemSummary]>) -> String {
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

fn render_watch_json(summary: &WatchEventSummary, query_results: Option<&[QueryItemSummary]>) -> CliResult<String> {
    let payload = WatchEventJson {
        event: summary.event.clone(),
        runtime_id: summary.runtime_id.clone(),
        parent_runtime_id: summary.parent_runtime_id.clone(),
        node: summary.node.clone(),
        query_results: query_results.map(<[QueryItemSummary]>::to_vec),
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
    use platynui_runtime::Runtime;
    use rstest::{fixture, rstest};
    use std::io::Cursor;

    #[cfg(any(test, feature = "mock-provider"))]
    use platynui_provider_mock;

    #[fixture]
    fn runtime() -> Runtime {
        return crate::test_support::runtime();
    }

    #[rstest]
    fn watch_text_streams_events(mut runtime: Runtime) {
        let args = WatchArgs { format: OutputFormat::Text, expression: None, limit: Some(1) };

        let mut buffer = Cursor::new(Vec::new());
        watch_with_writer_internal(&mut runtime, &args, &mut buffer, || {
            platynui_provider_mock::emit_node_updated("mock://button/ok");
        })
        .expect("watch");

        let output = String::from_utf8(buffer.into_inner()).expect("utf8");
        assert!(output.contains("NodeUpdated"));
        assert!(output.contains("mock://button/ok"));
    }

    #[rstest]
    fn watch_json_produces_serializable_payload(mut runtime: Runtime) {
        let args =
            WatchArgs { format: OutputFormat::Json, expression: Some("//control:Button".into()), limit: Some(1) };

        let mut buffer = Cursor::new(Vec::new());
        watch_with_writer_internal(&mut runtime, &args, &mut buffer, || {
            platynui_provider_mock::emit_node_updated("mock://button/ok");
        })
        .expect("watch");

        let line = String::from_utf8(buffer.into_inner()).expect("utf8");
        let value: serde_json::Value = serde_json::from_str(line.trim()).expect("json");
        assert_eq!(value["event"], "NodeUpdated");
    }

    /// Run `f` with its tracing output, down to debug, captured.
    fn logged<R>(f: impl FnOnce() -> R) -> (R, String) {
        use std::sync::Mutex;

        #[derive(Clone)]
        struct Captured(Arc<Mutex<Vec<u8>>>);
        impl IoWrite for Captured {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.0.lock().expect("log buffer").extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let buffer = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = Captured(Arc::clone(&buffer));
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing_subscriber::filter::LevelFilter::DEBUG)
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();
        let result = tracing::subscriber::with_default(subscriber, f);
        let log = String::from_utf8(buffer.lock().expect("log buffer").clone()).expect("utf-8 log");
        (result, log)
    }

    fn lines_at<'a>(log: &'a str, level: &str) -> Vec<&'a str> {
        log.lines().filter(|line| line.contains(&format!(" {level} "))).collect()
    }

    #[test]
    fn providers_without_events_give_one_warning() {
        use platynui_core::provider::ProviderKind;
        use platynui_core::ui::TechnologyId;

        let atspi = ProviderDescriptor::new("atspi", "AT-SPI", TechnologyId::from("AT-SPI"), ProviderKind::Native);
        let ((), log) = logged(|| report_event_sources([&atspi]));

        let warnings = lines_at(&log, "WARN");
        assert_eq!(warnings.len(), 1, "{log}");
        for expected in ["no active provider emits UI events; the command waits without output", r#"["atspi"]"#] {
            assert!(warnings[0].contains(expected), "the warning names `{expected}`: {}", warnings[0]);
        }

        let ((), log) = logged(|| report_event_sources([]));
        assert_eq!(lines_at(&log, "WARN").len(), 1, "no active provider at all is warned too\n{log}");
    }

    /// The mock provider emits events: `watch` logs no warning, and a debug
    /// record names it.
    #[rstest]
    fn watching_the_mock_logs_no_warning(mut runtime: Runtime) {
        let args = WatchArgs { format: OutputFormat::Text, expression: None, limit: Some(1) };

        let mut buffer = Cursor::new(Vec::new());
        let (result, log) = logged(|| {
            watch_with_writer_internal(&mut runtime, &args, &mut buffer, || {
                platynui_provider_mock::emit_node_updated("mock://button/ok");
            })
        });
        result.expect("watch");

        assert!(lines_at(&log, "WARN").is_empty(), "no warning\n{log}");
        let debugs: Vec<_> =
            lines_at(&log, "DEBUG").into_iter().filter(|line| line.contains("watching UI events")).collect();
        assert_eq!(debugs.len(), 1, "{log}");
        assert!(debugs[0].contains(r#"["mock"]"#), "the record names the mock provider: {}", debugs[0]);
    }
}
