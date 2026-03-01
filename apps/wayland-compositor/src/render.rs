//! Rendering pipeline — composites window surfaces into the output framebuffer.
//!
//! Builds a single flat render element list where each window's SSD decorations
//! are interleaved with the window's surface elements in correct z-order.
//! This ensures that a background window's title bar never paints over a
//! foreground window.

use smithay::backend::renderer::element::AsRenderElements;
use smithay::backend::renderer::element::render_elements;
use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::texture::TextureRenderElement;
use smithay::backend::renderer::gles::GlesTexture;
use smithay::backend::renderer::glow::GlowRenderer;
use smithay::input::pointer::{CursorImageStatus, CursorImageSurfaceData};
use smithay::output::Output;
use smithay::utils::{Logical, Physical, Point, Rectangle, Scale, Size};
use smithay::wayland::compositor;
use smithay::wayland::seat::WaylandFocus;
use smithay::wayland::shell::xdg::XdgToplevelSurfaceData;

use crate::{decorations, state::State};

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
#[allow(clippy::module_name_repetitions)]
pub fn collect_render_elements(
    renderer: &mut GlowRenderer,
    state: &mut State,
    output: &Output,
) -> Vec<CompositorRenderElement> {
    let mut elements: Vec<CompositorRenderElement> = Vec::new();

    let output_scale = output.current_scale().fractional_scale();
    let output_geo = state.space.output_geometry(output).unwrap_or_default();
    let scale = Scale::from(output_scale);

    // Determine which window is focused
    let focused_wl_surface = state.seat.get_keyboard().and_then(|kb| kb.current_focus()).and_then(|focus| {
        let cow = focus.wl_surface()?;
        Some(cow.into_owned())
    });

    // Iterate windows front-to-back (.rev() because elements() is back-to-front).
    for window in state.space.elements().rev() {
        let Some(window_loc) = state.space.element_location(window) else {
            continue;
        };

        // Convert window location to physical coordinates
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
                output_scale,
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
                f64::from(menu_loc.x) * output_scale,
                f64::from(menu_loc.y) * output_scale,
            ));
            if let Some(menu_element) = state.titlebar_renderer.render_context_menu_element(
                renderer,
                physical_loc,
                menu.is_maximized,
                hovered_item,
                output_scale,
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

    // Draw subtle separator lines between outputs (behind all windows).
    if state.outputs.len() > 1 {
        elements
            .extend(render_output_separators(state, output_scale).into_iter().map(CompositorRenderElement::Decoration));
    }

    // --- Client cursor surface (on top of everything, including context menu) ---
    if let CursorImageStatus::Surface(ref surface) = state.cursor_status {
        let hotspot = compositor::with_states(surface, |states| {
            states.data_map.get::<CursorImageSurfaceData>().map(|d| d.lock().unwrap().hotspot).unwrap_or_default()
        });

        #[allow(clippy::cast_possible_truncation)]
        let cursor_pos: Point<i32, Logical> =
            (state.pointer_location.x as i32 - hotspot.x, state.pointer_location.y as i32 - hotspot.y).into();
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

    elements
}

/// Render thin separator lines at output boundaries.
///
/// For horizontal layouts, draws a vertical line at each output's left edge
/// (except the first). For vertical layouts, draws a horizontal line at each
/// output's top edge (except the first). The line is a subtle dark gray so
/// that the user can see where one virtual monitor ends and the next begins.
fn render_output_separators(state: &State, output_scale: f64) -> Vec<SolidColorRenderElement> {
    /// Width of the separator line in logical pixels.
    const SEPARATOR_WIDTH: i32 = 2;
    /// Separator color — subtle dark gray, slightly lighter than the background.
    const SEPARATOR_COLOR: [f32; 4] = [0.25, 0.25, 0.25, 1.0];

    let combined = state.combined_output_geometry();
    let scale = Scale::from(output_scale);
    let mut separators = Vec::new();

    // Skip the first output; draw a line at the boundary between adjacent outputs.
    for output in state.outputs.iter().skip(1) {
        let Some(geo) = state.space.output_geometry(output) else {
            continue;
        };

        let (loc, size): (Point<i32, Logical>, Size<i32, Logical>) = if geo.loc.y == 0 {
            // Horizontal layout — vertical separator at the left edge of this output.
            ((geo.loc.x - SEPARATOR_WIDTH / 2, 0).into(), (SEPARATOR_WIDTH, combined.size.h).into())
        } else {
            // Vertical layout — horizontal separator at the top edge of this output.
            ((0, geo.loc.y - SEPARATOR_WIDTH / 2).into(), (combined.size.w, SEPARATOR_WIDTH).into())
        };

        let phys_loc: Point<i32, Physical> = loc.to_physical_precise_round(scale);
        let phys_size: Size<i32, Physical> = size.to_physical_precise_round(scale);
        let phys_rect = Rectangle::new(phys_loc, phys_size);

        separators.push(SolidColorRenderElement::new(
            smithay::backend::renderer::element::Id::new(),
            phys_rect,
            0,
            SEPARATOR_COLOR,
            smithay::backend::renderer::element::Kind::Unspecified,
        ));
    }

    separators
}

/// Extract the window title for SSD rendering.
///
/// Wayland windows store their title in [`XdgToplevelSurfaceData`].
/// X11 (`XWayland`) windows expose it directly via `X11Surface::title()`.
fn window_title(window: &smithay::desktop::Window) -> String {
    if let Some(toplevel) = window.toplevel()
        && let Some(title) = compositor::with_states(toplevel.wl_surface(), |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .and_then(|d| d.lock().ok())
                .and_then(|attrs| attrs.title.clone())
        })
    {
        return title;
    }

    if let Some(x11) = window.x11_surface() {
        return x11.title();
    }

    String::new()
}
