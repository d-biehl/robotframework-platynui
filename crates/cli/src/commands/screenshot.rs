use crate::util::{CliResult, map_platform_error};
use clap::Args;
use platynui_core::platform::{PixelFormat, Screenshot, ScreenshotRequest};
use platynui_core::types::Rect;
use platynui_runtime::Runtime;
use png::{BitDepth, ColorType, Encoder};
use std::env;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Args, Debug, Clone)]
pub struct ScreenshotArgs {
    #[arg(
        value_name = "FILE",
        help = "Destination PNG file (optional). If omitted, a default name is generated in the current directory."
    )]
    pub output: Option<PathBuf>,
    #[arg(
        long = "rect",
        value_parser = parse_rect_arg,
        value_name = "X,Y,WIDTH,HEIGHT",
        allow_hyphen_values = true,
        help = "Capture only the specified rectangle in desktop coordinates."
    )]
    pub rect: Option<Rect>,
}

pub fn run(runtime: &Runtime, args: &ScreenshotArgs) -> CliResult<String> {
    let request = match args.rect {
        Some(rect) => ScreenshotRequest::with_region(rect),
        None => ScreenshotRequest::entire_display(),
    };

    let screenshot = runtime.screenshot(&request).map_err(map_platform_error)?;
    let output_path = match &args.output {
        Some(path) => ensure_unique_path(path),
        None => default_output_path(),
    };
    write_png(&output_path, &screenshot)?;

    Ok(format!("Saved screenshot to {} ({}×{} px).", output_path.display(), screenshot.width, screenshot.height))
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

fn parse_rect_arg(value: &str) -> Result<Rect, String> {
    let parts: Vec<_> = value.split(',').collect();
    if parts.len() != 4 {
        return Err(format!("expected four comma-separated values, got `{value}`"));
    }

    let mut numbers = Vec::with_capacity(4);
    for part in parts {
        let number: f64 = part.trim().parse().map_err(|_| format!("invalid number in rect `{value}`"))?;
        numbers.push(number);
    }

    Ok(Rect::new(numbers[0], numbers[1], numbers[2], numbers[3]))
}

fn default_output_path() -> PathBuf {
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    // Timestamp-based default name; collisions highly unlikely. If it exists, we still uniquify below.
    let ts_ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let base = cwd.join(format!("screenshot-{}.png", ts_ms));
    ensure_unique_path(&base)
}

fn ensure_unique_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("screenshot");
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("png");
    for idx in 1..=9999 {
        let candidate = parent.join(format!("{}-{:03}.{}", stem, idx, ext));
        if !candidate.exists() {
            return candidate;
        }
    }
    // Fallback: append random-ish suffix
    parent.join(format!("{}-{}.{}", stem, ts_fallback(), ext))
}

fn ts_fallback() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::runtime;
    use platynui_platform_mock as _; // link platform-mock inventory
    use platynui_platform_mock::{reset_screenshot_state, take_screenshot_log};
    use platynui_runtime::Runtime;
    use rstest::rstest;
    use serial_test::serial;
    use std::fs;
    use std::sync::{LazyLock, Mutex};
    use tempfile::tempdir;

    static TEST_GUARD: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    #[rstest]
    #[serial]
    fn screenshot_command_writes_png(runtime: Runtime) {
        let _lock = TEST_GUARD.lock().unwrap();
        reset_screenshot_state();
        // runtime is already mutable from fixture
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("capture.png");
        let args = ScreenshotArgs { output: Some(path.clone()), rect: Some(Rect::new(3700.0, 600.0, 400.0, 200.0)) };

        let output = run(&runtime, &args).expect("screenshot run");
        assert!(output.contains("Saved screenshot"));
        assert!(path.exists());

        let data = fs::read(&path).expect("read png");
        assert!(!data.is_empty());
        let log = take_screenshot_log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].width, 400);
        assert_eq!(log[0].height, 200);
    }

    #[rstest]
    #[serial]
    fn screenshot_without_rect_uses_full_desktop(runtime: Runtime) {
        let _lock = TEST_GUARD.lock().unwrap();
        reset_screenshot_state();
        // runtime is already mutable from fixture
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("full.png");
        let args = ScreenshotArgs { output: Some(path.clone()), rect: None };

        let output = run(&runtime, &args).expect("screenshot run");
        assert!(output.contains("Saved screenshot"));
        assert!(path.exists());

        let log = take_screenshot_log();
        assert_eq!(log.len(), 1);
        assert!(log[0].request.region.is_none());
        assert_eq!(log[0].width, 7920);
        assert_eq!(log[0].height, 3840);
    }

    #[test]
    fn parse_rect_arg_rejects_invalid_input() {
        let err = parse_rect_arg("a,b,c,d").expect_err("expected parse failure");
        assert!(err.contains("invalid number in rect"));
    }

    #[rstest]
    #[serial]
    fn screenshot_generates_default_name(runtime: Runtime) {
        let _lock = TEST_GUARD.lock().unwrap();
        reset_screenshot_state();
        // runtime is already mutable from fixture

        let dir = tempdir().expect("tempdir");
        let old_cwd = env::current_dir().expect("cwd");
        env::set_current_dir(dir.path()).expect("chdir temp");

        let args = ScreenshotArgs { output: None, rect: Some(Rect::new(0.0, 0.0, 10.0, 10.0)) };
        let output = run(&runtime, &args).expect("screenshot run");
        assert!(output.contains("Saved screenshot"));

        // Find a png file in temp dir
        let entries: Vec<_> = fs::read_dir(dir.path())
            .expect("list")
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map(|e| e == "png").unwrap_or(false))
            .collect();
        assert_eq!(entries.len(), 1);

        env::set_current_dir(old_cwd).expect("restore cwd");
    }
}
