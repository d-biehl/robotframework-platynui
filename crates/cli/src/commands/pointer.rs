use clap::{Args, Subcommand, ValueEnum};
use platynui_core::platform::{PointOrigin, PointerAccelerationProfile, PointerButton, PointerMotionMode, ScrollDelta};
use platynui_core::types::{Point, Rect};
use platynui_core::ui::attribute_names::{activation_target, common, element};
use platynui_core::ui::{Namespace, UiNode, UiValue};
use platynui_runtime::{EvaluationItem, PointerError, PointerOverrides, Runtime};
use std::sync::Arc;
use std::time::Duration;

use crate::util::{CliResult, map_evaluate_error, parse_point, parse_pointer_button, parse_scroll_delta};

#[derive(Args)]
pub struct PointerArgs {
    #[command(subcommand)]
    pub command: PointerCommand,
}

#[derive(Subcommand)]
pub enum PointerCommand {
    Move(PointerMoveArgs),
    Click(PointerClickArgs),
    MultiClick(PointerMultiClickArgs),
    Press(PointerPressArgs),
    Release(PointerReleaseArgs),
    Scroll(PointerScrollArgs),
    Drag(PointerDragArgs),
    Position,
}

#[derive(Args)]
#[command(about = "Move the pointer to a point or XPath-selected element.")]
pub struct PointerMoveArgs {
    #[arg(
        value_name = "XPATH",
        help = "XPath selecting a node (preferred). Uses @ActivationPoint if available, otherwise the center of @Bounds."
    )]
    pub expression: Option<String>,
    #[arg(long = "point", value_parser = parse_point_arg, allow_hyphen_values = true, help = "Target point as 'x,y' (used if no XPATH is provided).")]
    pub point: Option<Point>,
    #[command(flatten)]
    overrides: OverrideArgs,
}

#[derive(Args)]
#[command(about = "Click at a point or on an XPath-selected element.")]
pub struct PointerClickArgs {
    #[arg(
        value_name = "XPATH",
        help = "XPath selecting a node (preferred). Uses @ActivationPoint if available, otherwise the center of @Bounds."
    )]
    pub expression: Option<String>,
    #[arg(long = "point", value_parser = parse_point_arg, allow_hyphen_values = true, help = "Target point as 'x,y' (used if no XPATH is provided).")]
    pub point: Option<Point>,
    #[arg(long = "button", default_value = "left", value_parser = parse_pointer_button_arg, help = "Mouse button (left/right/middle or numeric code).")]
    pub button: PointerButton,
    #[arg(long = "no-move", help = "Perform the click without moving the pointer.")]
    pub no_move: bool,
    #[command(flatten)]
    overrides: OverrideArgs,
}

#[derive(Args)]
#[command(about = "Perform multiple clicks at a point or on an XPath-selected element.")]
pub struct PointerMultiClickArgs {
    #[arg(
        value_name = "XPATH",
        help = "XPath selecting a node (preferred). Uses @ActivationPoint if available, otherwise the center of @Bounds."
    )]
    pub expression: Option<String>,
    #[arg(long = "point", value_parser = parse_point_arg, help = "Target point as 'x,y' (used if no XPATH is provided).")]
    pub point: Option<Point>,
    #[arg(long = "button", default_value = "left", value_parser = parse_pointer_button_arg, help = "Mouse button (left/right/middle or numeric code).")]
    pub button: PointerButton,
    #[arg(long = "count", default_value_t = 2, value_parser = parse_click_count, help = "Number of clicks (>=2).")]
    pub count: u32,
    #[arg(long = "no-move", help = "Perform the clicks without moving the pointer.")]
    pub no_move: bool,
    #[command(flatten)]
    overrides: OverrideArgs,
}

