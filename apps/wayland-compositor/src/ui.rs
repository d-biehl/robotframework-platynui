//! egui-based titlebar rendering for server-side decorations.
//!
//! Inspired by [smithay-egui](https://github.com/Smithay/smithay-egui).
//!
//! egui meshes are rendered directly into a GPU-resident
//! [`TextureRenderBuffer`](smithay::backend::renderer::element::texture::TextureRenderBuffer)
//! via smithay's offscreen API — no pixel readback.
//!
//! For environments without a hardware GPU, set `LIBGL_ALWAYS_SOFTWARE=1` to
//! use Mesa's software renderer (llvmpipe).

use std::sync::Arc;

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::element::texture::{TextureRenderBuffer, TextureRenderElement};
use smithay::backend::renderer::gles::{GlesError, GlesTexture};
use smithay::backend::renderer::glow::GlowRenderer;
use smithay::backend::renderer::{Bind, Frame, Offscreen, Renderer};
use smithay::utils::{Buffer as BufferCoords, Physical, Point, Rectangle, Size, Transform};

use crate::config::ThemeConfig;
use crate::decorations::{
    DecorationClick, HOVER_LIGHTEN_AMOUNT, TITLEBAR_BTN_GAP, TITLEBAR_BTN_HEIGHT, TITLEBAR_BTN_RIGHT_PAD,
    TITLEBAR_BTN_WIDTH,
};

/// Manages an [`egui::Context`] and renders titlebars via glow GPU rendering.
///
/// A single instance is shared across all windows.  Call
/// [`init_glow`](Self::init_glow) once when a GL context becomes available;
/// without it, titlebars fall back to a solid-color background.
pub struct TitlebarRenderer {
    ctx: egui::Context,
    glow_state: Option<GlowState>,
    start_time: std::time::Instant,
}

struct GlowState {
    painter: egui_glow::Painter,
    render_buffer: Option<CachedRenderBuffer>,
    menu_render_buffer: Option<CachedRenderBuffer>,
}

struct CachedRenderBuffer {
    buffer: TextureRenderBuffer<GlesTexture>,
    width: i32,
    height: i32,
}

/// Selects which [`CachedRenderBuffer`] slot to use for rendering.
#[derive(Clone, Copy)]
enum BufferSlot {
    Titlebar,
    ContextMenu,
}

impl TitlebarRenderer {
    /// Create a new titlebar renderer with the given font family and size.
    pub fn new(font_family: &str, font_size: f32) -> Self {
        let ctx = egui::Context::default();

        let mut style = (*ctx.style()).clone();
        style.text_styles.insert(egui::TextStyle::Body, egui::FontId::new(font_size, egui::FontFamily::Proportional));
        style.text_styles.insert(egui::TextStyle::Button, egui::FontId::new(font_size, egui::FontFamily::Proportional));
        ctx.set_style(style);

        // Do NOT run a warm-up frame here — it consumes the font-atlas
        // TexturesDelta, causing a blank titlebar on the first real render.

        tracing::debug!(font_family, font_size, "titlebar renderer initialised");

        Self { ctx, glow_state: None, start_time: std::time::Instant::now() }
    }

    #[must_use]
    pub fn is_glow_initialized(&self) -> bool {
        self.glow_state.is_some()
    }

    /// Initialize the glow-based GPU painter.  Call once when a GL context
    /// becomes available (e.g. first frame of the winit backend).
    pub fn init_glow(&mut self, renderer: &mut GlowRenderer) {
        if self.glow_state.is_some() {
            return;
        }

        let gl = match renderer.with_context(Arc::clone) {
            Ok(gl) => gl,
            Err(err) => {
                tracing::warn!(%err, "failed to get GL context for egui painter");
                return;
            }
        };

        let painter = match egui_glow::Painter::new(gl, "", None, false) {
            Ok(p) => p,
            Err(err) => {
                tracing::warn!(err = %err, "failed to create egui_glow painter, using fallback");
                return;
            }
        };

        tracing::info!("glow-based titlebar painter initialised");

        self.glow_state = Some(GlowState { painter, render_buffer: None, menu_render_buffer: None });
    }

