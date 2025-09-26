use clap::{Args, Subcommand, ValueEnum};
use platynui_core::platform::{
    PointOrigin, PointerButton, PointerMotionMode, PointerOverrides, ScrollDelta,
};
use platynui_core::types::{Point, Rect};
use platynui_runtime::{PointerError, Runtime};
use std::time::Duration;

use crate::util::{CliResult, parse_point, parse_pointer_button, parse_scroll_delta};

#[derive(Args)]
pub struct PointerArgs {
    #[command(subcommand)]
    pub command: PointerCommand,
}

#[derive(Subcommand)]
pub enum PointerCommand {
    Move(PointerMoveArgs),
    Click(PointerClickArgs),
    Press(PointerPressArgs),
    Release(PointerReleaseArgs),
    Scroll(PointerScrollArgs),
    Drag(PointerDragArgs),
    Position,
}

#[derive(Args)]
pub struct PointerMoveArgs {
    #[arg(value_parser = parse_point_arg)]
    pub point: Point,
    #[command(flatten)]
    overrides: OverrideArgs,
}

#[derive(Args)]
pub struct PointerClickArgs {
    #[arg(value_parser = parse_point_arg)]
    pub point: Point,
    #[arg(long = "button", default_value = "left", value_parser = parse_pointer_button_arg)]
    pub button: PointerButton,
    #[command(flatten)]
    overrides: OverrideArgs,
}

#[derive(Args)]
pub struct PointerPressArgs {
    #[arg(long = "point", value_parser = parse_point_arg)]
    pub point: Option<Point>,
    #[arg(long = "button", default_value = "left", value_parser = parse_pointer_button_arg)]
    pub button: PointerButton,
    #[command(flatten)]
    overrides: OverrideArgs,
}

#[derive(Args)]
pub struct PointerReleaseArgs {
    #[arg(long = "button", default_value = "left", value_parser = parse_pointer_button_arg)]
    pub button: PointerButton,
    #[command(flatten)]
    overrides: OverrideArgs,
}

#[derive(Args)]
pub struct PointerScrollArgs {
    #[arg(value_parser = parse_scroll_delta_arg)]
    pub delta: ScrollDelta,
    #[command(flatten)]
    overrides: OverrideArgs,
}