#[derive(Args)]
#[command(about = "Press a mouse button at a point or XPath-selected element.")]
pub struct PointerPressArgs {
    #[arg(
        value_name = "XPATH",
        help = "XPath selecting a node (preferred). Uses @ActivationPoint if available, otherwise the center of @Bounds."
    )]
    pub expression: Option<String>,
    #[arg(long = "point", value_parser = parse_point_arg, allow_hyphen_values = true, help = "Optional target point 'x,y'.")]
    pub point: Option<Point>,
    #[arg(long = "button", default_value = "left", value_parser = parse_pointer_button_arg, help = "Mouse button (left/right/middle or numeric code).")]
    pub button: PointerButton,
    #[arg(long = "no-move", help = "Perform the press without moving the pointer.")]
    pub no_move: bool,
    #[command(flatten)]
    overrides: OverrideArgs,
}

#[derive(Args)]
#[command(about = "Release a mouse button at the current or XPath-selected location.")]
pub struct PointerReleaseArgs {
    #[arg(long = "button", default_value = "left", value_parser = parse_pointer_button_arg, help = "Mouse button (left/right/middle or numeric code).")]
    pub button: PointerButton,
    #[arg(value_name = "XPATH", help = "XPath selecting a node to move to before release (unless --no-move).")]
    pub expression: Option<String>,
    #[arg(long = "no-move", help = "Perform the release without moving the pointer.")]
    pub no_move: bool,
    #[command(flatten)]
    overrides: OverrideArgs,
}

#[derive(Args)]
#[command(about = "Scroll, optionally targeting an element via XPath.")]
pub struct PointerScrollArgs {
    #[arg(value_parser = parse_scroll_delta_arg, allow_hyphen_values = true, help = "Scroll delta as 'x,y'.")]
    pub delta: ScrollDelta,
    #[arg(
        long = "expr",
        value_name = "XPATH",
        help = "Optional XPath selecting a node to move to before scrolling (unless --no-move)."
    )]
    pub expr: Option<String>,
    #[arg(long = "no-move", help = "Perform the scroll without moving the pointer.")]
    pub no_move: bool,
    #[command(flatten)]
    overrides: OverrideArgs,
}

#[derive(Args)]
#[command(about = "Drag from one point/element to another.")]
pub struct PointerDragArgs {
    #[arg(long = "from", value_parser = parse_point_arg, allow_hyphen_values = true, help = "Start point 'x,y' (or use --from-expr).")]
    pub from: Point,
    #[arg(long = "to", value_parser = parse_point_arg, allow_hyphen_values = true, help = "End point 'x,y' (or use --to-expr).")]
    pub to: Point,
    #[arg(long = "from-expr", value_name = "XPATH", help = "XPath selecting the start node.")]
    pub from_expr: Option<String>,
    #[arg(long = "to-expr", value_name = "XPATH", help = "XPath selecting the end node.")]
    pub to_expr: Option<String>,
    #[arg(long = "button", default_value = "left", value_parser = parse_pointer_button_arg, help = "Mouse button (left/right/middle or numeric code).")]
    pub button: PointerButton,
    #[command(flatten)]
    overrides: OverrideArgs,
}

#[derive(Clone, Copy, ValueEnum, Default)]
enum OriginKind {
    #[default]
    Desktop,
    Bounds,
    Absolute,
}

#[derive(Clone, Copy, ValueEnum)]
enum MotionKind {
    Direct,
    Linear,
    Bezier,
    Overshoot,
    Jitter,
}

#[derive(Clone, Copy, ValueEnum)]
enum AccelerationKind {
    Constant,
    EaseIn,
    EaseOut,
    SmoothStep,
}

