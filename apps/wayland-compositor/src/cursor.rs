//! Cursor theme loading and rendering.
//!
//! Loads xcursor theme images and provides [`MemoryRenderBuffer`]-backed cursor
//! elements for compositing into the render pipeline. This is essential for
//! screencopy: the host system cursor (managed by the winit backend) is not
//! captured, so we must render the cursor as a software element.
//!
//! The theme is loaded lazily from `$XCURSOR_THEME` / `$XCURSOR_SIZE` on first
//! use. Per-[`CursorIcon`] images are cached so subsequent frames only perform
//! a [`HashMap`] lookup.

use std::collections::HashMap;
use std::io::Read;
use std::time::Duration;

/// Default cursor size in pixels when `$XCURSOR_SIZE` is not set.
const DEFAULT_CURSOR_SIZE: u32 = 24;

use smithay::{
    backend::{allocator::Fourcc, renderer::element::memory::MemoryRenderBuffer},
    input::pointer::CursorIcon,
    utils::{Physical, Point, Transform},
};
use xcursor::{
    CursorTheme,
    parser::{Image, parse_xcursor},
};

/// Raw cursor image data for direct buffer copies (no GL pipeline needed).
///
/// Used by cursor session captures where the full render pipeline is
/// unnecessary — the xcursor pixel data is copied straight into the
/// client's SHM buffer.
pub struct CursorImageData {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Hotspot X coordinate in pixels.
    pub xhot: u32,
    /// Hotspot Y coordinate in pixels.
    pub yhot: u32,
    /// Pixel data in ARGB8888 format (on little-endian: B, G, R, A byte order).
    pub pixels: Vec<u8>,
}

/// Cached cursor theme state.
///
/// Stores the loaded xcursor theme and a per-icon cache of parsed images
/// plus their corresponding [`MemoryRenderBuffer`]s (keyed by image identity).
pub struct CursorThemeState {
    theme: CursorTheme,
    size: u32,
    /// Parsed xcursor images for each `CursorIcon` shape.
    icons: HashMap<CursorIcon, Vec<Image>>,
    /// GPU-uploadable buffers keyed by `(width, height, xhot, yhot, delay)` to
    /// avoid re-creating buffers for the same animation frame.
    buffer_cache: Vec<(ImageKey, MemoryRenderBuffer)>,
}

/// Identity key for a cursor image (avoids comparing pixel data).
#[derive(Clone, PartialEq, Eq, Hash)]
struct ImageKey {
    width: u32,
    height: u32,
    xhot: u32,
    yhot: u32,
    delay: u32,
    size: u32,
}

impl From<&Image> for ImageKey {
    fn from(img: &Image) -> Self {
        Self { width: img.width, height: img.height, xhot: img.xhot, yhot: img.yhot, delay: img.delay, size: img.size }
    }
}

impl CursorThemeState {
    /// Create a new cursor theme state, loading the theme from environment.
    ///
    /// Uses `$XCURSOR_THEME` (default: `"default"`) and `$XCURSOR_SIZE`
    /// (default: 24).
    pub fn new() -> Self {
        let name = std::env::var("XCURSOR_THEME").ok().unwrap_or_else(|| "default".into());
        let size = std::env::var("XCURSOR_SIZE").ok().and_then(|s| s.parse().ok()).unwrap_or(DEFAULT_CURSOR_SIZE);

        let theme = CursorTheme::load(&name);
        tracing::debug!(theme = %name, size, "loaded xcursor theme");

        Self { theme, size, icons: HashMap::new(), buffer_cache: Vec::new() }
    }

    /// Get the nominal cursor size configured for this theme.
    pub fn nominal_size(&self) -> u32 {
        self.size
    }

    /// Get raw cursor image data for an icon at the given scale and animation time.
    ///
    /// Bypasses the GL render pipeline — returns the xcursor pixel data
    /// directly, suitable for cursor session captures where GPU compositing
    /// is unnecessary.
    pub fn get_cursor_data(&mut self, icon: CursorIcon, scale: u32, time: Duration) -> Option<CursorImageData> {
        let image = self.get_image(icon, scale, time)?;
        Some(CursorImageData {
            width: image.width,
            height: image.height,
            xhot: image.xhot,
            yhot: image.yhot,
            pixels: image.pixels_rgba.clone(),
        })
    }

    /// Get the pixel dimensions of the default cursor icon at scale 1.
    ///
    /// Falls back to `(nominal_size, nominal_size)` if the icon cannot be loaded.
    pub fn default_cursor_dimensions(&mut self) -> (u32, u32) {
        if let Some(image) = self.get_image(CursorIcon::Default, 1, Duration::ZERO) {
            (image.width, image.height)
        } else {
            (self.size, self.size)
        }
    }

