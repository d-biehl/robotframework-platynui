use anyhow::{anyhow, bail};
use platynui_core::platform::{PlatformError, PointerButton, ScrollDelta};
use platynui_core::provider::ProviderError;
use platynui_core::types::Point;
use platynui_runtime::EvaluateError;

pub type CliResult<T> = anyhow::Result<T>;

pub fn map_provider_error(err: ProviderError) -> anyhow::Error {
    anyhow::Error::new(err)
}

pub fn map_evaluate_error(err: EvaluateError) -> anyhow::Error {
    anyhow::Error::new(err)
}

pub fn map_platform_error(err: PlatformError) -> anyhow::Error {
    anyhow::Error::new(err)
}

pub fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

pub fn parse_point(value: &str) -> CliResult<Point> {
    let mut parts = value.split(',');
    let x = parts
        .next()
        .ok_or_else(|| anyhow!("expected point 'x,y', got '{value}'"))?
        .trim()
        .parse::<f64>()
        .map_err(|err| anyhow!("invalid x component '{value}': {err}"))?;
    let y = parts
        .next()
        .ok_or_else(|| anyhow!("expected point 'x,y', got '{value}'"))?
        .trim()
        .parse::<f64>()
        .map_err(|err| anyhow!("invalid y component '{value}': {err}"))?;
    if parts.next().is_some() {
        bail!("expected point 'x,y', got '{value}'");
    }
    Ok(Point::new(x, y))
}

pub fn parse_scroll_delta(value: &str) -> CliResult<ScrollDelta> {
    let point = parse_point(value)?;
    Ok(ScrollDelta::new(point.x(), point.y()))
}

pub fn parse_pointer_button(value: &str) -> CliResult<PointerButton> {
    let normalized = value.trim().to_ascii_lowercase();
    let button = match normalized.as_str() {
        "left" | "primary" => PointerButton::Left,
        "right" | "secondary" => PointerButton::Right,
        "middle" | "wheel" => PointerButton::Middle,
        other => {
            let code = other.parse::<u16>().map_err(|_| anyhow!("unknown pointer button '{value}'"))?;
            PointerButton::Other(code)
        }
    };
    Ok(button)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("10,20", Point::new(10.0, 20.0))]
    #[case("-5.5, 42.1", Point::new(-5.5, 42.1))]
    fn parse_point_accepts_numbers(#[case] input: &str, #[case] expected: Point) {
        assert_eq!(parse_point(input).unwrap(), expected);
    }

    #[test]
    fn parse_point_rejects_invalid_input() {
        assert!(parse_point("abc").is_err());
    }

    #[test]
    fn parse_scroll_delta_converts_components() {
        let delta = parse_scroll_delta("0,-120").unwrap();
        assert_eq!(delta.horizontal, 0.0);
        assert_eq!(delta.vertical, -120.0);
    }

    #[rstest]
    #[case("left", PointerButton::Left)]
    #[case("Right", PointerButton::Right)]
    #[case("middle", PointerButton::Middle)]
    #[case("8", PointerButton::Other(8))]
    fn parse_pointer_button_accepts_aliases(#[case] input: &str, #[case] expected: PointerButton) {
        assert_eq!(parse_pointer_button(input).unwrap(), expected);
    }

    #[test]
    fn parse_pointer_button_rejects_unknown() {
        assert!(parse_pointer_button("foo").is_err());
        assert!(parse_pointer_button("back").is_err());
        assert!(parse_pointer_button("forward").is_err());
    }
}