#[derive(Args, Default)]
struct OverrideArgs {
    #[arg(long = "origin", value_enum, default_value_t = OriginKind::Desktop)]
    origin: OriginKind,
    #[arg(long = "bounds", allow_hyphen_values = true)]
    bounds: Option<String>,
    #[arg(long = "anchor", allow_hyphen_values = true)]
    anchor: Option<String>,
    #[arg(long = "motion", value_enum)]
    motion: Option<MotionKind>,
    #[arg(long = "after-move", value_parser = parse_millis)]
    after_move_delay: Option<Duration>,
    #[arg(long = "after-input", value_parser = parse_millis)]
    after_input_delay: Option<Duration>,
    #[arg(long = "press-release", value_parser = parse_millis)]
    press_release_delay: Option<Duration>,
    #[arg(long = "after-click", value_parser = parse_millis)]
    after_click_delay: Option<Duration>,
    #[arg(long = "before-next", value_parser = parse_millis)]
    before_next_click_delay: Option<Duration>,
    #[arg(long = "multi-click", value_parser = parse_millis)]
    multi_click_delay: Option<Duration>,
    #[arg(long = "ensure-threshold")]
    ensure_move_threshold: Option<f64>,
    #[arg(long = "ensure-timeout", value_parser = parse_millis)]
    ensure_move_timeout: Option<Duration>,
    #[arg(long = "scroll-delay", value_parser = parse_millis)]
    scroll_delay: Option<Duration>,
    #[arg(long = "scroll-step", value_parser = parse_scroll_delta_arg, allow_hyphen_values = true)]
    scroll_step: Option<ScrollDelta>,
    #[arg(long = "move-duration", value_parser = parse_millis)]
    move_duration: Option<Duration>,
    #[arg(long = "move-time-per-pixel", value_parser = parse_millis)]
    move_time_per_pixel: Option<Duration>,
    #[arg(long = "speed-factor")]
    speed_factor: Option<f64>,
    #[arg(long = "acceleration", value_enum)]
    acceleration: Option<AccelerationKind>,
}

pub fn run(runtime: &Runtime, args: &PointerArgs) -> CliResult<String> {
    match &args.command {
        PointerCommand::Move(move_args) => run_move(runtime, move_args),
        PointerCommand::Click(click_args) => run_click(runtime, click_args),
        PointerCommand::MultiClick(multi_click_args) => run_multi_click(runtime, multi_click_args),
        PointerCommand::Press(press_args) => run_press(runtime, press_args),
        PointerCommand::Release(release_args) => run_release(runtime, release_args),
        PointerCommand::Scroll(scroll_args) => run_scroll(runtime, scroll_args),
        PointerCommand::Drag(drag_args) => run_drag(runtime, drag_args),
        PointerCommand::Position => run_position(runtime),
    }
}

fn run_move(runtime: &Runtime, args: &PointerMoveArgs) -> CliResult<String> {
    let overrides = build_overrides(runtime, &args.overrides)?;
    let (target, element_info) = match (&args.expression, &args.point) {
        (Some(expr), _) => {
            let (point, node) = resolve_point_and_node_from_expr(runtime, expr)?;
            (point, Some(format_element_info(&node)))
        }
        (None, Some(p)) => (*p, None),
        (None, None) => anyhow::bail!("either --expr or a point must be provided"),
    };
    runtime.pointer_move_to(target, overrides).map_err(map_pointer_error)?;
    if let Some(info) = element_info { Ok(format!("Moved pointer to element: {}", info)) } else { Ok(String::new()) }
}

fn run_click(runtime: &Runtime, args: &PointerClickArgs) -> CliResult<String> {
    let overrides = build_overrides(runtime, &args.overrides)?;
    let element_info = if args.no_move {
        // Do not move: perform press+release at current location
        runtime.pointer_press(None, Some(args.button), overrides.clone()).map_err(map_pointer_error)?;
        runtime.pointer_release(Some(args.button), overrides).map_err(map_pointer_error)?;
        None
    } else {
        let (target, info) = match (&args.expression, &args.point) {
            (Some(expr), _) => {
                let (point, node) = resolve_point_and_node_from_expr(runtime, expr)?;
                (point, Some(format_element_info(&node)))
            }
            (None, Some(p)) => (*p, None),
            (None, None) => anyhow::bail!("either --expr or a point must be provided"),
        };
        runtime.pointer_click(target, Some(args.button), overrides).map_err(map_pointer_error)?;
        info
    };
    if let Some(info) = element_info { Ok(format!("Clicked on element: {}", info)) } else { Ok(String::new()) }
}

