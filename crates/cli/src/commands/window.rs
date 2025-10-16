use crate::util::{CliResult, map_evaluate_error, yes_no};
use anyhow::bail;
use clap::Args;
use owo_colors::{OwoColorize, Stream};
use platynui_core::types::{Point, Rect, Size};
use platynui_core::ui::attribute_names::{element, window_surface};
use platynui_core::ui::{Namespace, UiNode, UiValue, WindowSurfaceActions, WindowSurfacePattern};
use platynui_runtime::{EvaluationItem, Runtime};
use std::collections::HashSet;
use std::sync::Arc;

const DEFAULT_WINDOW_QUERY: &str = "//control:Window";

#[derive(Args, Debug, Clone)]
pub struct WindowArgs {
    #[arg(value_name = "XPATH", help = "XPath expression selecting windows.", required_unless_present = "list")]
    pub expression: Option<String>,

    #[arg(
        long,
        help = "List windows and their state without performing actions.",
        conflicts_with_all = ["activate", "minimize", "maximize", "restore", "close", "move_to", "resize"]
    )]
    pub list: bool,

    #[arg(long, help = "Activate the selected windows.")]
    pub activate: bool,
    #[arg(long, help = "Minimize the selected windows.")]
    pub minimize: bool,
    #[arg(long, help = "Maximize the selected windows.")]
    pub maximize: bool,
    #[arg(long, help = "Restore the selected windows.")]
    pub restore: bool,
    #[arg(long, help = "Close the selected windows.")]
    pub close: bool,
    #[arg(
        long = "move",
        value_names = ["X", "Y"],
        num_args = 2,
        value_parser = clap::value_parser!(f64),
        help = "Move windows to the given desktop coordinates."
    )]
    pub move_to: Option<Vec<f64>>,
    #[arg(
        long,
        value_names = ["WIDTH", "HEIGHT"],
        num_args = 2,
        value_parser = clap::value_parser!(f64),
        help = "Resize windows to the given size."
    )]
    pub resize: Option<Vec<f64>>,
}

pub fn run(runtime: &Runtime, args: &WindowArgs) -> CliResult<String> {
    if args.list {
        return list_windows(runtime, args);
    }

    let actions = WindowActions::from_args(args)?;
    if actions.is_empty() {
        bail!("no window action specified");
    }

    let expression = args.expression.as_deref().unwrap_or(DEFAULT_WINDOW_QUERY);

    execute_actions(runtime, expression, &actions)
}

fn list_windows(runtime: &Runtime, args: &WindowArgs) -> CliResult<String> {
    let expression = args.expression.as_deref().unwrap_or(DEFAULT_WINDOW_QUERY);
    let results = runtime.evaluate(None, expression).map_err(map_evaluate_error)?;
    let mut seen = HashSet::<String>::new();
    let mut windows = Vec::new();

    for item in results {
        let EvaluationItem::Node(node) = item else { continue };
        let runtime_id = node.runtime_id().as_str().to_owned();
        if !seen.insert(runtime_id.clone()) {
            continue;
        }
        windows.push((runtime_id, node));
    }

    if windows.is_empty() {
        return Ok(format!("No windows matched expression `{}`.", expression));
    }

    let mut lines = Vec::new();
    lines.push(format!("Windows ({}):", windows.len()));

    for (runtime_id, node) in windows {
        lines.push(render_window_header(&node));
        lines.push(format!("    RuntimeId: {runtime_id}"));

        if let Some(state) = window_state(&node) {
            lines.push(format!(
                "    State: minimized={}, maximized={}, topmost={}, accepts_input={}",
                yes_no(state.is_minimized),
                yes_no(state.is_maximized),
                yes_no(state.is_topmost),
                yes_no(state.accepts_user_input)
            ));
            lines.push(format!(
                "    Bounds: x={}, y={}, width={}, height={}",
                format_number(state.bounds.x()),
                format_number(state.bounds.y()),
                format_number(state.bounds.width()),
                format_number(state.bounds.height())
            ));
            lines.push(format!(
                "    Capabilities: move={}, resize={}",
                yes_no(state.supports_move),
                yes_no(state.supports_resize)
            ));
        } else {
            lines.push("    WindowSurface pattern not available.".to_owned());
        }
    }

    Ok(lines.join("\n"))
}

