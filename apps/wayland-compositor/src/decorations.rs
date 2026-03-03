//! Server-Side Decorations (SSD) — title bars with close/maximize/minimize buttons.
//!
//! ## Rendering
//!
//! egui-rendered titlebar as a GPU-resident
//! [`TextureRenderBuffer`](smithay::backend::renderer::element::texture::TextureRenderBuffer)
//! via [`GlowRenderer`]. Borders are [`SolidColorRenderElement`].
//!
//! For environments without a hardware GPU, set `LIBGL_ALWAYS_SOFTWARE=1` to
//! use Mesa's software renderer (llvmpipe).
//!
//! ## Hit-testing (inspired by cosmic-comp)
//!
//! A unified [`Focus`] enum covers the header and all eight resize edges.
//! [`Focus::under()`] replaces separate hit-test functions.

use smithay::{
    backend::renderer::{
        element::Kind, element::solid::SolidColorRenderElement, element::texture::TextureRenderElement,
        gles::GlesTexture, glow::GlowRenderer,
    },
    desktop::Window,
    reexports::wayland_protocols::xdg::{
        decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode, shell::server::xdg_toplevel,
    },
    utils::{Logical, Physical, Point, Rectangle, Size},
};

/// Height of the title bar in logical pixels.
pub const TITLEBAR_HEIGHT: i32 = 30;

/// Width of the invisible resize border around windows in logical pixels.
pub const RESIZE_BORDER: i32 = 8;

/// Visible border width around the window frame in logical pixels.
const BORDER_WIDTH: i32 = 2;

/// Titlebar button width in logical pixels.
pub(crate) const TITLEBAR_BTN_WIDTH: f64 = 26.0;
/// Titlebar button height in logical pixels.
pub(crate) const TITLEBAR_BTN_HEIGHT: f64 = 18.0;
/// Gap between titlebar buttons in logical pixels.
pub(crate) const TITLEBAR_BTN_GAP: f64 = 2.0;
/// Right padding after the last titlebar button in logical pixels.
pub(crate) const TITLEBAR_BTN_RIGHT_PAD: f64 = 6.0;
/// Brightness increase (0–255) applied to button colors on hover.
pub(crate) const HOVER_LIGHTEN_AMOUNT: u8 = 35;

/// Check if a window should have server-side decorations.
///
/// Returns `false` for fullscreen windows — they must cover the entire output
/// without any compositor chrome.
#[must_use]
pub fn window_has_ssd(window: &Window) -> bool {
    // Fullscreen windows never get SSD — they occupy the full output.
    if window_is_fullscreen(window) {
        return false;
    }

    // Wayland path — xdg-decoration negotiation
    if let Some(toplevel) = window.toplevel() {
        let pending_mode = toplevel.with_pending_state(|s| s.decoration_mode);
        return match pending_mode {
            Some(mode) => mode == Mode::ServerSide,
            None => toplevel.current_state().decoration_mode == Some(Mode::ServerSide),
        };
    }

    // X11 path — XWayland windows
    if let Some(x11) = window.x11_surface() {
        // Override-redirect windows (tooltips, menus) never get decorations.
        // is_decorated() = true means the client uses Motif hints to opt out of
        // WM decorations (CSD-equivalent).
        return !x11.is_override_redirect() && !x11.is_decorated();
    }

    false
}

/// Check if a window is currently in fullscreen state.
#[must_use]
pub fn window_is_fullscreen(window: &Window) -> bool {
    if let Some(toplevel) = window.toplevel() {
        return toplevel.current_state().states.contains(xdg_toplevel::State::Fullscreen);
    }
    if let Some(x11) = window.x11_surface() {
        return x11.is_fullscreen();
    }
    false
}

/// A click target within the title bar header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecorationClick {
    /// Title bar area (drag to move).
    TitleBar,
    /// Close button.
    Close,
    /// Maximize/restore button.
    Maximize,
    /// Minimize button.
    Minimize,
}

