use crate::util::{CliResult, map_platform_error};
use clap::Args;
use platynui_core::platform::{PixelFormat, Screenshot, ScreenshotRequest};
use platynui_core::types::Rect;
use platynui_runtime::Runtime;
use png::{BitDepth, ColorType, Encoder};
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

#[derive(Args, Debug, Clone)]
pub struct ScreenshotArgs {
    #[arg(
        long = "output",
        value_name = "FILE",
        required = true,
        help = "Path to the PNG file that will receive the captured screenshot."
    )]
    pub output: PathBuf,
    #[arg(
        long = "bbox",
        value_parser = parse_rect_argument,
        value_name = "X,Y,WIDTH,HEIGHT",
        help = "Restrict capture to the specified bounding box (desktop coordinates)."
    )]
    pub bbox: Option<Rect>,
}

pub fn run(runtime: &Runtime, args: &ScreenshotArgs) -> CliResult<String> {
    let request = match args.bbox {
        Some(rect) => ScreenshotRequest::with_region(rect),
        None => ScreenshotRequest::entire_display(),
    };

    let screenshot = runtime.screenshot(&request).map_err(map_platform_error)?;
    write_png(&args.output, &screenshot)?;

    Ok(format!(
        "Saved screenshot to {} ({}×{} px).",
        args.output.display(),
        screenshot.width,
        screenshot.height
    ))
}

fn write_png(path: &Path, screenshot: &Screenshot) -> CliResult<()> {
    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    let mut encoder = Encoder::new(writer, screenshot.width, screenshot.height);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    let rgba = ensure_rgba_bytes(screenshot);
    writer.write_image_data(&rgba)?;
    Ok(())
}

fn ensure_rgba_bytes(screenshot: &Screenshot) -> Vec<u8> {
    match screenshot.format {
        PixelFormat::Rgba8 => screenshot.pixels.clone(),
        PixelFormat::Bgra8 => {
            let mut converted = screenshot.pixels.clone();
            for chunk in converted.chunks_exact_mut(4) {
                chunk.swap(0, 2);
            }
            converted
        }
    }
}

fn parse_rect_argument(value: &str) -> Result<Rect, String> {
    let parts: Vec<_> = value.split(',').collect();
    if parts.len() != 4 {
        return Err(format!("expected four comma-separated values, got `{value}`"));
    }

    let mut numbers = Vec::with_capacity(4);
    for part in parts {
        let number: f64 =
            part.trim().parse().map_err(|_| format!("invalid number in bbox `{value}`"))?;
        numbers.push(number);
    }

    Ok(Rect::new(numbers[0], numbers[1], numbers[2], numbers[3]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::map_provider_error;
    use platynui_platform_mock::{reset_screenshot_state, take_screenshot_log};
    use platynui_runtime::Runtime;
    use rstest::rstest;
    use std::fs;
    use tempfile::tempdir;

    #[rstest]
    fn screenshot_command_writes_png() {
        reset_screenshot_state();
        let mut runtime = Runtime::new().map_err(map_provider_error).expect("runtime");
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("capture.png");
        let args = ScreenshotArgs {
            output: path.clone(),
            bbox: Some(Rect::new(3700.0, 600.0, 400.0, 200.0)),
        };

        let output = run(&runtime, &args).expect("screenshot run");
        assert!(output.contains("Saved screenshot"));
        assert!(path.exists());

        let data = fs::read(&path).expect("read png");
        assert!(!data.is_empty());
        let log = take_screenshot_log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].width, 400);
        assert_eq!(log[0].height, 200);

        runtime.shutdown();
    }

    #[rstest]
    fn screenshot_without_bbox_uses_full_desktop() {
        reset_screenshot_state();
        let mut runtime = Runtime::new().map_err(map_provider_error).expect("runtime");
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("full.png");
        let args = ScreenshotArgs { output: path.clone(), bbox: None };

        let output = run(&runtime, &args).expect("screenshot run");
        assert!(output.contains("Saved screenshot"));
        assert!(path.exists());

        let log = take_screenshot_log();
        assert_eq!(log.len(), 1);
        assert!(log[0].request.region.is_none());
        assert_eq!(log[0].width, 7920);
        assert_eq!(log[0].height, 3840);

        runtime.shutdown();
    }

    #[test]
    fn parse_rect_argument_rejects_invalid_input() {
        let err = parse_rect_argument("a,b,c,d").expect_err("expected parse failure");
        assert!(err.contains("invalid number"));
    }
}
