use crate::util::{CliResult, map_evaluate_error};
use clap::Args;
use owo_colors::{OwoColorize, Stream};
use platynui_core::ui::{Namespace, UiNode};
use platynui_runtime::{EvaluationItem, FocusError, Runtime};
use std::sync::Arc;

#[derive(Args, Debug, Clone)]
pub struct FocusArgs {
    #[arg(value_name = "XPATH", help = "XPath expression selecting nodes to focus.")]
    pub expression: String,
}

pub fn run(runtime: &Runtime, args: &FocusArgs) -> CliResult<String> {
    let item = runtime.evaluate_single(None, &args.expression).map_err(map_evaluate_error)?;
    let Some(EvaluationItem::Node(node)) = item else {
        anyhow::bail!("expression `{}` did not match any nodes", args.expression);
    };

    let mut lines = Vec::new();
    match runtime.focus(&node) {
        Ok(()) => {
            lines.push("Focused 1 node(s):".to_owned());
            lines.push(format!("- {}", render_node(&node)));
        }
        Err(FocusError::PatternMissing { .. }) => {
            lines.push("Focused 0 node(s).".to_owned());
            lines.push("Skipped (missing Focusable pattern):".to_owned());
            lines.push(format!("- {}", render_node(&node)));
        }
        Err(FocusError::ActionFailed { source, .. }) => {
            lines.push("Focused 0 node(s).".to_owned());
            lines.push("Failed to focus:".to_owned());
            lines.push(format!("- {}: {}", render_node(&node), source.message()));
        }
    }

    Ok(lines.join("\n"))
}

fn render_node(node: &Arc<dyn UiNode>) -> String {
    let namespace = node.namespace().as_str();
    let prefix = if namespace == Namespace::Control.as_str() { String::new() } else { format!("{namespace}:") };

    let role = node.role();
    let name = node.name();

    let label = if name.is_empty() {
        format!("{prefix}{role}")
    } else {
        let quoted_name = serde_json::to_string(&name).unwrap_or_else(|_| format!("\"{name}\""));
        format!("{prefix}{role} {quoted_name}")
    };

    let colored_label =
        label.if_supports_color(Stream::Stdout, |text| text.bold().fg_rgb::<79, 166, 255>().to_string()).to_string();

    format!("{colored_label} ({})", node.runtime_id().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::runtime;
    use crate::util::map_evaluate_error;
    use platynui_core::ui::UiValue;
    use platynui_core::ui::attribute_names::focusable;
    use platynui_runtime::Runtime;
    use rstest::rstest;
    use serial_test::serial;

    #[rstest]
    #[serial]
    fn focus_command_sets_focus(runtime: Runtime) {
        let args = FocusArgs { expression: "//control:Button[@Name='OK']".into() };

        let output = run(&runtime, &args).expect("focus execution");
        assert!(output.contains("Focused 1 node"));
        assert!(output.contains("mock://button/ok"));

        let results =
            runtime.evaluate(None, "//control:Button[@Name='OK']").map_err(map_evaluate_error).expect("evaluation");

        let focused = results
            .into_iter()
            .find_map(|item| match item {
                EvaluationItem::Node(node) => Some(node),
                _ => None,
            })
            .expect("button node present");

        let attr = focused.attribute(Namespace::Control, focusable::IS_FOCUSED).expect("focus attribute");
        assert_eq!(attr.value(), UiValue::from(true));
    }

    #[rstest]
    #[serial]
    fn focus_command_reports_missing_pattern(runtime: Runtime) {
        let args = FocusArgs { expression: "//control:Panel[@Name='Workspace']".into() };

        let output = run(&runtime, &args).expect("focus execution");
        assert!(output.contains("Skipped (missing Focusable pattern)"));
        assert!(output.contains("mock://panel/workspace"));
        assert!(output.contains("Focused 0 node"));
    }

    #[rstest]
    #[serial]
    fn focus_command_errors_on_empty_result(runtime: Runtime) {
        let args = FocusArgs { expression: "//control:Button[@Name='Nonexistent']".into() };

        let err = run(&runtime, &args).expect_err("no node should error");
        assert!(err.to_string().contains("did not match any nodes"));
    }
}
