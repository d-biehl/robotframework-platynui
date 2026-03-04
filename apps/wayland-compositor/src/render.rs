//! Rendering pipeline — composites window surfaces into the output framebuffer.
//!
//! Builds a single flat render element list where each window's SSD decorations
//! are interleaved with the window's surface elements in correct z-order.
//! This ensures that a background window's title bar never paints over a
//! foreground window.

use smithay::backend::renderer::element::AsRenderElements;
use smithay::backend::renderer::element::memory::MemoryRenderBufferRenderElement;
use smithay::backend::renderer::element::render_elements;
use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::texture::TextureRenderElement;
use smithay::backend::renderer::gles::GlesTexture;
use smithay::backend::renderer::glow::GlowRenderer;
use smithay::desktop::layer_map_for_output;
use smithay::input::pointer::{CursorIcon, CursorImageStatus, CursorImageSurfaceData};
use smithay::output::Output;
use smithay::utils::{Logical, Physical, Point, Rectangle, Scale, Size};
use smithay::wayland::compositor;
use smithay::wayland::seat::WaylandFocus;
use smithay::wayland::shell::wlr_layer::Layer as WlrLayer;

use crate::{decorations, handlers::foreign_toplevel, state::State};

// Render element type — GPU-resident titlebars via TextureRenderElement.
//
// The `<=GlowRenderer` syntax binds the enum to GlowRenderer specifically,
// allowing TextureRenderElement<GlesTexture> because
// GlowRenderer::TextureId = GlesTexture.
render_elements! {
    pub CompositorRenderElement<=GlowRenderer>;
    Surface=WaylandSurfaceRenderElement<GlowRenderer>,
    Decoration=SolidColorRenderElement,
    Titlebar=TextureRenderElement<GlesTexture>,
    Cursor=MemoryRenderBufferRenderElement<GlowRenderer>,
}