fn run_multi_click(runtime: &Runtime, args: &PointerMultiClickArgs) -> CliResult<String> {
    let overrides = build_overrides(runtime, &args.overrides)?;
    let element_info = if args.no_move {
        for _ in 0..args.count {
            runtime.pointer_press(None, Some(args.button), overrides.clone()).map_err(map_pointer_error)?;
            runtime.pointer_release(Some(args.button), overrides.clone()).map_err(map_pointer_error)?;
        }
        None
    } else {
        let (target, info) = match (&args.expression, &args.point) {
            (Some(expr), _) => {
                let (point, node) = resolve_point_and_node_from_expr(runtime, expr)?;
                (point, Some(format_element_info(&node)))
            }
            (None, Some(p)) => (*p, None),
            (None, None) => anyhow::bail!("either --expr or a point must be provided"),
        };
        runtime.pointer_multi_click(target, Some(args.button), args.count, overrides).map_err(map_pointer_error)?;
        info
    };
    if let Some(info) = element_info {
        Ok(format!("Multi-clicked {} times on element: {}", args.count, info))
    } else {
        Ok(String::new())
    }
}

fn run_press(runtime: &Runtime, args: &PointerPressArgs) -> CliResult<String> {
    let overrides = build_overrides(runtime, &args.overrides)?;
    let (target, element_info) = if args.no_move {
        (None, None)
    } else if let Some(expr) = &args.expression {
        let (point, node) = resolve_point_and_node_from_expr(runtime, expr)?;
        (Some(point), Some(format_element_info(&node)))
    } else {
        (args.point, None)
    };
    runtime.pointer_press(target, Some(args.button), overrides).map_err(map_pointer_error)?;
    if let Some(info) = element_info {
        Ok(format!("Pressed mouse button on element: {}", info))
    } else {
        Ok(String::new())
    }
}

fn run_release(runtime: &Runtime, args: &PointerReleaseArgs) -> CliResult<String> {
    let overrides = build_overrides(runtime, &args.overrides)?;
    let element_info = if !args.no_move {
        if let Some(expr) = &args.expression {
            let (target, node) = resolve_point_and_node_from_expr(runtime, expr)?;
            let _ = runtime.pointer_move_to(target, overrides.clone()).map_err(map_pointer_error)?;
            Some(format_element_info(&node))
        } else {
            None
        }
    } else {
        None
    };
    runtime.pointer_release(Some(args.button), overrides).map_err(map_pointer_error)?;
    if let Some(info) = element_info {
        Ok(format!("Released mouse button on element: {}", info))
    } else {
        Ok(String::new())
    }
}

fn run_scroll(runtime: &Runtime, args: &PointerScrollArgs) -> CliResult<String> {
    let overrides = build_overrides(runtime, &args.overrides)?;
    let element_info = if !args.no_move {
        if let Some(expr) = &args.expr {
            let (target, node) = resolve_point_and_node_from_expr(runtime, expr)?;
            let _ = runtime.pointer_move_to(target, overrides.clone()).map_err(map_pointer_error)?;
            Some(format_element_info(&node))
        } else {
            None
        }
    } else {
        None
    };
    runtime.pointer_scroll(args.delta, overrides).map_err(map_pointer_error)?;
    if let Some(info) = element_info { Ok(format!("Scrolled on element: {}", info)) } else { Ok(String::new()) }
}

fn run_drag(runtime: &Runtime, args: &PointerDragArgs) -> CliResult<String> {
    let overrides = build_overrides(runtime, &args.overrides)?;
    let mut start = args.from;
    let mut end = args.to;
    let mut from_info = None;
    let mut to_info = None;

    if let Some(expr) = &args.from_expr {
        let (point, node) = resolve_point_and_node_from_expr(runtime, expr)?;
        start = point;
        from_info = Some(format_element_info(&node));
    }
    if let Some(expr) = &args.to_expr {
        let (point, node) = resolve_point_and_node_from_expr(runtime, expr)?;
        end = point;
        to_info = Some(format_element_info(&node));
    }

    runtime.pointer_drag(start, end, Some(args.button), overrides).map_err(map_pointer_error)?;

    match (from_info, to_info) {
        (Some(from), Some(to)) => Ok(format!("Dragged from element: {} to element: {}", from, to)),
        (Some(from), None) => Ok(format!("Dragged from element: {} to point ({:.1}, {:.1})", from, end.x(), end.y())),
        (None, Some(to)) => Ok(format!("Dragged from point ({:.1}, {:.1}) to element: {}", start.x(), start.y(), to)),
        (None, None) => Ok(String::new()),
    }
}

