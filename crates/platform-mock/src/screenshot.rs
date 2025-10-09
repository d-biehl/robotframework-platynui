use crate::desktop::MOCK_PLATFORM;
use platynui_core::platform::{
    DesktopInfoProvider, PixelFormat, PlatformError, Screenshot, ScreenshotProvider, ScreenshotRequest,
};
use platynui_core::types::Rect;
use std::sync::Mutex;

pub static MOCK_SCREENSHOT: MockScreenshot = MockScreenshot::new();

// Mock screenshot provider does NOT auto-register - only available via explicit handles

#[derive(Debug)]
pub struct MockScreenshot {
    log: Mutex<Vec<ScreenshotLogEntry>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScreenshotLogEntry {
    pub request: ScreenshotRequest,
    pub width: u32,
    pub height: u32,
}

impl MockScreenshot {
    const fn new() -> Self {
        Self { log: Mutex::new(Vec::new()) }
    }

    fn record(&self, entry: ScreenshotLogEntry) {
        let mut log = self.log.lock().expect("screenshot log poisoned");
        log.push(entry);
    }
}

impl ScreenshotProvider for MockScreenshot {
    fn capture(&self, request: &ScreenshotRequest) -> Result<Screenshot, PlatformError> {
        let info = MOCK_PLATFORM.desktop_info()?;
        let region = match request.region {
            Some(rect) => rect,
            None => info.bounds,
        };
        let width = region.width().max(1.0).round() as u32;
        let height = region.height().max(1.0).round() as u32;
        let mut pixels = vec![0u8; (width * height * PixelFormat::Rgba8.bytes_per_pixel() as u32) as usize];
        let pitch = width as usize * 4;

        for chunk in pixels.chunks_exact_mut(4) {
            chunk[3] = 0xFF;
        }

        for monitor in info.monitors {
            if let Some(intersection) = region.intersection(&monitor.bounds) {
                fill_region(
                    &mut pixels,
                    width as usize,
                    height as usize,
                    pitch,
                    &region,
                    &intersection,
                    monitor_color(&monitor.id),
                );
                draw_label(&mut pixels, width as usize, height as usize, pitch, &region, &monitor.bounds);
            }
        }

        let screenshot = Screenshot::new(width, height, PixelFormat::Rgba8, pixels);
        self.record(ScreenshotLogEntry { request: request.clone(), width, height });
        Ok(screenshot)
    }
}

fn fill_region(
    buffer: &mut [u8],
    buffer_width: usize,
    buffer_height: usize,
    pitch: usize,
    region: &Rect,
    area: &Rect,
    color: [u8; 3],
) {
    let start_x = (area.x() - region.x()).round() as isize;
    let start_y = (area.y() - region.y()).round() as isize;
    let width = area.width().round().max(1.0) as usize;
    let height = area.height().round().max(1.0) as usize;

    for dy in 0..height {
        let row = start_y + dy as isize;
        if row < 0 || row >= buffer_height as isize {
            continue;
        }
        let row_offset = row as usize * pitch;
        for dx in 0..width {
            let col = start_x + dx as isize;
            if col < 0 || col >= buffer_width as isize {
                continue;
            }
            let idx = row_offset + col as usize * 4;
            buffer[idx] = color[0];
            buffer[idx + 1] = color[1];
            buffer[idx + 2] = color[2];
        }
    }
}

fn draw_label(
    buffer: &mut [u8],
    buffer_width: usize,
    buffer_height: usize,
    pitch: usize,
    region: &Rect,
    bounds: &Rect,
) {
    const TEXT: &str = "PLATYNUI";
    const GLYPH_W: usize = 5;
    const GLYPH_H: usize = 7;
    const GLYPH_SPACING: usize = 1;

    let glyph_count = TEXT.len();
    let total_cols = glyph_count * (GLYPH_W + GLYPH_SPACING) - GLYPH_SPACING;
    let max_scale_x = ((bounds.width() / total_cols as f64).floor() as usize).max(1);
    let max_scale_y = ((bounds.height() / GLYPH_H as f64).floor() as usize).max(1);
    let scale = max_scale_x.min(max_scale_y).max(1);

    let text_pixel_width = total_cols * scale;
    let text_pixel_height = GLYPH_H * scale;
    let start_x_abs = bounds.x() + (bounds.width() - text_pixel_width as f64) / 2.0;
    let start_y_abs = bounds.y() + (bounds.height() - text_pixel_height as f64) / 2.0;

    for (index, ch) in TEXT.chars().enumerate() {
        let glyph = glyph_for(ch);
        for (row, bits) in glyph.iter().enumerate() {
            let bits = *bits;
            for col in 0..GLYPH_W {
                if bits & (1 << (GLYPH_W - 1 - col)) == 0 {
                    continue;
                }
                for sy in 0..scale {
                    let abs_y = start_y_abs + ((row * scale + sy) as f64);
                    let buf_y = (abs_y - region.y()).round() as isize;
                    if buf_y < 0 || buf_y >= buffer_height as isize {
                        continue;
                    }
                    let row_offset = buf_y as usize * pitch;
                    for sx in 0..scale {
                        let abs_x = start_x_abs + (((index * (GLYPH_W + GLYPH_SPACING) + col) * scale + sx) as f64);
                        let buf_x = (abs_x - region.x()).round() as isize;
                        if buf_x < 0 || buf_x >= buffer_width as isize {
                            continue;
                        }
                        let idx = row_offset + buf_x as usize * 4;
                        buffer[idx] = 0xFF;
                        buffer[idx + 1] = 0xFF;
                        buffer[idx + 2] = 0xFF;
                        buffer[idx + 3] = 0xFF;
                    }
                }
            }
        }
    }
}

fn monitor_color(id: &str) -> [u8; 3] {
    match id {
        "mock-monitor-left" => [0xCC, 0x66, 0x33],
        "mock-monitor-center" => [0x33, 0x99, 0xFF],
        "mock-monitor-right" => [0x33, 0xCC, 0x66],
        _ => [0x88, 0x88, 0x88],
    }
}

fn glyph_for(character: char) -> [u8; 7] {
    match character {
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'I' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111],
        _ => [0; 7],
    }
}