// ---------------------------------------------------------------------------
// Titlebar context menu (right-click)
// ---------------------------------------------------------------------------

/// An open right-click context menu on a window's SSD titlebar.
pub struct TitlebarContextMenu {
    /// The window this menu belongs to.
    pub window: Window,
    /// Screen position of the menu's top-left corner (logical pixels).
    pub position: Point<i32, Logical>,
    /// Whether the window was maximized when the menu was opened (affects the
    /// "Maximize" / "Restore" label).
    pub is_maximized: bool,
}

impl TitlebarContextMenu {
    /// Menu width in logical pixels.
    pub const WIDTH: i32 = 180;
    /// Menu height in logical pixels (items + separator + vertical padding).
    #[allow(clippy::cast_possible_truncation)]
    pub const HEIGHT: i32 = (Self::ITEM_HEIGHT * 3.0 + Self::SEPARATOR_HEIGHT + Self::PADDING_Y * 2.0) as i32;

    const ITEM_HEIGHT: f64 = 26.0;
    const PADDING_Y: f64 = 4.0;
    const SEPARATOR_HEIGHT: f64 = 9.0;

    /// Determine which menu item index the pointer is over.
    ///
    /// - `0` → Minimize
    /// - `1` → Maximize / Restore
    /// - `2` → Close
    ///
    /// Returns `None` when the pointer is outside the menu or on a
    /// non-interactive area (padding, separator).
    #[must_use]
    pub fn item_at(&self, pointer: Point<f64, Logical>) -> Option<usize> {
        let rx = pointer.x - f64::from(self.position.x);
        let ry = pointer.y - f64::from(self.position.y);

        if rx < 0.0 || rx >= f64::from(Self::WIDTH) || ry < 0.0 || ry >= f64::from(Self::HEIGHT) {
            return None;
        }

        let y = ry - Self::PADDING_Y;
        if y < 0.0 {
            return None;
        }
        if y < Self::ITEM_HEIGHT {
            return Some(0);
        }
        let y = y - Self::ITEM_HEIGHT;
        if y < Self::ITEM_HEIGHT {
            return Some(1);
        }
        let y = y - Self::ITEM_HEIGHT;
        if y < Self::SEPARATOR_HEIGHT {
            return None;
        }
        let y = y - Self::SEPARATOR_HEIGHT;
        if y < Self::ITEM_HEIGHT {
            return Some(2);
        }
        None
    }

    /// Check if the pointer is anywhere within the menu bounds.
    #[must_use]
    pub fn contains(&self, pointer: Point<f64, Logical>) -> bool {
        let rx = pointer.x - f64::from(self.position.x);
        let ry = pointer.y - f64::from(self.position.y);
        rx >= 0.0 && rx < f64::from(Self::WIDTH) && ry >= 0.0 && ry < f64::from(Self::HEIGHT)
    }

    /// Map a menu item index to the corresponding decoration action.
    #[must_use]
    pub fn item_action(idx: usize) -> Option<DecorationClick> {
        match idx {
            0 => Some(DecorationClick::Minimize),
            1 => Some(DecorationClick::Maximize),
            2 => Some(DecorationClick::Close),
            _ => None,
        }
    }
}

/// An SSD focus zone — header (title bar) or one of eight resize edges.
///
/// `None` from [`Focus::under()`] means the point is in the client area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// Title bar / header area.
    Header,
    /// Resize: north edge.
    ResizeTop,
    /// Resize: south edge.
    ResizeBottom,
    /// Resize: west edge.
    ResizeLeft,
    /// Resize: east edge.
    ResizeRight,
    /// Resize: north-west corner.
    ResizeTopLeft,
    /// Resize: north-east corner.
    ResizeTopRight,
    /// Resize: south-west corner.
    ResizeBottomLeft,
    /// Resize: south-east corner.
    ResizeBottomRight,
}