    /// Get the cursor image for a given icon, scale, and animation time.
    ///
    /// Returns the image frame and its hotspot in physical pixels, or `None`
    /// if the cursor could not be loaded.
    fn get_image(&mut self, icon: CursorIcon, scale: u32, time: Duration) -> Option<Image> {
        // Lazily load icons for this shape.
        let images = self.icons.entry(icon).or_insert_with(|| {
            load_icon(&self.theme, icon).or_else(|_| load_icon(&self.theme, CursorIcon::Default)).unwrap_or_else(
                |err| {
                    tracing::warn!(%err, ?icon, "failed to load xcursor, using fallback");
                    fallback_cursor()
                },
            )
        });

        if images.is_empty() {
            return None;
        }

        #[allow(clippy::cast_possible_truncation)]
        Some(select_frame(time.as_millis() as u32, self.size * scale, images))
    }

    /// Get a [`MemoryRenderBuffer`] for the given cursor icon at the specified
    /// scale and animation time.
    ///
    /// Returns `(buffer, hotspot)` where hotspot is in logical pixels.
    pub fn get_buffer(
        &mut self,
        icon: CursorIcon,
        scale: u32,
        time: Duration,
    ) -> Option<(MemoryRenderBuffer, Point<i32, Physical>)> {
        let image = self.get_image(icon, scale, time)?;
        let key = ImageKey::from(&image);

        // Check cache.
        let buffer = self
            .buffer_cache
            .iter()
            .find_map(|(k, buf)| if *k == key { Some(buf.clone()) } else { None })
            .unwrap_or_else(|| {
                // The xcursor file format stores pixels as 32-bit ARGB.
                // On little-endian systems the byte order is [B, G, R, A],
                // which matches `Fourcc::Argb8888` directly — no conversion needed.
                let buf = MemoryRenderBuffer::from_slice(
                    &image.pixels_rgba,
                    Fourcc::Argb8888,
                    (image.width.cast_signed(), image.height.cast_signed()),
                    1,
                    Transform::Normal,
                    None,
                );
                self.buffer_cache.push((key, buf.clone()));
                buf
            });

        let hotspot = Point::<i32, Physical>::from((image.xhot.cast_signed(), image.yhot.cast_signed()));

        Some((buffer, hotspot))
    }
}

/// Load xcursor images for a specific cursor icon from the theme.
fn load_icon(theme: &CursorTheme, icon: CursorIcon) -> Result<Vec<Image>, CursorError> {
    let icon_path = theme.load_icon(icon.name()).ok_or(CursorError::IconNotFound)?;
    let mut cursor_file = std::fs::File::open(icon_path)?;
    let mut cursor_data = Vec::new();
    cursor_file.read_to_end(&mut cursor_data)?;
    parse_xcursor(&cursor_data).ok_or(CursorError::Parse)
}

/// Fallback 1×1 transparent cursor when no theme is available.
fn fallback_cursor() -> Vec<Image> {
    vec![Image {
        size: DEFAULT_CURSOR_SIZE,
        width: 1,
        height: 1,
        xhot: 0,
        yhot: 0,
        delay: 1,
        pixels_rgba: vec![0, 0, 0, 0],
        pixels_argb: vec![],
    }]
}

/// Select the appropriate animation frame for the given time and nominal size.
fn select_frame(mut millis: u32, size: u32, images: &[Image]) -> Image {
    let nearest = nearest_images(size, images);
    let total: u32 = nearest.iter().map(|img| img.delay).sum();

    if total == 0 {
        return nearest.into_iter().next().cloned().unwrap_or_else(|| fallback_cursor().remove(0));
    }

    millis %= total;
    for img in &nearest {
        if millis < img.delay {
            return (*img).clone();
        }
        millis -= img.delay;
    }

    nearest.into_iter().next().cloned().unwrap_or_else(|| fallback_cursor().remove(0))
}

/// Filter images to those matching the nearest nominal size.
fn nearest_images(size: u32, images: &[Image]) -> Vec<&Image> {
    let Some(nearest) = images.iter().min_by_key(|img| (i64::from(size) - i64::from(img.size)).abs()) else {
        return Vec::new();
    };

    images.iter().filter(|img| img.width == nearest.width && img.height == nearest.height).collect()
}

/// Errors that can occur loading xcursor images.
#[derive(Debug)]
enum CursorError {
    IconNotFound,
    Io(std::io::Error),
    Parse,
}

impl std::fmt::Display for CursorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IconNotFound => write!(f, "cursor icon not found in theme"),
            Self::Io(e) => write!(f, "failed to read xcursor file: {e}"),
            Self::Parse => write!(f, "failed to parse xcursor file"),
        }
    }
}

impl std::error::Error for CursorError {}

impl From<std::io::Error> for CursorError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