/// Build the full render element list with correct z-ordering.
///
/// Iterates windows from front (top) to back (bottom).  For each window:
/// 1. Window surface elements (at lower indices → drawn later → in front)
/// 2. Decoration elements (at higher indices → drawn earlier → behind surfaces)
///
/// The resulting Vec has the front-most pixels at index 0. Smithay's
/// `OutputDamageTracker::render_output` iterates `.iter().rev()` so index 0
/// is drawn last (on top).
///
/// Titlebars are rendered as GPU-resident [`TextureRenderElement<GlesTexture>`]
/// via smithay's offscreen API — the texture stays on the GPU with no pixel
/// readback.
#[allow(clippy::module_name_repetitions, clippy::too_many_lines)]
pub fn collect_render_elements(
    renderer: &mut GlowRenderer,
    state: &mut State,
    output: &Output,
    draw_cursor: bool,
) -> Vec<CompositorRenderElement> {
    let mut elements: Vec<CompositorRenderElement> = Vec::new();

    // For single-output, use that output's scale.  For multi-output in a
    // single framebuffer (winit), we MUST use the max scale across all
    // outputs.  The framebuffer is sized at `logical_bbox * max_scale`,
    // so every element must be positioned and sized at the same scale for
    // the physical pixel positions to match the pointer mapping (which
    // divides physical cursor position by the same max_scale).
    // Using the primary output's scale when it differs from max_scale
    // creates a mismatch: elements render at the wrong position and
    // subsequent clicks don't find them.
    let output_scale =
        if state.outputs.len() > 1 { state.max_output_scale() } else { output.current_scale().fractional_scale() };
    let output_geo = state.space.output_geometry(output).unwrap_or_default();
    // Apply window_scale for winit preview: shrinks the rendering so that
    // large multi-output setups fit in a smaller host window.  For headless
    // and DRM backends window_scale is 1.0 (no effect).
    let render_scale = output_scale * state.window_scale;
    let scale = Scale::from(render_scale);

    // Determine which window is focused
    let focused_wl_surface = state.seat.get_keyboard().and_then(|kb| kb.current_focus()).and_then(|focus| {
        let cow = focus.wl_surface()?;
        Some(cow.into_owned())
    });

    // --- Overlay layer surfaces (topmost, above even context menu) ---
    // In multi-output mode, render layer surfaces from ALL outputs so that
    // panels/bars on non-primary outputs are visible in the combined
    // framebuffer.  For single-output this is equivalent to the old code.
    for o in &state.outputs {
        let o_geo = state.space.output_geometry(o).unwrap_or_default();
        render_layer_surfaces(&mut elements, renderer, o, o_geo, scale, WlrLayer::Overlay);
    }

    // --- Top layer surfaces (above windows, below overlay) ---
    for o in &state.outputs {
        let o_geo = state.space.output_geometry(o).unwrap_or_default();
        render_layer_surfaces(&mut elements, renderer, o, o_geo, scale, WlrLayer::Top);
    }

    // Iterate windows front-to-back (.rev() because elements() is back-to-front).
    for window in state.space.elements().rev() {
        let Some(window_loc) = state.space.element_location(window) else {
            continue;
        };

        let loc_in_output = window_loc - output_geo.loc;
        let physical_loc = loc_in_output.to_physical_precise_round(scale);

        // Window surface elements (in front of decorations)
        let surface_elements: Vec<WaylandSurfaceRenderElement<GlowRenderer>> =
            window.render_elements(renderer, physical_loc, scale, 1.0);
        elements.extend(surface_elements.into_iter().map(CompositorRenderElement::Surface));

        // SSD decoration elements (behind the window surfaces)
        if decorations::window_has_ssd(window) {
            let focused =
                focused_wl_surface.as_ref().is_some_and(|fs| window.wl_surface().is_some_and(|ws| *ws == *fs));

            let titlebar_loc: Point<i32, Logical> = (window_loc.x, window_loc.y - decorations::TITLEBAR_HEIGHT).into();
            let client_size = window.geometry().size;

            let title = window_title(window);

            let hovered_button =
                decorations::titlebar_button_hit_test(state.pointer_location, titlebar_loc, client_size).filter(
                    |click| {
                        matches!(
                            click,
                            decorations::DecorationClick::Close
                                | decorations::DecorationClick::Maximize
                                | decorations::DecorationClick::Minimize
                        )
                    },
                );

            let (deco_elements, titlebar_element) = decorations::render_decorations(
                renderer,
                &mut state.titlebar_renderer,
                titlebar_loc,
                client_size,
                render_scale,
                focused,
                &title,
                &state.config.theme,
                hovered_button,
            );
            elements.extend(deco_elements.into_iter().map(CompositorRenderElement::Decoration));
            if let Some(tb) = titlebar_element {
                elements.push(CompositorRenderElement::Titlebar(tb));
            }
        }
    }

    // --- Context menu overlay (on top of everything) ---
    if let Some(ref menu) = state.context_menu {
        // Dismiss the menu if the window is no longer in the space.
        if state.space.element_location(&menu.window).is_some() {
            let hovered_item = menu.item_at(state.pointer_location);
            let menu_loc = menu.position - output_geo.loc;
            #[allow(clippy::cast_possible_truncation)]
            let physical_loc = smithay::utils::Point::<f64, Physical>::from((
                f64::from(menu_loc.x) * render_scale,
                f64::from(menu_loc.y) * render_scale,
            ));
            if let Some(menu_element) = state.titlebar_renderer.render_context_menu_element(
                renderer,
                physical_loc,
                menu.is_maximized,
                hovered_item,
                render_scale,
                &state.config.theme,
            ) {
                // Insert at index 0 so the menu is drawn last (on top).
                elements.insert(0, CompositorRenderElement::Titlebar(menu_element));
            }
        } else {
            // Window gone — dismiss the menu.  We can't modify through the
            // shared ref, so flag it for cleanup.  A simple approach: just
            // don't render; the next click will clear it anyway since the
            // item_at/contains checks will fail.
        }
    }

    // --- Bottom layer surfaces (below windows, above background) ---
    for o in &state.outputs {
        let o_geo = state.space.output_geometry(o).unwrap_or_default();
        render_layer_surfaces(&mut elements, renderer, o, o_geo, scale, WlrLayer::Bottom);
    }

    // Draw subtle separator lines between outputs (behind all windows).
    if state.outputs.len() > 1 {
        elements
            .extend(render_output_separators(state, render_scale).into_iter().map(CompositorRenderElement::Decoration));
    }

    // --- Background layer surfaces (lowest, wallpapers) ---
    for o in &state.outputs {
        let o_geo = state.space.output_geometry(o).unwrap_or_default();
        render_layer_surfaces(&mut elements, renderer, o, o_geo, scale, WlrLayer::Background);
    }

    // --- Cursor rendering (software cursor, on top of everything) ---
    // Only composited when `draw_cursor` is true:
    //   - winit backend: controlled by --software-cursor CLI flag
    //   - screencopy: controlled by client's `paint_cursors` option
    //   - IPC screenshots: always true
    //
    // Compositor-driven cursor shapes (SSD resize borders, move grabs) take
    // priority over client-requested cursors.  When compositor_cursor_shape
    // is not Default, we render the corresponding xcursor theme icon instead
    // of whatever the client set via wl_pointer.set_cursor / wp-cursor-shape.
    if draw_cursor {
        // Check if the compositor wants to override the cursor (SSD interactions).
        let compositor_override = compositor_cursor_shape_to_icon(state.compositor_cursor_shape);

        if let Some(override_icon) = compositor_override {
            // Compositor-driven cursor (resize borders, move grab, etc.)
            render_xcursor_icon(&mut elements, renderer, state, override_icon, output_geo, scale);
        } else {
            // Normal client-driven cursor
            match &state.cursor_status {
                CursorImageStatus::Surface(surface) => {
                    let hotspot = compositor::with_states(surface, |states| {
                        states
                            .data_map
                            .get::<CursorImageSurfaceData>()
                            .map(|d| d.lock().expect("mutex poisoned").hotspot)
                            .unwrap_or_default()
                    });

                    #[allow(clippy::cast_possible_truncation)]
                    let cursor_pos: Point<i32, Logical> =
                        (state.pointer_location.x as i32 - hotspot.x, state.pointer_location.y as i32 - hotspot.y)
                            .into();
                    let loc_in_output = cursor_pos - output_geo.loc;
                    let physical_loc: Point<i32, Physical> = loc_in_output.to_physical_precise_round(scale);

                    let cursor_elements: Vec<WaylandSurfaceRenderElement<GlowRenderer>> =
                        smithay::backend::renderer::element::surface::render_elements_from_surface_tree(
                            renderer,
                            surface,
                            physical_loc,
                            scale,
                            1.0,
                            smithay::backend::renderer::element::Kind::Cursor,
                        );

                    for (i, elem) in cursor_elements.into_iter().enumerate() {
                        elements.insert(i, CompositorRenderElement::Surface(elem));
                    }
                }
                CursorImageStatus::Named(icon) => {
                    render_xcursor_icon(&mut elements, renderer, state, *icon, output_geo, scale);
                }
                CursorImageStatus::Hidden => {}
            }
        }
    }

    elements
}