impl Focus {
    /// Unified hit-test: determine which SSD zone contains the pointer.
    ///
    /// `window_loc` is the top-left of the **client** area.  Returns `None`
    /// for the client content area.
    #[must_use]
    pub fn under(
        point: Point<f64, Logical>,
        window_loc: Point<i32, Logical>,
        client_size: Size<i32, Logical>,
    ) -> Option<Self> {
        let rel_x = point.x - f64::from(window_loc.x);
        let rel_y = point.y - f64::from(window_loc.y) + f64::from(TITLEBAR_HEIGHT);

        let total_h = f64::from(TITLEBAR_HEIGHT + client_size.h);
        let w = f64::from(client_size.w);

        let near_top = rel_y < 0.0;
        let near_bottom = rel_y >= total_h;
        let near_left = rel_x < 0.0;
        let near_right = rel_x >= w;

        match (near_top, near_bottom, near_left, near_right) {
            (true, _, true, _) => return Some(Self::ResizeTopLeft),
            (true, _, _, true) => return Some(Self::ResizeTopRight),
            (_, true, true, _) => return Some(Self::ResizeBottomLeft),
            (_, true, _, true) => return Some(Self::ResizeBottomRight),
            (true, _, _, _) => return Some(Self::ResizeTop),
            (_, true, _, _) => return Some(Self::ResizeBottom),
            (_, _, true, _) => return Some(Self::ResizeLeft),
            (_, _, _, true) => return Some(Self::ResizeRight),
            _ => {}
        }

        // Inside the frame — header or client?
        if rel_y < f64::from(TITLEBAR_HEIGHT) {
            Some(Self::Header)
        } else {
            None // client area
        }
    }

    /// Whether this focus zone is a resize edge (not `Header`).
    #[must_use]
    pub fn is_resize(self) -> bool {
        !matches!(self, Self::Header)
    }

    /// Map to the appropriate cursor shape for this zone.
    #[must_use]
    pub fn cursor_shape(self) -> CursorShape {
        match self {
            Self::Header => CursorShape::Default,
            Self::ResizeTop => CursorShape::ResizeN,
            Self::ResizeBottom => CursorShape::ResizeS,
            Self::ResizeLeft => CursorShape::ResizeW,
            Self::ResizeRight => CursorShape::ResizeE,
            Self::ResizeTopLeft => CursorShape::ResizeNw,
            Self::ResizeTopRight => CursorShape::ResizeNe,
            Self::ResizeBottomLeft => CursorShape::ResizeSw,
            Self::ResizeBottomRight => CursorShape::ResizeSe,
        }
    }

    /// Convert to an XDG resize edge.
    ///
    /// Returns `None` for `Focus::Header` which is not a resize edge.
    #[must_use]
    pub fn to_xdg_resize_edge(
        self,
    ) -> Option<smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::ResizeEdge> {
        use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::ResizeEdge as E;
        match self {
            Self::ResizeTop => Some(E::Top),
            Self::ResizeBottom => Some(E::Bottom),
            Self::ResizeLeft => Some(E::Left),
            Self::ResizeRight => Some(E::Right),
            Self::ResizeTopLeft => Some(E::TopLeft),
            Self::ResizeTopRight => Some(E::TopRight),
            Self::ResizeBottomLeft => Some(E::BottomLeft),
            Self::ResizeBottomRight => Some(E::BottomRight),
            Self::Header => None,
        }
    }
}

/// Cursor shape to show based on what the pointer is hovering over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorShape {
    /// Normal pointer — over client content or empty space.
    #[default]
    Default,
    /// Grabbing/moving a window by its title bar.
    Move,
    /// Resize north edge.
    ResizeN,
    /// Resize south edge.
    ResizeS,
    /// Resize west edge.
    ResizeW,
    /// Resize east edge.
    ResizeE,
    /// Resize north-west corner.
    ResizeNw,
    /// Resize north-east corner.
    ResizeNe,
    /// Resize south-west corner.
    ResizeSw,
    /// Resize south-east corner.
    ResizeSe,
}

impl From<Focus> for CursorShape {
    fn from(focus: Focus) -> Self {
        focus.cursor_shape()
    }
}

