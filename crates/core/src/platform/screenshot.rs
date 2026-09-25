use crate::platform::PlatformError;
use crate::types::Rect;
use std::fmt;

/// Describes the pixel format of a captured screenshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    /// 8-bit per channel RGBA with straight alpha ordering (R, G, B, A).
    Rgba8,
    /// 8-bit per channel BGRA with straight alpha ordering (B, G, R, A).
    Bgra8,
}

impl PixelFormat {
    #[must_use]
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            PixelFormat::Rgba8 | PixelFormat::Bgra8 => 4,
        }
    }
}

impl fmt::Display for PixelFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PixelFormat::Rgba8 => f.write_str("Rgba8"),
            PixelFormat::Bgra8 => f.write_str("Bgra8"),
        }
    }
}

/// Request structure describing the portion of the desktop that should be captured.
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenshotRequest {
    pub region: Option<Rect>,
}

impl ScreenshotRequest {
    #[must_use]
    pub fn entire_display() -> Self {
        Self { region: None }
    }

    #[must_use]
    pub fn with_region(region: Rect) -> Self {
        Self { region: Some(region) }
    }
}

/// Screenshot image containing raw pixel data.
#[derive(Clone, Debug, PartialEq)]
pub struct Screenshot {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub pixels: Vec<u8>,
}

impl Screenshot {
    #[must_use]
    pub fn new(width: u32, height: u32, format: PixelFormat, pixels: Vec<u8>) -> Self {
        Self { width, height, format, pixels }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.pixels.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pixels.is_empty()
    }
}

/// Trait implemented by platform crates to provide screenshot functionality.
pub trait ScreenshotProvider: Send + Sync {
    /// Capture the region described by `request` (or the whole desktop).
    ///
    /// # Errors
    ///
    /// Returns a [`PlatformError`] when the platform cannot capture the
    /// requested region.
    fn capture(&self, request: &ScreenshotRequest) -> Result<Screenshot, PlatformError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_format_reports_bpp() {
        assert_eq!(PixelFormat::Rgba8.bytes_per_pixel(), 4);
        assert_eq!(PixelFormat::Bgra8.bytes_per_pixel(), 4);
    }

    // The region is stored verbatim, so exact equality is what the test means.
    #[allow(clippy::float_cmp)]
    #[test]
    fn screenshot_helpers() {
        let request = ScreenshotRequest::with_region(Rect::new(10.0, 20.0, 100.0, 50.0));
        assert_eq!(request.region.unwrap().x(), 10.0);
        let shot = Screenshot::new(10, 5, PixelFormat::Rgba8, vec![0; 200]);
        assert_eq!(shot.len(), 200);
        assert!(!shot.is_empty());
    }
}
