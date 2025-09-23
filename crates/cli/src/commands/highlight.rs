use crate::util::{CliResult, map_evaluate_error, map_platform_error};
use clap::Args;
use platynui_core::platform::HighlightRequest;
use platynui_core::ui::{Namespace, UiValue};
use platynui_runtime::{EvaluationItem, Runtime};
use std::time::Duration;

#[derive(Args, Debug, Clone)]
pub struct HighlightArgs {
    #[arg(value_name = "XPATH")]
    pub expression: Option<String>,
    #[arg(
        long = "duration-ms",
        value_parser = parse_duration_ms,
        help = "Duration in milliseconds the highlight stays visible before fading."
    )]
    pub duration_ms: Option<u64>,
    #[arg(long = "clear", help = "Clear existing highlights before applying a new one or alone.")]
    pub clear: bool,
}

pub fn run(runtime: &Runtime, args: &HighlightArgs) -> CliResult<String> {
    if args.expression.is_none() && !args.clear {
        return Err("highlight requires an XPath expression unless --clear is set".into());
    }

    let mut messages = Vec::new();

    if args.clear {
        runtime.clear_highlight().map_err(map_platform_error)?;
        messages.push("Cleared existing highlights.".to_owned());
    }

    if let Some(expression) = &args.expression {
        let highlight = collect_highlight_requests(runtime, expression, args.duration_ms)?;
        if highlight.requests.is_empty() {
            return Err(format!("no highlightable nodes for expression `{expression}`").into());
        }

        runtime.highlight(&highlight.requests).map_err(map_platform_error)?;

        let mut message = format!("Highlighted {} node(s).", highlight.requests.len());
        if !highlight.skipped.is_empty() {
            let skipped = highlight.skipped.join(", ");
            message.push_str(&format!(" Skipped nodes without Bounds: {skipped}."));
        }
        messages.push(message);
    }

    if messages.is_empty() {
        messages.push("No highlight action executed.".to_owned());
    }

    Ok(messages.join("\n"))
}

struct HighlightComputation {
    requests: Vec<HighlightRequest>,
    skipped: Vec<String>,
}

fn collect_highlight_requests(
    runtime: &Runtime,
    expression: &str,
    duration_ms: Option<u64>,
) -> CliResult<HighlightComputation> {
    let results = runtime.evaluate(None, expression).map_err(map_evaluate_error)?;
    let mut requests = Vec::new();
    let mut skipped = Vec::new();

    for item in results {
        let EvaluationItem::Node(node) = item else {
            continue;
        };

        let Some(attribute) = node.attribute(Namespace::Control, "Bounds") else {
            skipped.push(node.runtime_id().as_str().to_owned());
            continue;
        };

        let value = attribute.value();
        let UiValue::Rect(bounds) = value else {
            skipped.push(node.runtime_id().as_str().to_owned());
            continue;
        };

        if bounds.is_empty() {
            skipped.push(node.runtime_id().as_str().to_owned());
            continue;
        }

        let request = if let Some(ms) = duration_ms {
            HighlightRequest::new(bounds).with_duration(Duration::from_millis(ms))
        } else {
            HighlightRequest::new(bounds)
        };
        requests.push(request);
    }

    Ok(HighlightComputation { requests, skipped })
}

fn parse_duration_ms(value: &str) -> Result<u64, String> {
    value.parse::<u64>().map_err(|_| format!("invalid duration in milliseconds: {value}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::map_provider_error;
    use platynui_platform_mock::{
        highlight_clear_count, reset_highlight_state, take_highlight_log,
    };
    use platynui_runtime::Runtime;
    use rstest::rstest;

    #[rstest]
    fn highlight_records_requests() {
        reset_highlight_state();
        let mut runtime = Runtime::new().map_err(map_provider_error).expect("runtime");

        let args = HighlightArgs {
            expression: Some("//control:Button".into()),
            duration_ms: Some(500),
            clear: false,
        };

        let output = run(&runtime, &args).expect("highlight execution");
        assert!(output.contains("Highlighted"));

        let log = take_highlight_log();
        assert!(!log.is_empty());
        assert_eq!(log[0][0].duration, Some(Duration::from_millis(500)));

        reset_highlight_state();
        runtime.shutdown();
    }

    #[rstest]
    fn highlight_clear_only_triggers_provider_clear() {
        reset_highlight_state();
        let mut runtime = Runtime::new().map_err(map_provider_error).expect("runtime");

        let args = HighlightArgs { expression: None, duration_ms: None, clear: true };
        let output = run(&runtime, &args).expect("highlight clear");
        assert!(output.contains("Cleared"));
        assert_eq!(highlight_clear_count(), 1);

        reset_highlight_state();
        runtime.shutdown();
    }

    #[rstest]
    fn highlight_requires_expression_or_clear() {
        let mut runtime = Runtime::new().map_err(map_provider_error).expect("runtime");

        let err =
            run(&runtime, &HighlightArgs { expression: None, duration_ms: None, clear: false })
                .expect_err("missing expression should error");
        assert!(err.to_string().contains("requires"));

        runtime.shutdown();
    }
}