/// Result of pointer hit-testing against all windows (front-to-back).
#[derive(Debug, Clone)]
pub enum PointerHitResult {
    /// Pointer hit a server-side decoration zone (titlebar or resize border).
    Ssd(Window, Focus),
    /// Pointer hit the client content area of a window.
    ClientArea(Window, Point<i32, Logical>),
    /// No window under the pointer.
    Empty,
}

/// Front-to-back pointer hit-test.  The first window whose bounds contain
/// the pointer owns the point (no click-through).
#[must_use]
pub fn pointer_hit_test(space: &smithay::desktop::Space<Window>, point: Point<f64, Logical>) -> PointerHitResult {
    for window in space.elements().rev() {
        let Some(window_loc) = space.element_location(window) else {
            continue;
        };

        if window_has_ssd(window) {
            let client_size = window.geometry().size;

            // Quick bounds check: is the point within the extended SSD area?
            let border = f64::from(RESIZE_BORDER);
            let top = f64::from(window_loc.y) - f64::from(TITLEBAR_HEIGHT) - border;
            let left = f64::from(window_loc.x) - border;
            let bottom = f64::from(window_loc.y + client_size.h) + border;
            let right = f64::from(window_loc.x + client_size.w) + border;

            if point.x < left || point.x > right || point.y < top || point.y > bottom {
                continue;
            }

            // This SSD window's full bounds contain the pointer — it owns the point.
            match Focus::under(point, window_loc, client_size) {
                Some(focus) => return PointerHitResult::Ssd(window.clone(), focus),
                None => return PointerHitResult::ClientArea(window.clone(), window_loc),
            }
        }

        // CSD window — full bounding box (including shadows/resize handles).
        let bbox = window.bbox();
        let bbox_rect = Rectangle::new((window_loc.x + bbox.loc.x, window_loc.y + bbox.loc.y).into(), bbox.size);
        if bbox_rect.to_f64().contains(point) {
            return PointerHitResult::ClientArea(window.clone(), window_loc);
        }
    }

    PointerHitResult::Empty
}

/// Hit-test within the title bar header to determine which button was clicked.
///
/// The button positions must match the egui layout in [`build_titlebar_ui`]:
/// right-to-left with 6 px right padding, 26×18 px buttons, 2 px gap between
/// them, centred vertically in the 30 px high title bar.
#[must_use]
pub fn titlebar_button_hit_test(
    point: Point<f64, Logical>,
    titlebar_loc: Point<i32, Logical>,
    client_size: Size<i32, Logical>,
) -> Option<DecorationClick> {
    let x = point.x - f64::from(titlebar_loc.x);
    let y = point.y - f64::from(titlebar_loc.y);

    if y < 0.0 || y >= f64::from(TITLEBAR_HEIGHT) {
        return None;
    }

    let w = f64::from(client_size.w);
    if x < 0.0 || x > w {
        return None;
    }

    // Button geometry — shared with ui.rs `build_titlebar_ui`.
    let btn_w = TITLEBAR_BTN_WIDTH;
    let btn_h = TITLEBAR_BTN_HEIGHT;
    let btn_gap = TITLEBAR_BTN_GAP;
    let right_pad = TITLEBAR_BTN_RIGHT_PAD;
    let btn_y = (f64::from(TITLEBAR_HEIGHT) - btn_h) / 2.0;

    // Vertical check — only match when inside the button row height.
    if y < btn_y || y >= btn_y + btn_h {
        // Still in the titlebar, just not on a button → drag area.
        return Some(DecorationClick::TitleBar);
    }

    // Right-to-left: Close, Maximize, Minimize.
    let close_right = w - right_pad;
    let close_left = close_right - btn_w;
    if x >= close_left && x < close_right {
        return Some(DecorationClick::Close);
    }

    let max_right = close_left - btn_gap;
    let max_left = max_right - btn_w;
    if x >= max_left && x < max_right {
        return Some(DecorationClick::Maximize);
    }

    let min_right = max_left - btn_gap;
    let min_left = min_right - btn_w;
    if x >= min_left && x < min_right {
        return Some(DecorationClick::Minimize);
    }

    Some(DecorationClick::TitleBar)
}