    /// Render a titlebar as a GPU-resident [`TextureRenderElement`].
    ///
    /// Returns `None` if the glow painter is not initialised or dimensions are zero.
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation, clippy::too_many_arguments)]
    pub fn render_titlebar_element(
        &mut self,
        renderer: &mut GlowRenderer,
        loc: Point<f64, Physical>,
        title: &str,
        focused: bool,
        width: u32,
        height: u32,
        scale: f64,
        theme: &ThemeConfig,
        hovered_button: Option<DecorationClick>,
    ) -> Option<TextureRenderElement<GlesTexture>> {
        let int_scale = scale.ceil() as i32;
        let buf_w = width.cast_signed().checked_mul(int_scale)?;
        let buf_h = height.cast_signed().checked_mul(int_scale)?;

        let (bg, text_color, close_fill, max_fill, min_fill) = compute_button_colors(theme, focused, hovered_button);
        let title_owned = title.to_string();

        self.render_egui_element(renderer, loc, buf_w, buf_h, int_scale, BufferSlot::Titlebar, |ctx| {
            build_titlebar_ui(ctx, &title_owned, bg, text_color, close_fill, max_fill, min_fill);
        })
    }

    /// Render a titlebar context menu as a GPU-resident [`TextureRenderElement`].
    ///
    /// Returns `None` if the glow painter is not initialised or dimensions are zero.
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    pub fn render_context_menu_element(
        &mut self,
        renderer: &mut GlowRenderer,
        loc: Point<f64, Physical>,
        is_maximized: bool,
        hovered_item: Option<usize>,
        scale: f64,
        theme: &ThemeConfig,
    ) -> Option<TextureRenderElement<GlesTexture>> {
        use crate::decorations::TitlebarContextMenu;

        let int_scale = scale.ceil() as i32;
        let buf_w = TitlebarContextMenu::WIDTH.checked_mul(int_scale)?;
        let buf_h = TitlebarContextMenu::HEIGHT.checked_mul(int_scale)?;

        let bg = theme_color(&theme.titlebar_background_focused);
        let text_color = theme_color(&theme.titlebar_text);
        let hover_bg = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30);
        let border_color = egui::Color32::from_gray(80);

        self.render_egui_element(renderer, loc, buf_w, buf_h, int_scale, BufferSlot::ContextMenu, |ctx| {
            build_context_menu_ui(ctx, is_maximized, hovered_item, bg, text_color, hover_bg, border_color);
        })
    }

    /// Shared GPU rendering pipeline for egui-based UI elements.
    ///
    /// Runs the egui layout closure, tessellates the output, and renders into
    /// a cached [`TextureRenderBuffer`].  The buffer is recreated only when
    /// the dimensions change.
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation, clippy::too_many_arguments)]
    fn render_egui_element(
        &mut self,
        renderer: &mut GlowRenderer,
        loc: Point<f64, Physical>,
        buf_w: i32,
        buf_h: i32,
        int_scale: i32,
        slot: BufferSlot,
        ui_fn: impl FnMut(&egui::Context),
    ) -> Option<TextureRenderElement<GlesTexture>> {
        if buf_w <= 0 || buf_h <= 0 {
            return None;
        }

        let glow = self.glow_state.as_mut()?;

        #[allow(clippy::cast_precision_loss)]
        let pixels_per_point = int_scale as f32;
        let max_tex_side = glow.painter.max_texture_side();
        let elapsed = self.start_time.elapsed().as_secs_f64();
        let raw_input = make_raw_input(buf_w, buf_h, pixels_per_point, elapsed, Some(max_tex_side));

        let full_output = self.ctx.run(raw_input, ui_fn);
        let clipped = self.ctx.tessellate(full_output.shapes, pixels_per_point);

        let cached_slot = match slot {
            BufferSlot::Titlebar => &mut glow.render_buffer,
            BufferSlot::ContextMenu => &mut glow.menu_render_buffer,
        };

        let needs_recreate = cached_slot.as_ref().is_none_or(|rb| rb.width != buf_w || rb.height != buf_h);

        if needs_recreate {
            let buf_size: Size<i32, BufferCoords> = (buf_w, buf_h).into();
            let render_texture = renderer
                .create_buffer(Fourcc::Abgr8888, buf_size)
                .map_err(|err| tracing::warn!(%err, "failed to create egui GPU texture"))
                .ok()?;
            *cached_slot = Some(CachedRenderBuffer {
                buffer: TextureRenderBuffer::from_texture(
                    renderer,
                    render_texture,
                    int_scale,
                    Transform::Flipped180,
                    None,
                ),
                width: buf_w,
                height: buf_h,
            });
        }

        let cached = cached_slot.as_mut()?;

        let draw_result = cached.buffer.render().draw(|tex| {
            let mut fb = renderer.bind(tex)?;
            let phys_size: Size<i32, Physical> = (buf_w, buf_h).into();
            {
                let mut frame = renderer.render(&mut fb, phys_size, Transform::Normal)?;
                frame.clear([0.0, 0.0, 0.0, 0.0].into(), &[Rectangle::new((0, 0).into(), phys_size)])?;
                glow.painter.paint_and_update_textures(
                    [buf_w as u32, buf_h as u32],
                    pixels_per_point,
                    &clipped,
                    &full_output.textures_delta,
                );
            }

            let damage: Rectangle<i32, BufferCoords> = Rectangle::new((0, 0).into(), (buf_w, buf_h).into());
            Result::<_, GlesError>::Ok(vec![damage])
        });

        if let Err(ref err) = draw_result {
            tracing::warn!(%err, "GPU egui render failed");
            return None;
        }

        Some(TextureRenderElement::from_texture_render_buffer(loc, &cached.buffer, None, None, None, Kind::Unspecified))
    }
}