/// Map a compositor-driven [`CursorShape`](decorations::CursorShape) to a
/// [`CursorIcon`] for xcursor theme rendering.
///
/// Returns `None` for `CursorShape::Default` (no compositor override — use
/// the client cursor) and `Some(icon)` for all SSD interaction shapes.
fn compositor_cursor_shape_to_icon(shape: decorations::CursorShape) -> Option<CursorIcon> {
    match shape {
        decorations::CursorShape::Default => None,
        decorations::CursorShape::Move => Some(CursorIcon::Move),
        decorations::CursorShape::ResizeN => Some(CursorIcon::NResize),
        decorations::CursorShape::ResizeS => Some(CursorIcon::SResize),
        decorations::CursorShape::ResizeW => Some(CursorIcon::WResize),
        decorations::CursorShape::ResizeE => Some(CursorIcon::EResize),
        decorations::CursorShape::ResizeNw => Some(CursorIcon::NwResize),
        decorations::CursorShape::ResizeNe => Some(CursorIcon::NeResize),
        decorations::CursorShape::ResizeSw => Some(CursorIcon::SwResize),
        decorations::CursorShape::ResizeSe => Some(CursorIcon::SeResize),
    }
}

/// Render a named cursor icon from the xcursor theme at the current pointer position.
fn render_xcursor_icon(
    elements: &mut Vec<CompositorRenderElement>,
    renderer: &mut GlowRenderer,
    state: &mut State,
    icon: CursorIcon,
    output_geo: Rectangle<i32, Logical>,
    scale: Scale<f64>,
) {
    let time = state.start_time.elapsed();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let cursor_scale = scale.x.max(scale.y).ceil() as u32;
    if let Some((buffer, hotspot)) = state.cursor_theme.get_buffer(icon, cursor_scale, time) {
        #[allow(clippy::cast_possible_truncation)]
        let cursor_pos: Point<i32, Logical> = (state.pointer_location.x as i32, state.pointer_location.y as i32).into();
        let loc_in_output = cursor_pos - output_geo.loc;
        let physical_loc: Point<i32, Physical> = loc_in_output.to_physical_precise_round(scale);
        let pos = Point::from((f64::from(physical_loc.x - hotspot.x), f64::from(physical_loc.y - hotspot.y)));

        if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            pos,
            &buffer,
            None,
            None,
            None,
            smithay::backend::renderer::element::Kind::Cursor,
        ) {
            elements.insert(0, CompositorRenderElement::Cursor(elem));
        }
    }
}