/// Generate render elements for SSD (GPU-resident egui titlebar via [`GlowRenderer`]).
#[allow(clippy::cast_possible_truncation, clippy::too_many_arguments)]
pub fn render_decorations(
    renderer: &mut GlowRenderer,
    titlebar_renderer: &mut crate::ui::TitlebarRenderer,
    window_loc: Point<i32, Logical>,
    window_geo: Size<i32, Logical>,
    scale: f64,
    focused: bool,
    title: &str,
    theme: &crate::config::ThemeConfig,
    hovered_button: Option<DecorationClick>,
) -> (Vec<SolidColorRenderElement>, Option<TextureRenderElement<GlesTexture>>) {
    let Some((physical_loc, physical_size)) = titlebar_physical_geometry(window_loc, window_geo, scale) else {
        return (Vec::new(), None);
    };

    let titlebar_element = titlebar_renderer.render_titlebar_element(
        renderer,
        (f64::from(physical_loc.x), f64::from(physical_loc.y)).into(),
        title,
        focused,
        window_geo.w.unsigned_abs(),
        TITLEBAR_HEIGHT.unsigned_abs(),
        scale,
        theme,
        hovered_button,
    );

    let borders = render_borders(physical_loc, physical_size, window_geo, scale, focused, theme);
    (borders, titlebar_element)
}

/// Compute the physical location and size of a titlebar, returning `None` if degenerate.
#[allow(clippy::cast_possible_truncation)]
fn titlebar_physical_geometry(
    window_loc: Point<i32, Logical>,
    window_geo: Size<i32, Logical>,
    scale: f64,
) -> Option<(Point<i32, Physical>, Size<i32, Physical>)> {
    let physical_loc = Point::<i32, Physical>::from((
        (f64::from(window_loc.x) * scale) as i32,
        (f64::from(window_loc.y) * scale) as i32,
    ));
    let physical_size = Size::<i32, Physical>::from((
        (f64::from(window_geo.w) * scale) as i32,
        (f64::from(TITLEBAR_HEIGHT) * scale) as i32,
    ));
    if physical_size.w <= 0 || physical_size.h <= 0 { None } else { Some((physical_loc, physical_size)) }
}

/// Render the four border edges around a decorated window frame.
#[allow(clippy::cast_possible_truncation)]
fn render_borders(
    physical_loc: Point<i32, Physical>,
    physical_size: Size<i32, Physical>,
    window_geo: Size<i32, Logical>,
    scale: f64,
    focused: bool,
    theme: &crate::config::ThemeConfig,
) -> Vec<SolidColorRenderElement> {
    let bw = (f64::from(BORDER_WIDTH) * scale) as i32;
    if bw <= 0 {
        return Vec::new();
    }

    let border_color = if focused { theme.active_border_rgba() } else { theme.inactive_border_rgba() };
    let frame_x = physical_loc.x;
    let frame_y = physical_loc.y;
    let frame_w = physical_size.w;
    let client_h = (f64::from(window_geo.h) * scale) as i32;
    let frame_h = physical_size.h + client_h;

    vec![
        solid_color((frame_x - bw, frame_y - bw).into(), (frame_w + 2 * bw, bw).into(), border_color),
        solid_color((frame_x - bw, frame_y + frame_h).into(), (frame_w + 2 * bw, bw).into(), border_color),
        solid_color((frame_x - bw, frame_y).into(), (bw, frame_h).into(), border_color),
        solid_color((frame_x + frame_w, frame_y).into(), (bw, frame_h).into(), border_color),
    ]
}

fn solid_color(loc: Point<i32, Physical>, size: Size<i32, Physical>, color: [f32; 4]) -> SolidColorRenderElement {
    SolidColorRenderElement::new(
        smithay::backend::renderer::element::Id::new(),
        Rectangle::new(loc, size),
        0,
        color,
        Kind::Unspecified,
    )
}