impl Drop for GlowState {
    fn drop(&mut self) {
        self.painter.destroy();
    }
}

/// Build the egui titlebar UI layout (background, title text, window buttons).
#[allow(clippy::cast_possible_truncation)]
fn build_titlebar_ui(
    ctx: &egui::Context,
    title: &str,
    bg: egui::Color32,
    text_color: egui::Color32,
    close_fill: egui::Color32,
    max_fill: egui::Color32,
    min_fill: egui::Color32,
) {
    let frame = egui::Frame::NONE.fill(bg).inner_margin(egui::Margin::ZERO);
    egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
        ui.horizontal_centered(|ui| {
            // Left padding + title text
            ui.add_space(8.0);
            ui.label(egui::RichText::new(title).color(text_color).size(13.0));

            // Push buttons to the right
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(TITLEBAR_BTN_RIGHT_PAD as f32);
                ui.spacing_mut().item_spacing.x = TITLEBAR_BTN_GAP as f32;

                let btn_size = egui::vec2(TITLEBAR_BTN_WIDTH as f32, TITLEBAR_BTN_HEIGHT as f32);

                // Close button (rightmost)
                ui.add(
                    egui::Button::new(egui::RichText::new("\u{00D7}").color(egui::Color32::WHITE).size(12.0))
                        .fill(close_fill)
                        .corner_radius(3.0)
                        .min_size(btn_size),
                );

                // Maximize button
                ui.add(
                    egui::Button::new(egui::RichText::new("\u{25CB}").color(egui::Color32::WHITE).size(10.0))
                        .fill(max_fill)
                        .corner_radius(3.0)
                        .min_size(btn_size),
                );

                // Minimize button
                ui.add(
                    egui::Button::new(egui::RichText::new("\u{2013}").color(egui::Color32::WHITE).size(10.0))
                        .fill(min_fill)
                        .corner_radius(3.0)
                        .min_size(btn_size),
                );
            });
        });
    });
}