fn execute_actions(runtime: &Runtime, expression: &str, actions: &WindowActions) -> CliResult<String> {
    let results = runtime.evaluate(None, expression).map_err(map_evaluate_error)?;
    let mut seen = HashSet::<String>::new();

    let mut applied = Vec::new();
    let mut missing_pattern = Vec::new();
    let mut failed = Vec::new();

    for item in results {
        let EvaluationItem::Node(node) = item else { continue };
        let runtime_id = node.runtime_id().as_str().to_owned();
        if !seen.insert(runtime_id.clone()) {
            continue;
        }

        let Some(pattern) = node.pattern::<WindowSurfaceActions>() else {
            missing_pattern.push(render_window_header(&node));
            continue;
        };

        match apply_actions(&node, &pattern, actions) {
            Ok(descriptions) => {
                let summary = if descriptions.is_empty() { "no changes".to_owned() } else { descriptions.join(", ") };
                applied.push(format!("- {}: {summary}", render_window_header(&node)));
            }
            Err(err) => {
                failed.push(format!("- {}: {}", render_window_header(&node), err.message()));
            }
        }
    }

    if applied.is_empty() && missing_pattern.is_empty() && failed.is_empty() {
        anyhow::bail!("expression `{expression}` did not match any windows");
    }

    let mut lines = Vec::new();

    if !applied.is_empty() {
        lines.push("Window actions applied:".to_owned());
        lines.extend(applied);
    }

    if !missing_pattern.is_empty() {
        lines.push("Skipped (missing WindowSurface pattern):".to_owned());
        lines.extend(missing_pattern.into_iter().map(|entry| format!("- {entry}")));
    }

    if !failed.is_empty() {
        lines.push("Failed:".to_owned());
        lines.extend(failed);
    }

    Ok(lines.join("\n"))
}

fn apply_actions(
    node: &Arc<dyn UiNode>,
    pattern: &Arc<WindowSurfaceActions>,
    actions: &WindowActions,
) -> Result<Vec<String>, platynui_core::ui::PatternError> {
    let mut descriptions = Vec::new();

    if actions.activate {
        pattern.activate()?;
        descriptions.push("activated".to_owned());
    }
    if actions.minimize {
        pattern.minimize()?;
        descriptions.push("minimized".to_owned());
    }
    if actions.maximize {
        pattern.maximize()?;
        descriptions.push("maximized".to_owned());
    }
    if actions.restore {
        pattern.restore()?;
        descriptions.push("restored".to_owned());
    }
    if actions.close {
        pattern.close()?;
        descriptions.push("closed".to_owned());
    }
    if let Some(point) = actions.move_to {
        pattern.move_to(point)?;
        descriptions.push(format!("moved to ({}, {})", format_number(point.x()), format_number(point.y())));
    }
    if let Some(size) = actions.resize {
        pattern.resize(size)?;
        descriptions.push(format!("resized to {}×{}", format_number(size.width()), format_number(size.height())));
    }

    if let Some(state) = window_state(node) {
        descriptions.push(format!(
            "state: minimized={}, maximized={}, topmost={}, accepts_input={}",
            yes_no(state.is_minimized),
            yes_no(state.is_maximized),
            yes_no(state.is_topmost),
            yes_no(state.accepts_user_input)
        ));
    }

    Ok(descriptions)
}

fn render_window_header(node: &Arc<dyn UiNode>) -> String {
    let namespace = node.namespace().as_str();
    let prefix = if namespace == Namespace::Control.as_str() { String::new() } else { format!("{namespace}:") };

    let role = node.role();
    let name = node.name();
    let label = if name.is_empty() {
        format!("{prefix}{role}")
    } else {
        let quoted = serde_json::to_string(&name).unwrap_or_else(|_| format!("\"{name}\""));
        format!("{prefix}{role} {quoted}")
    };

    label.if_supports_color(Stream::Stdout, |text| text.bold().fg_rgb::<255, 184, 79>().to_string()).to_string()
}

fn format_number(value: f64) -> String {
    if (value.fract()).abs() < f64::EPSILON { format!("{:.0}", value) } else { format!("{:.2}", value) }
}

