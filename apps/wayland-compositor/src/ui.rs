//! CPU-rendered titlebar overlay for server-side decorations.
//!
//! Uses **tiny-skia** for drawing button symbols (×, □, —) and **swash**
//! for font shaping + glyph rasterization.  The result is a transparent
//! [`MemoryRenderBuffer`] that overlays the solid-color titlebar background
//! and button rectangles produced by
//! [`render_decorations`](crate::decorations::render_decorations).

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::utils::Transform;

use crate::config::ThemeConfig;
use crate::decorations::{
    DecorationClick, TITLEBAR_BTN_GAP, TITLEBAR_BTN_HEIGHT, TITLEBAR_BTN_RIGHT_PAD, TITLEBAR_BTN_WIDTH,
};

// ---------------------------------------------------------------------------
// Font cache
// ---------------------------------------------------------------------------

/// Cached font data for swash shaping and rasterization.
struct FontCache {
    data: Vec<u8>,
    offset: u32,
    size: f32,
}

impl FontCache {
    /// Build a font cache by looking up `family` in the system font database.
    ///
    /// Falls back to the first available font if the requested family is not found.
    fn new(family: &str, font_size: f32) -> Option<Self> {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();

        // Try to find the requested family, fall back to any available font.
        let face_id = db
            .faces()
            .find(|f| f.families.iter().any(|(name, _)| name.eq_ignore_ascii_case(family)))
            .or_else(|| db.faces().next())
            .map(|f| f.id)?;

        let mut raw_data: Option<Vec<u8>> = None;
        db.with_face_data(face_id, |data, _index| {
            raw_data = Some(data.to_vec());
        });
        let raw_data = raw_data?;

        // Find the font reference offset for swash.
        let font_ref = swash::FontRef::from_index(&raw_data, 0)?;
        let offset = font_ref.offset;

        Some(Self { data: raw_data, offset, size: font_size })
    }

    /// Obtain a [`swash::FontRef`] pointing into the cached data.
    fn font_ref(&self) -> Option<swash::FontRef<'_>> {
        swash::FontRef::from_index(&self.data, self.offset as usize)
    }
}

// ---------------------------------------------------------------------------
// TitlebarRenderer
// ---------------------------------------------------------------------------

/// CPU-side titlebar overlay renderer using tiny-skia + swash.
///
/// Produces a transparent [`MemoryRenderBuffer`] containing only the
/// button symbols (×, □, —), title text, and toplevel icon.  The opaque
/// background and button fill rectangles are rendered as
/// [`SolidColorRenderElement`](smithay::backend::renderer::element::solid::SolidColorRenderElement)
/// by [`render_decorations`](crate::decorations::render_decorations).
pub struct TitlebarRenderer {
    font_cache: Option<FontCache>,
}

impl TitlebarRenderer {
    /// Create a new titlebar renderer with the given font family and size.
    pub fn new(font_family: &str, font_size: f32) -> Self {
        let font_cache = FontCache::new(font_family, font_size);
        if font_cache.is_none() {
            tracing::warn!(font_family, "no suitable font found; titlebars will have no text");
        } else {
            tracing::debug!(font_family, font_size, "titlebar renderer initialised");
        }
        Self { font_cache }
    }