#[derive(Args)]
pub struct PointerDragArgs {
    #[arg(long = "from", value_parser = parse_point_arg)]
    pub from: Point,
    #[arg(long = "to", value_parser = parse_point_arg)]
    pub to: Point,
    #[arg(long = "button", default_value = "left", value_parser = parse_pointer_button_arg)]
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

#[derive(Args, Default)]
struct OverrideArgs {
    #[arg(long = "origin", value_enum, default_value_t = OriginKind::Desktop)]
    origin: OriginKind,
    #[arg(long = "bounds")]
    bounds: Option<String>,
    #[arg(long = "anchor")]
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
    #[arg(long = "scroll-step", value_parser = parse_scroll_delta_arg)]
    scroll_step: Option<ScrollDelta>,
}

pub fn run(runtime: &Runtime, args: &PointerArgs) -> CliResult<String> {
    match &args.command {
        PointerCommand::Move(move_args) => run_move(runtime, move_args),
        PointerCommand::Click(click_args) => run_click(runtime, click_args),
        PointerCommand::Press(press_args) => run_press(runtime, press_args),
        PointerCommand::Release(release_args) => run_release(runtime, release_args),
        PointerCommand::Scroll(scroll_args) => run_scroll(runtime, scroll_args),
        PointerCommand::Drag(drag_args) => run_drag(runtime, drag_args),
        PointerCommand::Position => run_position(runtime),
    }
}

fn run_move(runtime: &Runtime, args: &PointerMoveArgs) -> CliResult<String> {
    let overrides = build_overrides(runtime, &args.overrides)?;
    runtime.pointer_move_to(args.point, overrides).map_err(map_pointer_error)?;
    Ok(format!("Pointer moved to ({:.1}, {:.1}).", args.point.x(), args.point.y()))
}

fn run_click(runtime: &Runtime, args: &PointerClickArgs) -> CliResult<String> {
    let overrides = build_overrides(runtime, &args.overrides)?;
    runtime.pointer_click(args.point, Some(args.button), overrides).map_err(map_pointer_error)?;
    Ok(format!(
        "Clicked {button:?} at ({:.1}, {:.1}).",
        args.point.x(),
        args.point.y(),
        button = args.button
    ))
}

fn run_press(runtime: &Runtime, args: &PointerPressArgs) -> CliResult<String> {
    let overrides = build_overrides(runtime, &args.overrides)?;
    runtime.pointer_press(args.point, Some(args.button), overrides).map_err(map_pointer_error)?;
    if let Some(point) = args.point {
        Ok(format!(
            "Pressed {button:?} at ({:.1}, {:.1}).",
            point.x(),
            point.y(),
            button = args.button
        ))
    } else {
        Ok(format!("Pressed {button:?}.", button = args.button))
    }
}

fn run_release(runtime: &Runtime, args: &PointerReleaseArgs) -> CliResult<String> {
    let overrides = build_overrides(runtime, &args.overrides)?;
    runtime.pointer_release(Some(args.button), overrides).map_err(map_pointer_error)?;
    Ok(format!("Released {button:?}.", button = args.button))
}

fn run_scroll(runtime: &Runtime, args: &PointerScrollArgs) -> CliResult<String> {
    let overrides = build_overrides(runtime, &args.overrides)?;
    runtime.pointer_scroll(args.delta, overrides).map_err(map_pointer_error)?;
    Ok(format!("Scrolled by ({:.1}, {:.1}).", args.delta.horizontal, args.delta.vertical))
}

fn run_drag(runtime: &Runtime, args: &PointerDragArgs) -> CliResult<String> {
    let overrides = build_overrides(runtime, &args.overrides)?;
    runtime
        .pointer_drag(args.from, args.to, Some(args.button), overrides)
        .map_err(map_pointer_error)?;
    Ok(format!(
        "Dragged {button:?} from ({:.1}, {:.1}) to ({:.1}, {:.1}).",
        args.from.x(),
        args.from.y(),
        args.to.x(),
        args.to.y(),
        button = args.button
    ))
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

    match args.origin {
        OriginKind::Desktop => {}
        OriginKind::Bounds => {
            let rect = args
                .bounds
                .as_deref()
                .ok_or_else(|| "--bounds must be provided when --origin bounds".to_owned())
                .and_then(parse_rect)?;
            overrides.origin = Some(PointOrigin::Bounds(rect));
        }
        OriginKind::Absolute => {
            let anchor = args
                .anchor
                .as_deref()
                .ok_or_else(|| "--anchor must be provided when --origin absolute".to_owned())
                .and_then(parse_point_arg)?;
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

fn next_f64<'a>(
    parts: &mut impl Iterator<Item = &'a str>,
    name: &str,
    original: &str,
) -> Result<f64, String> {
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

fn map_pointer_error(err: PointerError) -> Box<dyn std::error::Error> {
    Box::new(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_platform_mock::{PointerLogEntry, reset_pointer_state, take_pointer_log};
    use platynui_runtime::Runtime;
    use rstest::rstest;
    use serial_test::serial;

    fn runtime() -> Runtime {
        Runtime::new().expect("runtime")
    }

    #[rstest]
    #[serial]
    fn move_command_moves_pointer() {
        reset_pointer_state();
        let runtime = runtime();
        let args =
            PointerMoveArgs { point: Point::new(100.0, 150.0), overrides: OverrideArgs::default() };
        let output = super::run_move(&runtime, &args).expect("move");
        assert!(output.contains("100.0"));
        let log = take_pointer_log();
        assert!(
            log.iter()
                .any(|entry| matches!(entry, PointerLogEntry::Move(point) if *point == args.point))
        );
    }

    #[rstest]
    #[serial]
    fn move_command_supports_negative_coordinates() {
        reset_pointer_state();
        let runtime = runtime();
        let args =
            PointerMoveArgs { point: Point::new(-2560.0, 0.0), overrides: OverrideArgs::default() };
        let output = super::run_move(&runtime, &args).expect("move negative");
        assert!(output.contains("-2560.0"));
    }

    #[rstest]
    #[serial]
    fn click_command_clicks_button() {
        reset_pointer_state();
        let runtime = runtime();
        let args = PointerClickArgs {
            point: Point::new(50.0, 60.0),
            button: PointerButton::Left,
            overrides: OverrideArgs::default(),
        };
        let output = super::run_click(&runtime, &args).expect("click");
        assert!(output.contains("Clicked"));
        let log = take_pointer_log();
        assert!(
            log.iter().any(|entry| matches!(entry, PointerLogEntry::Press(PointerButton::Left)))
        );
        assert!(
            log.iter().any(|entry| matches!(entry, PointerLogEntry::Release(PointerButton::Left)))
        );
    }

    #[rstest]
    #[serial]
    fn scroll_command_emits_steps() {
        reset_pointer_state();
        let runtime = runtime();
        let args = PointerScrollArgs {
            delta: ScrollDelta::new(0.0, -30.0),
            overrides: OverrideArgs {
                scroll_step: Some(ScrollDelta::new(0.0, -10.0)),
                ..Default::default()
            },
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