fn run_position(runtime: &Runtime) -> CliResult<String> {
    let point = runtime.pointer_position().map_err(map_pointer_error)?;
    Ok(format!("Pointer currently at ({:.1}, {:.1}).", point.x(), point.y()))
}

fn build_overrides(runtime: &Runtime, args: &OverrideArgs) -> CliResult<Option<PointerOverrides>> {
    let mut overrides = PointerOverrides::new();

    if let Some(delay) = args.after_move_delay {
        overrides = overrides.after_move_delay(delay);
    }
    if let Some(delay) = args.after_input_delay {
        overrides = overrides.after_input_delay(delay);
    }
    if let Some(delay) = args.press_release_delay {
        overrides = overrides.press_release_delay(delay);
    }
    if let Some(delay) = args.after_click_delay {
        overrides = overrides.after_click_delay(delay);
    }
    if let Some(delay) = args.before_next_click_delay {
        overrides = overrides.before_next_click_delay(delay);
    }
    if let Some(delay) = args.multi_click_delay {
        overrides = overrides.multi_click_delay(delay);
    }
    if let Some(threshold) = args.ensure_move_threshold {
        overrides = overrides.ensure_move_threshold(threshold);
    }
    if let Some(timeout) = args.ensure_move_timeout {
        overrides = overrides.ensure_move_timeout(timeout);
    }
    if let Some(delay) = args.scroll_delay {
        overrides = overrides.scroll_delay(delay);
    }
    if let Some(step) = args.scroll_step {
        overrides = overrides.scroll_step(step);
    }
    if let Some(duration) = args.move_duration {
        overrides = overrides.move_duration(duration);
    }
    if let Some(duration) = args.move_time_per_pixel {
        overrides = overrides.move_time_per_pixel(duration);
    }
    if let Some(speed) = args.speed_factor {
        if speed <= 0.0 {
            anyhow::bail!("--speed-factor must be greater than 0");
        }
        overrides = overrides.speed_factor(speed);
    }
    if let Some(acceleration) = args.acceleration {
        let profile = match acceleration {
            AccelerationKind::Constant => PointerAccelerationProfile::Constant,
            AccelerationKind::EaseIn => PointerAccelerationProfile::EaseIn,
            AccelerationKind::EaseOut => PointerAccelerationProfile::EaseOut,
            AccelerationKind::SmoothStep => PointerAccelerationProfile::SmoothStep,
        };
        overrides = overrides.acceleration_profile(profile);
    }

    match args.origin {
        OriginKind::Desktop => {}
        OriginKind::Bounds => {
            let rect_s = args
                .bounds
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("--bounds must be provided when --origin bounds"))?;
            let rect = parse_rect(rect_s).map_err(|e| anyhow::anyhow!(e))?;
            overrides.origin = Some(PointOrigin::Bounds(rect));
        }
        OriginKind::Absolute => {
            let anchor_s = args
                .anchor
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("--anchor must be provided when --origin absolute"))?;
            let anchor = parse_point_arg(anchor_s).map_err(|e| anyhow::anyhow!(e))?;
            overrides.origin = Some(PointOrigin::Absolute(anchor));
        }
    }

    if let Some(motion) = args.motion {
        let mut profile = runtime.pointer_profile();
        profile.mode = match motion {
            MotionKind::Direct => PointerMotionMode::Direct,
            MotionKind::Linear => PointerMotionMode::Linear,
            MotionKind::Bezier => PointerMotionMode::Bezier,
            MotionKind::Overshoot => PointerMotionMode::Overshoot,
            MotionKind::Jitter => PointerMotionMode::Jitter,
        };
        overrides.profile = Some(profile);
    }

    if overrides == PointerOverrides::default() { Ok(None) } else { Ok(Some(overrides)) }
}

fn parse_rect(value: &str) -> Result<Rect, String> {
    let mut parts = value.split(',');
    let x = next_f64(&mut parts, "x", value)?;
    let y = next_f64(&mut parts, "y", value)?;
    let width = next_f64(&mut parts, "width", value)?;
    let height = next_f64(&mut parts, "height", value)?;
    if parts.next().is_some() {
        return Err(format!("expected rect 'x,y,width,height', got '{value}'"));
    }
    Ok(Rect::new(x, y, width, height))
}