    /// Render a titlebar overlay into a [`MemoryRenderBuffer`].
    ///
    /// The buffer has a **transparent** background and contains only the
    /// button symbols, title text, and icon.  The caller layers this on top
    /// of solid-color background and button elements.
    ///
    /// Returns `None` if dimensions are zero.
    #[must_use]
    #[allow(
        clippy::cast_sign_loss,
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        clippy::too_many_arguments
    )]
    pub fn render_titlebar(
        &self,
        title: &str,
        icon: Option<&crate::handlers::toplevel_icon::ToplevelIconPixels>,
        focused: bool,
        width: u32,
        height: u32,
        scale: f64,
        theme: &ThemeConfig,
        hovered_button: Option<DecorationClick>,
    ) -> Option<MemoryRenderBuffer> {
        let int_scale = scale.ceil() as i32;
        let buf_w = (width as i32).checked_mul(int_scale)?;
        let buf_h = (height as i32).checked_mul(int_scale)?;

        if buf_w <= 0 || buf_h <= 0 {
            return None;
        }

        let pixels = self.paint_titlebar(buf_w, buf_h, int_scale, title, icon, focused, theme, hovered_button);

        Some(MemoryRenderBuffer::from_slice(
            &pixels,
            Fourcc::Abgr8888,
            (buf_w, buf_h),
            int_scale,
            Transform::Normal,
            None,
        ))
    }

    /// Render a context menu into a [`MemoryRenderBuffer`].
    ///
    /// Returns `None` if dimensions are zero.  The caller converts the buffer
    /// into a render element.
    #[must_use]
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    pub fn render_context_menu(
        &self,
        is_maximized: bool,
        hovered_item: Option<usize>,
        scale: f64,
        theme: &ThemeConfig,
    ) -> Option<MemoryRenderBuffer> {
        use crate::decorations::TitlebarContextMenu;

        let int_scale = scale.ceil() as i32;
        let buf_w = TitlebarContextMenu::WIDTH.checked_mul(int_scale)?;
        let buf_h = TitlebarContextMenu::HEIGHT.checked_mul(int_scale)?;

        if buf_w <= 0 || buf_h <= 0 {
            return None;
        }

        let pixels = self.paint_context_menu(buf_w, buf_h, int_scale, is_maximized, hovered_item, theme);

        Some(MemoryRenderBuffer::from_slice(
            &pixels,
            Fourcc::Abgr8888,
            (buf_w, buf_h),
            int_scale,
            Transform::Normal,
            None,
        ))
    }

    // -----------------------------------------------------------------------
    // Painting helpers
    // -----------------------------------------------------------------------

    /// Paint the titlebar overlay (symbols, text, icon) on a transparent pixmap.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::too_many_arguments
    )]
    fn paint_titlebar(
        &self,
        buf_w: i32,
        buf_h: i32,
        int_scale: i32,
        title: &str,
        icon: Option<&crate::handlers::toplevel_icon::ToplevelIconPixels>,
        _focused: bool,
        theme: &ThemeConfig,
        _hovered_button: Option<DecorationClick>,
    ) -> Vec<u8> {
        let w = buf_w as u32;
        let h = buf_h as u32;
        // Transparent pixmap — bg and button fills are SolidColorRenderElements.
        let mut pixmap =
            tiny_skia::Pixmap::new(w, h).unwrap_or_else(|| tiny_skia::Pixmap::new(1, 1).expect("1×1 pixmap"));

        let text_color = parse_theme_color(&theme.titlebar_text);

        let scale_f = int_scale as f32;

        // --- Button geometry (must match decorations::render_titlebar_solids) ---
        let btn_w = TITLEBAR_BTN_WIDTH as f32 * scale_f;
        let btn_h = TITLEBAR_BTN_HEIGHT as f32 * scale_f;
        let btn_gap = TITLEBAR_BTN_GAP as f32 * scale_f;
        let right_pad = TITLEBAR_BTN_RIGHT_PAD as f32 * scale_f;
        let btn_y = (buf_h as f32 - btn_h) / 2.0;

        let close_x = buf_w as f32 - right_pad - btn_w;
        let max_x = close_x - btn_gap - btn_w;
        let min_x = max_x - btn_gap - btn_w;

        // --- Button symbols (white, drawn on the transparent overlay) ---
        let sym_color = [255u8, 255, 255, 255];

        // Close: × (two crossing lines).
        let cx = close_x + btn_w / 2.0;
        let cy = btn_y + btn_h / 2.0;
        let s = 4.0 * scale_f;
        draw_line(&mut pixmap, cx - s, cy - s, cx + s, cy + s, 1.5 * scale_f, sym_color);
        draw_line(&mut pixmap, cx + s, cy - s, cx - s, cy + s, 1.5 * scale_f, sym_color);

        // Maximize: □ (rectangle outline).
        let mx = max_x + btn_w / 2.0;
        let my = btn_y + btn_h / 2.0;
        let rs = 3.5 * scale_f;
        draw_rect_outline(&mut pixmap, mx - rs, my - rs, rs * 2.0, rs * 2.0, 1.2 * scale_f, sym_color);

        // Minimize: — (horizontal line).
        let nx = min_x + btn_w / 2.0;
        let ny = btn_y + btn_h / 2.0;
        let ls = 4.0 * scale_f;
        draw_line(&mut pixmap, nx - ls, ny, nx + ls, ny, 1.5 * scale_f, sym_color);

        // --- Icon (if provided) ---
        let mut text_x = 8.0 * scale_f;
        if let Some(px) = icon {
            let icon_size = 16.0 * scale_f;
            let icon_y = (buf_h as f32 - icon_size) / 2.0;
            blit_icon(&mut pixmap, text_x as i32, icon_y as i32, icon_size as u32, px);
            text_x += icon_size + 4.0 * scale_f;
        }

        // --- Title text ---
        let text_y = buf_h as f32 / 2.0;
        self.draw_text(&mut pixmap, text_x, text_y, title, scale_f, text_color);

        // Return premultiplied RGBA directly — the GL compositor blends with
        // GL_ONE / GL_ONE_MINUS_SRC_ALPHA which expects premultiplied data.
        // Converting to straight alpha would cause washed-out antialiased edges.
        pixmap.data().to_vec()
    }

    /// Paint the context menu text overlay on a transparent pixmap.
    ///
    /// Background, border, hover highlight, and separator are rendered as
    /// [`SolidColorRenderElement`](smithay::backend::renderer::element::solid::SolidColorRenderElement)
    /// by [`render_context_menu_solids`](crate::decorations::render_context_menu_solids).
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss)]
    fn paint_context_menu(
        &self,
        buf_w: i32,
        buf_h: i32,
        int_scale: i32,
        is_maximized: bool,
        _hovered_item: Option<usize>,
        theme: &ThemeConfig,
    ) -> Vec<u8> {
        let w = buf_w as u32;
        let h = buf_h as u32;
        // Transparent pixmap — bg, border, hover, separator are SolidColorRenderElements.
        let mut pixmap =
            tiny_skia::Pixmap::new(w, h).unwrap_or_else(|| tiny_skia::Pixmap::new(1, 1).expect("1×1 pixmap"));

        let text_color = parse_theme_color(&theme.titlebar_text);
        let scale_f = int_scale as f32;

        let padding_y = 4.0 * scale_f;
        let item_height = 26.0 * scale_f;
        let separator_height = 9.0 * scale_f;

        let items = ["Minimize", if is_maximized { "Restore" } else { "Maximize" }, "Close"];

        let mut y = padding_y;
        for (idx, label) in items.iter().enumerate() {
            if idx == 2 {
                // Skip separator space (drawn as SolidColorRenderElement).
                y += separator_height;
            }

            let text_y = y + item_height / 2.0;
            self.draw_text(&mut pixmap, 12.0 * scale_f, text_y, label, scale_f, text_color);

            y += item_height;
        }

        // Premultiplied RGBA — matches GL compositor's blending mode.
        pixmap.data().to_vec()
    }

    /// Draw text at the given position using swash for shaping and rasterization.
    ///
    /// `y` is the vertical centre of the text line.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_lossless, clippy::manual_midpoint)]
    fn draw_text(&self, pixmap: &mut tiny_skia::Pixmap, x: f32, y: f32, text: &str, scale: f32, color: [u8; 4]) {
        let Some(ref cache) = self.font_cache else {
            return;
        };
        let Some(font_ref) = cache.font_ref() else {
            return;
        };

        let font_size = cache.size * scale;

        // Obtain font metrics for baseline positioning.
        let metrics = font_ref.metrics(&[]);
        let scale_factor = font_size / metrics.units_per_em as f32;
        let ascent = metrics.ascent * scale_factor;
        let descent = metrics.descent * scale_factor;
        let baseline_y = y + (ascent + descent) / 2.0 - descent;

        // Shape the text and rasterize each glyph.
        let mut shape_context = swash::shape::ShapeContext::new();
        let mut shaper = shape_context.builder(font_ref).size(font_size).build();
        shaper.add_str(text);

        let mut pen_x = x;
        let mut scale_context = swash::scale::ScaleContext::new();

        shaper.shape_with(|cluster| {
            for glyph in cluster.glyphs {
                let offset_x = pen_x + glyph.x;
                let offset_y = baseline_y - glyph.y;

                // Rasterize this glyph.
                let mut scaler = scale_context.builder(font_ref).size(font_size).build();

                if let Some(image) =
                    swash::scale::Render::new(&[swash::scale::Source::ColorOutline(0), swash::scale::Source::Outline])
                        .format(swash::zeno::Format::Alpha)
                        .render(&mut scaler, glyph.id)
                {
                    blit_glyph(
                        pixmap,
                        offset_x as i32 + image.placement.left,
                        offset_y as i32 - image.placement.top,
                        image.placement.width,
                        image.placement.height,
                        &image.data,
                        color,
                    );
                }

                pen_x += glyph.advance;
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Color helpers
// ---------------------------------------------------------------------------

/// Parse a CSS hex color into `[u8; 4]` RGBA.  Falls back to magenta on failure.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn parse_theme_color(hex: &str) -> [u8; 4] {
    ThemeConfig::parse_color(hex).map_or([255, 0, 255, 255], |[r, g, b, a]| {
        [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, (a * 255.0) as u8]
    })
}

/// Convert `[u8; 4]` RGBA to a tiny-skia [`Color`].
fn to_skia_color(c: [u8; 4]) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(c[0], c[1], c[2], c[3])
}

// ---------------------------------------------------------------------------
// Drawing primitives
// ---------------------------------------------------------------------------

/// Draw a line segment.
fn draw_line(pixmap: &mut tiny_skia::Pixmap, x1: f32, y1: f32, x2: f32, y2: f32, width: f32, color: [u8; 4]) {
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(x1, y1);
    pb.line_to(x2, y2);
    let Some(path) = pb.finish() else { return };
    let paint = tiny_skia::Paint {
        shader: tiny_skia::Shader::SolidColor(to_skia_color(color)),
        anti_alias: true,
        ..Default::default()
    };
    let stroke = tiny_skia::Stroke { width, ..Default::default() };
    pixmap.stroke_path(&path, &paint, &stroke, tiny_skia::Transform::identity(), None);
}

/// Draw a rectangle outline (stroke only).
fn draw_rect_outline(
    pixmap: &mut tiny_skia::Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    stroke_width: f32,
    color: [u8; 4],
) {
    let half = stroke_width / 2.0;
    let Some(rect) = tiny_skia::Rect::from_xywh(x + half, y + half, w - stroke_width, h - stroke_width) else {
        return;
    };
    let mut pb = tiny_skia::PathBuilder::new();
    pb.push_rect(rect);
    let Some(path) = pb.finish() else { return };
    let paint = tiny_skia::Paint {
        shader: tiny_skia::Shader::SolidColor(to_skia_color(color)),
        anti_alias: false,
        ..Default::default()
    };
    let stroke = tiny_skia::Stroke { width: stroke_width, ..Default::default() };
    pixmap.stroke_path(&path, &paint, &stroke, tiny_skia::Transform::identity(), None);
}

/// Blit a single alpha-coverage glyph into the pixmap with the given color.
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
fn blit_glyph(
    pixmap: &mut tiny_skia::Pixmap,
    x: i32,
    y: i32,
    glyph_w: u32,
    glyph_h: u32,
    alpha_data: &[u8],
    color: [u8; 4],
) {
    let pw = pixmap.width() as i32;
    let ph = pixmap.height() as i32;
    let pixels = pixmap.pixels_mut();

    for gy in 0..glyph_h as i32 {
        let py = y + gy;
        if py < 0 || py >= ph {
            continue;
        }
        for gx in 0..glyph_w as i32 {
            let px_x = x + gx;
            if px_x < 0 || px_x >= pw {
                continue;
            }
            let alpha = alpha_data[(gy as u32 * glyph_w + gx as u32) as usize];
            if alpha == 0 {
                continue;
            }

            let idx = (py * pw + px_x) as usize;
            let a = u16::from(alpha) * u16::from(color[3]) / 255;

            // Alpha-composite the glyph onto the existing pixel (premultiplied).
            let dst = pixels[idx];
            let dr = u16::from(dst.red());
            let dg = u16::from(dst.green());
            let db = u16::from(dst.blue());
            let da = u16::from(dst.alpha());

            let sr = u16::from(color[0]) * a / 255;
            let sg = u16::from(color[1]) * a / 255;
            let sb = u16::from(color[2]) * a / 255;
            let sa = a;

            let inv_sa = 255 - sa;
            let or = (sr + dr * inv_sa / 255).min(255) as u8;
            let og = (sg + dg * inv_sa / 255).min(255) as u8;
            let ob = (sb + db * inv_sa / 255).min(255) as u8;
            let oa = (sa + da * inv_sa / 255).min(255) as u8;

            pixels[idx] = tiny_skia::PremultipliedColorU8::from_rgba(or, og, ob, oa).expect("valid premul color");
        }
    }
}

/// Blit a toplevel icon (RGBA, possibly different size) scaled to `target_size` square.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
fn blit_icon(
    pixmap: &mut tiny_skia::Pixmap,
    x: i32,
    y: i32,
    target_size: u32,
    icon: &crate::handlers::toplevel_icon::ToplevelIconPixels,
) {
    if icon.rgba.len() < (icon.width * icon.height * 4) as usize {
        return;
    }

    // Create a tiny-skia pixmap from the icon data and draw it scaled.
    let Some(icon_pixmap) = tiny_skia::PixmapRef::from_bytes(&icon.rgba, icon.width, icon.height) else {
        return;
    };

    let sx = target_size as f32 / icon.width as f32;
    let sy = target_size as f32 / icon.height as f32;
    let transform = tiny_skia::Transform::from_translate(x as f32, y as f32).post_scale(sx, sy);

    let paint = tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bilinear, ..Default::default() };

    pixmap.draw_pixmap(0, 0, icon_pixmap, &paint, transform, None);
}