/// Render layer surfaces on a specific layer for the given output.
///
/// Iterates all mapped layer surfaces on `layer` from the output's `LayerMap`
/// and appends their render elements to `elements`.
fn render_layer_surfaces(
    elements: &mut Vec<CompositorRenderElement>,
    renderer: &mut GlowRenderer,
    output: &Output,
    output_geo: Rectangle<i32, Logical>,
    scale: Scale<f64>,
    layer: WlrLayer,
) {
    let map = layer_map_for_output(output);
    for layer_surface in map.layers_on(layer) {
        let geo = map.layer_geometry(layer_surface).unwrap_or_default();
        // Layer surface geometry is relative to the output; convert to global
        // physical coordinates for rendering.
        let loc = (geo.loc + output_geo.loc).to_physical_precise_round(scale);

        let surface_elements: Vec<WaylandSurfaceRenderElement<GlowRenderer>> =
            layer_surface.render_elements(renderer, loc, scale, 1.0);

        elements.extend(surface_elements.into_iter().map(CompositorRenderElement::Surface));
    }
}

/// Render a thin border frame around each output.
///
/// Draws a 1-pixel (logical) outline around every output so the user can
/// clearly see where each virtual monitor begins and ends.  This works
/// correctly for any layout (horizontal, vertical, L-shaped, etc.).
fn render_output_separators(state: &State, output_scale: f64) -> Vec<SolidColorRenderElement> {
    /// Border thickness in logical pixels.
    const BORDER: i32 = 1;
    /// Border color — subtle dark gray, slightly lighter than the background.
    const COLOR: [f32; 4] = [0.25, 0.25, 0.25, 1.0];

    let scale = Scale::from(output_scale);
    let mut elements = Vec::new();

    for output in &state.outputs {
        let Some(geo) = state.space.output_geometry(output) else {
            continue;
        };

        // Four edges: top, bottom, left, right.
        let edges: [(Point<i32, Logical>, Size<i32, Logical>); 4] = [
            // Top edge
            (geo.loc, (geo.size.w, BORDER).into()),
            // Bottom edge
            ((geo.loc.x, geo.loc.y + geo.size.h - BORDER).into(), (geo.size.w, BORDER).into()),
            // Left edge
            (geo.loc, (BORDER, geo.size.h).into()),
            // Right edge
            ((geo.loc.x + geo.size.w - BORDER, geo.loc.y).into(), (BORDER, geo.size.h).into()),
        ];

        for (loc, size) in edges {
            let phys_loc: Point<i32, Physical> = loc.to_physical_precise_round(scale);
            let phys_size: Size<i32, Physical> = size.to_physical_precise_round(scale);
            let phys_rect = Rectangle::new(phys_loc, phys_size);

            elements.push(SolidColorRenderElement::new(
                smithay::backend::renderer::element::Id::new(),
                phys_rect,
                0,
                COLOR,
                smithay::backend::renderer::element::Kind::Unspecified,
            ));
        }
    }

    elements
}