fn next_f64<'a>(parts: &mut impl Iterator<Item = &'a str>, name: &str, original: &str) -> Result<f64, String> {
    parts
        .next()
        .ok_or_else(|| format!("expected rect 'x,y,width,height', got '{original}'"))?
        .trim()
        .parse::<f64>()
        .map_err(|err| format!("invalid {name} component '{original}': {err}"))
}

fn parse_millis(value: &str) -> Result<Duration, String> {
    let millis: u64 = value.parse().map_err(|err| format!("invalid duration '{value}': {err}"))?;
    Ok(Duration::from_millis(millis))
}

fn parse_point_arg(value: &str) -> Result<Point, String> {
    parse_point(value).map_err(|err| err.to_string())
}

fn parse_scroll_delta_arg(value: &str) -> Result<ScrollDelta, String> {
    parse_scroll_delta(value).map_err(|err| err.to_string())
}

fn parse_pointer_button_arg(value: &str) -> Result<PointerButton, String> {
    parse_pointer_button(value).map_err(|err| err.to_string())
}

fn parse_click_count(value: &str) -> Result<u32, String> {
    let count: u32 = value.parse().map_err(|err| format!("invalid click count '{value}': {err}"))?;
    if count < 2 {
        return Err("--count must be at least 2".to_owned());
    }
    Ok(count)
}

fn map_pointer_error(err: PointerError) -> anyhow::Error {
    anyhow::Error::new(err)
}

fn format_element_info(node: &Arc<dyn UiNode>) -> String {
    let role = node.role();
    let name = node.name();
    let runtime_id = node.runtime_id().as_str();

    // Try to get additional info from attributes if available
    let technology = node.attribute(Namespace::Control, common::TECHNOLOGY).and_then(|attr| match attr.value() {
        UiValue::String(s) => Some(s),
        _ => None,
    });

    let mut info = format!("{}[\"{}\"]", role, name);
    if let Some(tech) = technology {
        info.push_str(&format!(" ({})", tech));
    }
    info.push_str(&format!(", id: {}", runtime_id));
    info
}

fn resolve_point_and_node_from_expr(runtime: &Runtime, expr: &str) -> CliResult<(Point, Arc<dyn UiNode>)> {
    let item = runtime
        .evaluate_single(None, expr)
        .map_err(map_evaluate_error)?
        .ok_or_else(|| anyhow::anyhow!("expression `{expr}` did not match any items"))?;
    let node = match item {
        EvaluationItem::Node(node) => node,
        _ => anyhow::bail!("expression `{expr}` must select a node"),
    };
    let point = activation_point_or_bounds_center(&node)
        .ok_or_else(|| anyhow::anyhow!("expression `{expr}` did not yield a usable ActivationPoint/Bounds"))?;
    Ok((point, node))
}

