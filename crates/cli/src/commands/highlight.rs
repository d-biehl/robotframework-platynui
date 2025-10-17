use crate::util::{CliResult, map_evaluate_error, map_platform_error};
use clap::Args;
use platynui_core::platform::HighlightRequest;
use platynui_core::types::Rect;
use platynui_core::ui::{Namespace, UiValue};
use platynui_runtime::{EvaluationItem, Runtime};
use std::time::Duration;

#[derive(Args, Debug, Clone)]
pub struct HighlightArgs {
    #[arg(value_name = "XPATH", conflicts_with = "rect")]
    pub expression: Option<String>,
    #[arg(
        long = "rect",
        value_parser = parse_rect_arg,
        value_name = "X,Y,WIDTH,HEIGHT",
        allow_hyphen_values = true,
        help = "Highlight a specific rectangle in desktop coordinates. Conflicts with XPATH."
    )]
    pub rect: Option<Rect>,
    #[arg(
        long = "duration-ms",
        default_value_t = 1500u64,
        help = "Duration in milliseconds the highlight stays visible before fading (default: 1500)."
    )]
    pub duration_ms: u64,
    #[arg(long = "clear", help = "Clear existing highlights before applying a new one or alone.")]
    pub clear: bool,
}

pub fn run(runtime: &Runtime, args: &HighlightArgs) -> CliResult<String> {
    if args.expression.is_none() && args.rect.is_none() && !args.clear {
        anyhow::bail!("highlight requires an XPath expression or --rect unless --clear is set");
    }

    let mut messages = Vec::new();

    if args.clear {
        runtime.clear_highlight().map_err(map_platform_error)?;
        messages.push("Cleared existing highlights.".to_owned());
    }

    let mut highlighted = 0usize;
    let mut hold_ms: Option<u64> = None;
    if let Some(rect) = args.rect {
        let req = HighlightRequest::new(rect).with_duration(Duration::from_millis(args.duration_ms));
        runtime.highlight(&req).map_err(map_platform_error)?;
        highlighted = 1;
        hold_ms = Some(args.duration_ms);
    } else if let Some(expression) = &args.expression {
        let highlight = collect_highlight_requests(runtime, expression, Some(args.duration_ms))?;
        if highlight.rects.is_empty() {
            anyhow::bail!("no highlightable nodes for expression `{expression}`");
        }
        let mut req = HighlightRequest::from_rects(highlight.rects);
        if let Some(ms) = highlight.duration_ms {
            req = req.with_duration(Duration::from_millis(ms));
        }
        runtime.highlight(&req).map_err(map_platform_error)?;
        highlighted = 1;
        hold_ms = Some(args.duration_ms);
        if !highlight.skipped.is_empty() {
            let skipped = highlight.skipped.join(", ");
            messages.push(format!("Skipped nodes without Bounds: {skipped}."));
        }
    }

    // Keep the process alive so the overlay stays visible until the duration elapses.
    if let Some(ms) = hold_ms
        && !cfg!(test)
    {
        std::thread::sleep(Duration::from_millis(ms));
    }

    if messages.is_empty() {
        if highlighted > 0 {
            messages.push(format!("Highlighted {} region(s).", highlighted));
        } else {
            messages.push("No highlight action executed.".to_owned());
        }
    }

    Ok(messages.join("\n"))
}

struct HighlightComputation {
    rects: Vec<Rect>,
    duration_ms: Option<u64>,
    skipped: Vec<String>,
}

fn collect_highlight_requests(
    runtime: &Runtime,
    expression: &str,
    duration_ms: Option<u64>,
) -> CliResult<HighlightComputation> {
    let results = runtime.evaluate(None, expression).map_err(map_evaluate_error)?;
    let mut rects = Vec::new();
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

        rects.push(bounds);
    }
    Ok(HighlightComputation { rects, duration_ms, skipped })
}

fn parse_rect_arg(value: &str) -> Result<Rect, String> {
    let parts: Vec<_> = value.split(',').collect();
    if parts.len() != 4 {
        return Err(format!("expected four comma-separated values, got `{value}`"));
    }
    let mut nums = [0f64; 4];
    for (i, part) in parts.iter().enumerate() {
        nums[i] = part.trim().parse::<f64>().map_err(|_| format!("invalid number in rect `{value}`"))?;
    }
    Ok(Rect::new(nums[0], nums[1], nums[2], nums[3]))
}

#[cfg(test)]
mod tests {
    use super::*;
    // Link platform-mock inventory for highlight provider
    use crate::test_support::runtime;
    use platynui_platform_mock as _; // link platform-mock inventory
    use platynui_platform_mock::{highlight_clear_count, reset_highlight_state, take_highlight_log};
    use platynui_runtime::Runtime;
    use rstest::rstest;
    use std::sync::{LazyLock, Mutex};

    static TEST_GUARD: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    #[rstest]
    fn highlight_records_requests(runtime: Runtime) {
        let _lock = TEST_GUARD.lock().unwrap();
        reset_highlight_state();

        let args =
            HighlightArgs { expression: Some("//control:Button".into()), rect: None, duration_ms: 500, clear: false };

        let output = run(&runtime, &args).expect("highlight execution");
        assert!(output.contains("Highlighted"));

        let log = take_highlight_log();
        assert!(!log.is_empty());
        assert_eq!(log[0].duration, Some(Duration::from_millis(500)));

        reset_highlight_state();
    }

    #[rstest]
    fn highlight_clear_only_triggers_provider_clear(runtime: Runtime) {
        let _lock = TEST_GUARD.lock().unwrap();
        reset_highlight_state();

        let args = HighlightArgs { expression: None, rect: None, duration_ms: 1500, clear: true };
        let output = run(&runtime, &args).expect("highlight clear");
        assert!(output.contains("Cleared"));
        assert_eq!(highlight_clear_count(), 1);

        reset_highlight_state();
    }

    #[rstest]
    fn highlight_requires_expression_or_clear(runtime: Runtime) {
        let _lock = TEST_GUARD.lock().unwrap();

        let err = run(&runtime, &HighlightArgs { expression: None, rect: None, duration_ms: 1500, clear: false })
            .expect_err("missing expression or rect should error");
        assert!(err.to_string().contains("requires"));
    }

    #[rstest]
    fn highlight_rect_path_uses_default_duration(runtime: Runtime) {
        let _lock = TEST_GUARD.lock().unwrap();
        reset_highlight_state();

        let args = HighlightArgs {
            expression: None,
            rect: Some(Rect::new(10.0, 10.0, 40.0, 20.0)),
            duration_ms: 1500,
            clear: false,
        };
        let output = run(&runtime, &args).expect("highlight rect");
        assert!(output.contains("Highlighted 1 region"));
        let log = take_highlight_log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].duration, Some(Duration::from_millis(1500)));

        reset_highlight_state();
    }
}