fn window_state(node: &Arc<dyn UiNode>) -> Option<WindowStatus> {
    let bounds =
        node.attribute(Namespace::Control, element::BOUNDS).map(|attr| attr.value()).and_then(|value| match value {
            UiValue::Rect(rect) => Some(rect),
            _ => None,
        })?;

    Some(WindowStatus {
        bounds,
        is_minimized: attr_bool(node, window_surface::IS_MINIMIZED),
        is_maximized: attr_bool(node, window_surface::IS_MAXIMIZED),
        is_topmost: attr_bool(node, window_surface::IS_TOPMOST),
        supports_move: attr_bool(node, window_surface::SUPPORTS_MOVE),
        supports_resize: attr_bool(node, window_surface::SUPPORTS_RESIZE),
        accepts_user_input: attr_bool(node, window_surface::ACCEPTS_USER_INPUT),
    })
}

fn attr_bool(node: &Arc<dyn UiNode>, name: &str) -> bool {
    node.attribute(Namespace::Control, name)
        .map(|attr| attr.value())
        .and_then(|value| match value {
            UiValue::Bool(v) => Some(v),
            UiValue::Integer(v) => Some(v != 0),
            UiValue::Number(v) => Some(v != 0.0),
            _ => None,
        })
        .unwrap_or(false)
}

#[derive(Clone, Copy)]
struct WindowStatus {
    bounds: Rect,
    is_minimized: bool,
    is_maximized: bool,
    is_topmost: bool,
    supports_move: bool,
    supports_resize: bool,
    accepts_user_input: bool,
}

#[derive(Clone, Copy)]
struct WindowActions {
    activate: bool,
    minimize: bool,
    maximize: bool,
    restore: bool,
    close: bool,
    move_to: Option<Point>,
    resize: Option<Size>,
}

impl WindowActions {
    fn from_args(args: &WindowArgs) -> CliResult<Self> {
        let move_to = args.move_to.as_ref().map(|values| Point::new(values[0], values[1]));
        let resize = args.resize.as_ref().map(|values| Size::new(values[0], values[1]));

        Ok(Self {
            activate: args.activate,
            minimize: args.minimize,
            maximize: args.maximize,
            restore: args.restore,
            close: args.close,
            move_to,
            resize,
        })
    }

    fn is_empty(&self) -> bool {
        !self.activate
            && !self.minimize
            && !self.maximize
            && !self.restore
            && !self.close
            && self.move_to.is_none()
            && self.resize.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::runtime;
    use crate::util::yes_no;
    use platynui_runtime::Runtime;
    use rstest::rstest;
    use serial_test::serial;

    #[rstest]
    #[serial]
    fn list_windows_outputs_state(runtime: Runtime) {
        let args = WindowArgs {
            expression: None,
            list: true,
            activate: false,
            minimize: false,
            maximize: false,
            restore: false,
            close: false,
            move_to: None,
            resize: None,
        };
        let output = run(&runtime, &args).expect("list windows");
        assert!(output.contains("Windows"));
        assert!(output.contains("Operations Console"));
        assert!(output.contains(yes_no(false)));
    }

    #[rstest]
    #[serial]
    fn window_actions_apply_sequence(runtime: Runtime) {
        let args = WindowArgs {
            expression: Some("//control:Window[@Name='Operations Console']".into()),
            list: false,
            activate: true,
            minimize: true,
            maximize: false,
            restore: true,
            close: false,
            move_to: Some(vec![200.0, 220.0]),
            resize: Some(vec![800.0, 600.0]),
        };

        let output = run(&runtime, &args).expect("window actions");
        assert!(output.contains("Window actions applied"));
        assert!(output.contains("Operations Console"));
        assert!(output.contains("moved to"));
        assert!(output.contains("resized"));
    }

    #[rstest]
    #[serial]
    fn window_actions_require_match(runtime: Runtime) {
        let args = WindowArgs {
            expression: Some("//control:Window[@Name='Nonexistent']".into()),
            list: false,
            activate: true,
            minimize: false,
            maximize: false,
            restore: false,
            close: false,
            move_to: None,
            resize: None,
        };

        let err = run(&runtime, &args).expect_err("no match should error");
        assert!(err.to_string().contains("did not match any windows"));
    }
}