fn activation_point_or_bounds_center(node: &Arc<dyn UiNode>) -> Option<Point> {
    if let Some(attr) = node.attribute(Namespace::Control, activation_target::ACTIVATION_POINT)
        && let UiValue::Point(p) = attr.value()
    {
        return Some(p);
    }
    if let Some(attr) = node.attribute(Namespace::Control, element::BOUNDS)
        && let UiValue::Rect(r) = attr.value()
    {
        return Some(Point::new(r.x() + r.width() / 2.0, r.y() + r.height() / 2.0));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::runtime;
    use platynui_platform_mock as _; // link platform-mock inventory
    use platynui_platform_mock::{PointerLogEntry, reset_pointer_state, take_pointer_log};
    use rstest::rstest;
    use serial_test::serial;

    #[rstest]
    #[serial]
    fn move_command_moves_pointer() {
        reset_pointer_state();
        let runtime = runtime();
        let args = PointerMoveArgs {
            expression: None,
            point: Some(Point::new(100.0, 150.0)),
            overrides: OverrideArgs::default(),
        };
        let output = super::run_move(&runtime, &args).expect("move");
        assert!(output.is_empty());
        let log = take_pointer_log();
        assert!(log.iter().any(|entry| matches!(entry, PointerLogEntry::Move(point) if *point == args.point.unwrap())));
    }

    #[rstest]
    #[serial]
    fn move_command_supports_negative_coordinates() {
        reset_pointer_state();
        let runtime = runtime();
        let args = PointerMoveArgs {
            expression: None,
            point: Some(Point::new(-2560.0, 0.0)),
            overrides: OverrideArgs::default(),
        };
        let output = super::run_move(&runtime, &args).expect("move negative");
        assert!(output.is_empty());
    }

    #[rstest]
    #[serial]
    fn click_command_clicks_button() {
        reset_pointer_state();
        let runtime = runtime();
        let args = PointerClickArgs {
            expression: None,
            point: Some(Point::new(50.0, 60.0)),
            button: PointerButton::Left,
            no_move: false,
            overrides: OverrideArgs::default(),
        };
        let output = super::run_click(&runtime, &args).expect("click");
        assert!(output.is_empty());
        let log = take_pointer_log();
        assert!(log.iter().any(|entry| matches!(entry, PointerLogEntry::Press(PointerButton::Left))));
        assert!(log.iter().any(|entry| matches!(entry, PointerLogEntry::Release(PointerButton::Left))));
    }

    #[rstest]
    #[serial]
    fn multi_click_command_clicks_multiple_times() {
        reset_pointer_state();
        let runtime = runtime();
        let args = PointerMultiClickArgs {
            expression: None,
            point: Some(Point::new(30.0, 40.0)),
            button: PointerButton::Left,
            count: 3,
            no_move: false,
            overrides: OverrideArgs::default(),
        };
        let output = super::run_multi_click(&runtime, &args).expect("multi-click");
        assert!(output.is_empty());
        let log = take_pointer_log();
        let presses = log.iter().filter(|entry| matches!(entry, PointerLogEntry::Press(PointerButton::Left))).count();
        assert_eq!(presses, 3);
    }

    #[rstest]
    #[serial]
    fn scroll_command_emits_steps() {
        reset_pointer_state();
        let runtime = runtime();
        let args = PointerScrollArgs {
            delta: ScrollDelta::new(0.0, -30.0),
            expr: None,
            no_move: false,
            overrides: OverrideArgs { scroll_step: Some(ScrollDelta::new(0.0, -10.0)), ..Default::default() },
        };
        let _ = super::run_scroll(&runtime, &args).expect("scroll");
        let log = take_pointer_log();
        let scrolls: Vec<_> = log
            .into_iter()
            .filter_map(|entry| match entry {
                PointerLogEntry::Scroll(delta) => Some(delta),
                _ => None,
            })
            .collect();
        assert_eq!(scrolls.len(), 3);
    }

    #[test]
    fn overrides_require_bounds() {
        let runtime = runtime();
        let overrides = OverrideArgs { origin: OriginKind::Bounds, ..Default::default() };
        let err = build_overrides(&runtime, &overrides).expect_err("missing bounds");
        assert!(err.to_string().contains("--bounds must be provided"));
    }

    #[test]
    fn overrides_require_anchor() {
        let runtime = runtime();
        let overrides = OverrideArgs { origin: OriginKind::Absolute, ..Default::default() };
        let err = build_overrides(&runtime, &overrides).expect_err("missing anchor");
        assert!(err.to_string().contains("--anchor must be provided"));
    }

    #[test]
    fn build_overrides_returns_none_if_empty() {
        let runtime = runtime();
        let overrides = OverrideArgs::default();
        let result = build_overrides(&runtime, &overrides).expect("overrides");
        assert!(result.is_none());
    }

    #[test]
    fn parse_click_count_requires_minimum() {
        let err = super::parse_click_count("1").expect_err("count below minimum");
        assert!(err.contains("at least 2"));
    }

    #[rstest]
    #[serial]
    fn position_command_reports_current_location() {
        reset_pointer_state();
        let runtime = runtime();
        let target = Point::new(42.0, 84.0);
        runtime.pointer_move_to(target, None).expect("move pointer");

        let output = super::run_position(&runtime).expect("position");
        assert!(output.contains("42.0"));
        assert!(output.contains("84.0"));
    }
}