pub fn take_screenshot_log() -> Vec<ScreenshotLogEntry> {
    let mut log = MOCK_SCREENSHOT.log.lock().expect("screenshot log poisoned");
    log.drain(..).collect()
}

pub fn reset_screenshot_state() {
    MOCK_SCREENSHOT.log.lock().expect("screenshot log poisoned").clear();
}

// Expose provider reference for explicit injection in tests/integration code.
// Test helpers for exposing internal state

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::platform::{ScreenshotRequest, screenshot_providers};
    use platynui_core::types::Rect;
    use rstest::rstest;

    #[rstest]
    fn screenshot_provider_not_auto_registered() {
        reset_screenshot_state();
        let providers: Vec<_> = screenshot_providers().collect();
        // Mock provider should NOT be in the registry
        // On most test systems this will be empty, or contain OS providers
        let mock_in_registry = providers.iter().any(|p| std::ptr::eq(*p, &MOCK_SCREENSHOT as &dyn ScreenshotProvider));
        assert!(!mock_in_registry, "Mock screenshot provider should not be auto-registered");

        // Use direct reference for testing the provider itself
        let provider = &MOCK_SCREENSHOT;

        let full = provider.capture(&ScreenshotRequest::entire_display()).unwrap();
        assert_eq!(full.width, 7920);
        assert_eq!(full.height, 3840);
        let log = take_screenshot_log();
        assert_eq!(log.len(), 1);
        assert!(log[0].request.region.is_none());
        assert_eq!(log[0].width, 7920);
        assert_eq!(log[0].height, 3840);

        let info = MOCK_PLATFORM.desktop_info().unwrap();
        let desktop_bounds = info.bounds;
        let monitors = info.monitors;
        let pixels = &full.pixels;
        let pitch = full.width as usize * 4;

        let center_bounds =
            monitors.iter().find(|m| m.id == "mock-monitor-center").map(|m| m.bounds).expect("center bounds");
        let expected_center = monitor_color("mock-monitor-center");
        let center_idx = ((center_bounds.y() + 100.0 - desktop_bounds.y()).round() as usize)
            .clamp(0, full.height as usize - 1)
            * pitch
            + ((center_bounds.x() + 100.0 - desktop_bounds.x()).round() as usize).clamp(0, full.width as usize - 1) * 4;
        assert_eq!(&pixels[center_idx..center_idx + 3], &expected_center);

        let portrait_bounds =
            monitors.iter().find(|m| m.id == "mock-monitor-left").map(|m| m.bounds).expect("portrait bounds");
        let expected_portrait = monitor_color("mock-monitor-left");
        let portrait_idx = ((portrait_bounds.y() + 100.0 - desktop_bounds.y()).round() as usize)
            .clamp(0, full.height as usize - 1)
            * pitch
            + ((portrait_bounds.x() + 100.0 - desktop_bounds.x()).round() as usize).clamp(0, full.width as usize - 1)
                * 4;
        assert_eq!(&pixels[portrait_idx..portrait_idx + 3], &expected_portrait);

        const GLYPH_W: usize = 5;
        const GLYPH_H: usize = 7;
        const GLYPH_SPACING: usize = 1;
        const TEXT: &str = "PLATYNUI";

        let total_cols = TEXT.len() * (GLYPH_W + GLYPH_SPACING) - GLYPH_SPACING;
        let max_scale_x = ((center_bounds.width() / total_cols as f64).floor() as usize).max(1);
        let max_scale_y = ((center_bounds.height() / GLYPH_H as f64).floor() as usize).max(1);
        let scale = max_scale_x.min(max_scale_y).max(1);
        let text_pixel_width = total_cols * scale;
        let text_pixel_height = GLYPH_H * scale;
        let start_x_abs = center_bounds.x() + (center_bounds.width() - text_pixel_width as f64) / 2.0;
        let start_y_abs = center_bounds.y() + (center_bounds.height() - text_pixel_height as f64) / 2.0;

        let sample_abs_x = start_x_abs + (scale as f64 * 0.5);
        let sample_abs_y = start_y_abs + (scale as f64 * 0.5);
        let sample_idx = ((sample_abs_y - desktop_bounds.y()).round() as usize).clamp(0, full.height as usize - 1)
            * pitch
            + ((sample_abs_x - desktop_bounds.x()).round() as usize).clamp(0, full.width as usize - 1) * 4;
        assert_eq!(&pixels[sample_idx..sample_idx + 3], &[0xFF, 0xFF, 0xFF]);

        let black_idx = ((-400.0 - desktop_bounds.y()).round() as usize).clamp(0, full.height as usize - 1) * pitch
            + ((100.0 - desktop_bounds.x()).round() as usize).clamp(0, full.width as usize - 1) * 4;
        assert_eq!(&pixels[black_idx..black_idx + 3], &[0, 0, 0]);
        assert_eq!(pixels[black_idx + 3], 0xFF);

        let request = ScreenshotRequest::with_region(Rect::new(-1800.0, -200.0, 300.0, 400.0));
        let screenshot = provider.capture(&request).unwrap();
        assert_eq!(screenshot.width, 300);
        assert_eq!(screenshot.height, 400);
        let log = take_screenshot_log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].request.region, Some(Rect::new(-1800.0, -200.0, 300.0, 400.0)));
        assert_eq!(log[0].width, 300);
        assert_eq!(log[0].height, 400);
    }

    #[rstest]
    fn screenshot_region_can_span_monitors() {
        reset_screenshot_state();
        // Use direct reference to mock provider
        let provider = &MOCK_SCREENSHOT;

        let region = Rect::new(3700.0, 600.0, 400.0, 200.0);
        let shot = provider.capture(&ScreenshotRequest::with_region(region)).expect("cross-monitor capture");

        assert_eq!(shot.width, 400);
        assert_eq!(shot.height, 200);
        let log = take_screenshot_log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].request.region, Some(region));
    }
}