/// Extract the window title for SSD rendering.
///
/// Delegates to [`foreign_toplevel::window_title`] which handles
/// both Wayland (`XdgToplevelSurfaceData`) and X11 windows.
fn window_title(window: &smithay::desktop::Window) -> String {
    foreign_toplevel::window_title(window)
}

/// Render the compositor scene into an offscreen buffer and return raw RGBA pixels.
///
/// This is the shared implementation behind both the IPC screenshot command
/// (`control.rs`) and the screencopy/VNC pipeline (`screencopy.rs`).
///
/// The caller must supply a renderer that is **not** the main renderer
/// (to avoid borrow conflicts with `state`).  Typically this is
/// `state.screenshot_renderer`, temporarily taken out via `Option::take`.
///
/// # Arguments
///
/// * `renderer` — A `GlowRenderer` (standalone or shared-context).
/// * `state` — Compositor state (for `collect_render_elements`).
/// * `output` — The output to render.
/// * `size` — Physical pixel dimensions of the offscreen buffer.
/// * `scale` — Output scale for the damage tracker.
/// * `paint_cursors` — Whether to include cursor elements in the render.
///
/// # Errors
///
/// Returns a human-readable error string on GL failures.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn render_to_pixels(
    renderer: &mut GlowRenderer,
    state: &mut State,
    output: &Output,
    size: Size<i32, Physical>,
    scale: f64,
    paint_cursors: bool,
) -> Result<Vec<u8>, String> {
    use smithay::backend::allocator::Fourcc as DrmFourcc;
    use smithay::backend::renderer::damage::OutputDamageTracker;
    use smithay::backend::renderer::gles::GlesTexture;
    use smithay::backend::renderer::{Bind, ExportMem, Offscreen};
    use smithay::utils::Transform;

    let buffer_size: Size<i32, smithay::utils::Buffer> = (size.w, size.h).into();

    // Lazy-init the screenshot titlebar painter on the screenshot renderer's
    // GL context.  VAOs are per-context in OpenGL — they are NOT shared even
    // when contexts share textures via EGLContext::new_shared.
    if !state.screenshot_titlebar_renderer.is_glow_initialized() {
        state.screenshot_titlebar_renderer.init_glow(renderer);
    }

    // Create offscreen GL texture (Abgr8888 for GL compatibility).
    let mut texture: GlesTexture =
        renderer.create_buffer(DrmFourcc::Abgr8888, buffer_size).map_err(|e| format!("create_buffer: {e}"))?;

    let mut framebuffer = renderer.bind(&mut texture).map_err(|e| format!("bind: {e}"))?;

    // Swap in the screenshot titlebar renderer so that collect_render_elements
    // paints egui titlebars using the correct GL context.
    std::mem::swap(&mut state.titlebar_renderer, &mut state.screenshot_titlebar_renderer);

    let render_elements = collect_render_elements(renderer, state, output, paint_cursors);

    // Swap back so the main render loop keeps its own titlebar renderer.
    std::mem::swap(&mut state.titlebar_renderer, &mut state.screenshot_titlebar_renderer);

    // Render into the offscreen buffer.
    let mut damage_tracker = OutputDamageTracker::new(size, scale, Transform::Normal);
    damage_tracker
        .render_output(renderer, &mut framebuffer, 0, &render_elements, crate::state::BACKGROUND_COLOR)
        .map_err(|e| format!("render_output: {e}"))?;

    // Read back pixels.
    let region = Rectangle::from_size(buffer_size);
    let mapping = renderer
        .copy_framebuffer(&framebuffer, region, DrmFourcc::Abgr8888)
        .map_err(|e| format!("copy_framebuffer: {e}"))?;

    let pixel_data = renderer.map_texture(&mapping).map_err(|e| format!("map_texture: {e}"))?;

    Ok(pixel_data.to_vec())
}