/// Compute button fill colors with optional hover highlighting.
fn compute_button_colors(
    theme: &ThemeConfig,
    focused: bool,
    hovered_button: Option<DecorationClick>,
) -> (egui::Color32, egui::Color32, egui::Color32, egui::Color32, egui::Color32) {
    let bg =
        if focused { theme_color(&theme.titlebar_background_focused) } else { theme_color(&theme.titlebar_background) };
    let text_color = theme_color(&theme.titlebar_text);
    let close_base = theme_color(&theme.button_close);
    let max_base = theme_color(&theme.button_maximize);
    let min_base = theme_color(&theme.button_minimize);

    let hover_tint = |base: egui::Color32| -> egui::Color32 {
        let [r, g, b, a] = base.to_array();
        egui::Color32::from_rgba_unmultiplied(
            r.saturating_add(HOVER_LIGHTEN_AMOUNT),
            g.saturating_add(HOVER_LIGHTEN_AMOUNT),
            b.saturating_add(HOVER_LIGHTEN_AMOUNT),
            a,
        )
    };

    let close_fill = if hovered_button == Some(DecorationClick::Close) { hover_tint(close_base) } else { close_base };
    let max_fill = if hovered_button == Some(DecorationClick::Maximize) { hover_tint(max_base) } else { max_base };
    let min_fill = if hovered_button == Some(DecorationClick::Minimize) { hover_tint(min_base) } else { min_base };

    (bg, text_color, close_fill, max_fill, min_fill)
}

/// Build [`egui::RawInput`] for a given viewport size.
#[allow(clippy::cast_precision_loss)]
fn make_raw_input(
    width: i32,
    height: i32,
    pixels_per_point: f32,
    time: f64,
    max_texture_side: Option<usize>,
) -> egui::RawInput {
    let mut viewports = egui::ViewportIdMap::default();
    viewports.insert(
        egui::ViewportId::ROOT,
        egui::ViewportInfo { native_pixels_per_point: Some(pixels_per_point), ..Default::default() },
    );
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(width as f32 / pixels_per_point, height as f32 / pixels_per_point),
        )),
        time: Some(time),
        max_texture_side,
        viewports,
        ..Default::default()
    }
}

/// Parse a CSS hex color or return a visible fallback.
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
fn theme_color(hex: &str) -> egui::Color32 {
    ThemeConfig::parse_color(hex).map_or(egui::Color32::from_rgb(255, 0, 255), |[r, g, b, a]| {
        egui::Color32::from_rgba_unmultiplied(
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8,
            (a * 255.0) as u8,
        )
    })
}

// ---------------------------------------------------------------------------
// Context menu UI
// ---------------------------------------------------------------------------

/// Build the egui layout for a titlebar context menu.
fn build_context_menu_ui(
    ctx: &egui::Context,
    is_maximized: bool,
    hovered_item: Option<usize>,
    bg: egui::Color32,
    text_color: egui::Color32,
    hover_bg: egui::Color32,
    border_color: egui::Color32,
) {
    let frame = egui::Frame::NONE
        .fill(bg)
        .inner_margin(egui::Margin::symmetric(0, 4))
        .stroke(egui::Stroke::new(1.0, border_color));

    egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;

        context_menu_item(ui, 0, "Minimize", hovered_item, text_color, hover_bg);
        context_menu_item(ui, 1, if is_maximized { "Restore" } else { "Maximize" }, hovered_item, text_color, hover_bg);

        // Separator between window actions and close
        ui.add_space(3.0);
        ui.separator();
        ui.add_space(2.0);

        context_menu_item(ui, 2, "Close", hovered_item, text_color, hover_bg);
    });
}

/// Render a single context menu item row.
fn context_menu_item(
    ui: &mut egui::Ui,
    idx: usize,
    label: &str,
    hovered_item: Option<usize>,
    text_color: egui::Color32,
    hover_bg: egui::Color32,
) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), egui::Sense::hover());
    if hovered_item == Some(idx) {
        ui.painter().rect_filled(rect, 2.0, hover_bg);
    }
    let text_pos = rect.left_center() + egui::vec2(12.0, 0.0);
    ui.painter().text(
        text_pos,
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::new(13.0, egui::FontFamily::Proportional),
        text_color,
    );
}
