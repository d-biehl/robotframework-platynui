//! Compositor-side highlight overlay state and rendering helpers.
//!
//! Renders a highlight frame around one or more rectangles, matching the visual
//! style of the X11 highlight provider: red 3px border with 1px gap, dashed
//! edges where the frame is clipped by output bounds.

use std::time::{Duration, Instant};

use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::utils::{Logical, Point, Rectangle, Scale, Size};

const HIGHLIGHT_BORDER_WIDTH: i32 = 3;
const HIGHLIGHT_GAP: i32 = 1;
const HIGHLIGHT_COLOR: [f32; 4] = [1.0, 0.0, 0.0, 1.0]; // Red, matching X11
const DASH_LEN: i32 = 8;
const GAP_LEN: i32 = 4;

#[derive(Debug, Default, Clone)]
pub struct HighlightOverlay {
    rects: Vec<Rectangle<i32, Logical>>,
    clear_at: Option<Instant>,
}

impl HighlightOverlay {
    pub fn show(&mut self, rects: Vec<Rectangle<i32, Logical>>, duration: Option<Duration>) {
        self.rects = rects.into_iter().filter(|rect| rect.size.w > 0 && rect.size.h > 0).collect();
        self.clear_at = duration.map(|timeout| Instant::now() + timeout);
    }

    pub fn clear(&mut self) {
        self.rects.clear();
        self.clear_at = None;
    }

    pub fn clear_if_expired(&mut self) {
        if self.clear_at.is_some_and(|deadline| Instant::now() >= deadline) {
            self.clear();
        }
    }

    pub fn rects(&self) -> &[Rectangle<i32, Logical>] {
        &self.rects
    }
}

pub fn logical_rectangle(x: f64, y: f64, width: f64, height: f64) -> Option<Rectangle<i32, Logical>> {
    if !x.is_finite() || !y.is_finite() || !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return None;
    }

    Some(Rectangle::new(
        Point::from((round_to_i32(x)?, round_to_i32(y)?)),
        Size::from((round_to_i32(width)?, round_to_i32(height)?)),
    ))
}

pub fn render_elements(
    rects: &[Rectangle<i32, Logical>],
    output_geo: Rectangle<i32, Logical>,
    scale: Scale<f64>,
) -> Vec<SolidColorRenderElement> {
    let mut elements = Vec::new();
    let t = HIGHLIGHT_BORDER_WIDTH;
    let g = HIGHLIGHT_GAP;

    for rect in rects {
        // Expand by border thickness + gap on each side
        let expanded = Rectangle::new(
            Point::from((rect.loc.x - (t + g), rect.loc.y - (t + g))),
            Size::from((rect.size.w + 2 * (t + g), rect.size.h + 2 * (t + g))),
        );

        // Clamp to output bounds
        let Some(clamped) = intersect_rect(expanded, output_geo) else {
            continue;
        };

        if clamped.size.w <= 0 || clamped.size.h <= 0 {
            continue;
        }

        let styles = edge_styles(expanded, clamped);

        let mut fb = FrameBuilder { elements: &mut elements, output_geo, scale };

        // Top edge
        fb.push_hline(clamped.loc.x, clamped.loc.x + clamped.size.w, clamped.loc.y, t, styles.top);
        // Bottom edge
        fb.push_hline(
            clamped.loc.x,
            clamped.loc.x + clamped.size.w,
            clamped.loc.y + clamped.size.h - t,
            t,
            styles.bottom,
        );
        // Left edge
        fb.push_vline(clamped.loc.y, clamped.loc.y + clamped.size.h, clamped.loc.x, t, styles.left);
        // Right edge
        fb.push_vline(
            clamped.loc.y,
            clamped.loc.y + clamped.size.h,
            clamped.loc.x + clamped.size.w - t,
            t,
            styles.right,
        );
    }

    elements
}

// --- Geometry helpers ---

fn intersect_rect(a: Rectangle<i32, Logical>, b: Rectangle<i32, Logical>) -> Option<Rectangle<i32, Logical>> {
    let left = a.loc.x.max(b.loc.x);
    let top = a.loc.y.max(b.loc.y);
    let right = (a.loc.x + a.size.w).min(b.loc.x + b.size.w);
    let bottom = (a.loc.y + a.size.h).min(b.loc.y + b.size.h);
    let w = right - left;
    let h = bottom - top;
    if w > 0 && h > 0 { Some(Rectangle::new(Point::from((left, top)), Size::from((w, h)))) } else { None }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LineStyle {
    Solid,
    Dashed,
}

struct EdgeStyles {
    top: LineStyle,
    right: LineStyle,
    bottom: LineStyle,
    left: LineStyle,
}

fn edge_styles(expanded: Rectangle<i32, Logical>, clamped: Rectangle<i32, Logical>) -> EdgeStyles {
    EdgeStyles {
        top: if clamped.loc.y > expanded.loc.y { LineStyle::Dashed } else { LineStyle::Solid },
        right: if (clamped.loc.x + clamped.size.w) < (expanded.loc.x + expanded.size.w) {
            LineStyle::Dashed
        } else {
            LineStyle::Solid
        },
        bottom: if (clamped.loc.y + clamped.size.h) < (expanded.loc.y + expanded.size.h) {
            LineStyle::Dashed
        } else {
            LineStyle::Solid
        },
        left: if clamped.loc.x > expanded.loc.x { LineStyle::Dashed } else { LineStyle::Solid },
    }
}

/// Bundled render context to reduce argument counts on line-drawing helpers.
struct FrameBuilder<'a> {
    elements: &'a mut Vec<SolidColorRenderElement>,
    output_geo: Rectangle<i32, Logical>,
    scale: Scale<f64>,
}

impl FrameBuilder<'_> {
    fn push_solid(&mut self, x: i32, y: i32, w: i32, h: i32) {
        if w <= 0 || h <= 0 {
            return;
        }
        let loc = Point::<i32, Logical>::from((x - self.output_geo.loc.x, y - self.output_geo.loc.y));
        let size = Size::<i32, Logical>::from((w, h));
        let phys_rect =
            Rectangle::new(loc.to_physical_precise_round(self.scale), size.to_physical_precise_round(self.scale));
        self.elements.push(SolidColorRenderElement::new(
            smithay::backend::renderer::element::Id::new(),
            phys_rect,
            0,
            HIGHLIGHT_COLOR,
            Kind::Unspecified,
        ));
    }

    fn push_hline(&mut self, x_start: i32, x_end: i32, y: i32, thickness: i32, style: LineStyle) {
        if style == LineStyle::Solid {
            self.push_solid(x_start, y, x_end - x_start, thickness);
            return;
        }
        let mut x = x_start;
        while x < x_end {
            let len = (x_end - x).min(DASH_LEN);
            self.push_solid(x, y, len, thickness);
            x += DASH_LEN + GAP_LEN;
        }
    }

    fn push_vline(&mut self, y_start: i32, y_end: i32, x: i32, thickness: i32, style: LineStyle) {
        if style == LineStyle::Solid {
            self.push_solid(x, y_start, thickness, y_end - y_start);
            return;
        }
        let mut y = y_start;
        while y < y_end {
            let len = (y_end - y).min(DASH_LEN);
            self.push_solid(x, y, thickness, len);
            y += DASH_LEN + GAP_LEN;
        }
    }
}

fn round_to_i32(value: f64) -> Option<i32> {
    let rounded = value.round();
    if rounded < f64::from(i32::MIN) || rounded > f64::from(i32::MAX) {
        return None;
    }

    #[allow(clippy::cast_possible_truncation)]
    Some(rounded as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Logical> {
        Rectangle::new(Point::from((x, y)), Size::from((w, h)))
    }

    #[test]
    fn expand_adds_gap_and_thickness() {
        // A 100x50 rect at (10, 20) with border=3 and gap=1 should expand by 4 on each side
        let input = rect(10, 20, 100, 50);
        let t = HIGHLIGHT_BORDER_WIDTH;
        let g = HIGHLIGHT_GAP;
        let expanded: Rectangle<i32, Logical> = Rectangle::new(
            Point::from((input.loc.x - (t + g), input.loc.y - (t + g))),
            Size::from((input.size.w + 2 * (t + g), input.size.h + 2 * (t + g))),
        );
        assert_eq!(expanded.loc.x, 6); // 10 - 4
        assert_eq!(expanded.loc.y, 16); // 20 - 4
        assert_eq!(expanded.size.w, 108); // 100 + 8
        assert_eq!(expanded.size.h, 58); // 50 + 8
    }

    #[test]
    fn intersect_rect_basic() {
        let a = rect(0, 0, 100, 100);
        let b = rect(50, 50, 100, 100);
        let i = intersect_rect(a, b).unwrap();
        assert_eq!(i, rect(50, 50, 50, 50));
    }

    #[test]
    fn intersect_rect_no_overlap() {
        let a = rect(0, 0, 10, 10);
        let b = rect(20, 20, 10, 10);
        assert!(intersect_rect(a, b).is_none());
    }

    #[test]
    fn edge_styles_all_solid_when_not_clipped() {
        let expanded = rect(0, 0, 100, 100);
        let clamped = rect(0, 0, 100, 100);
        let s = edge_styles(expanded, clamped);
        assert_eq!(s.top, LineStyle::Solid);
        assert_eq!(s.right, LineStyle::Solid);
        assert_eq!(s.bottom, LineStyle::Solid);
        assert_eq!(s.left, LineStyle::Solid);
    }

    #[test]
    fn edge_styles_dashed_when_clipped() {
        let expanded = rect(-10, -10, 200, 200);
        let clamped = rect(0, 0, 180, 180);
        let s = edge_styles(expanded, clamped);
        assert_eq!(s.top, LineStyle::Dashed); // top clipped
        assert_eq!(s.left, LineStyle::Dashed); // left clipped
        assert_eq!(s.right, LineStyle::Dashed); // right clipped
        assert_eq!(s.bottom, LineStyle::Dashed); // bottom clipped
    }

    #[test]
    fn render_elements_produces_output_for_visible_rect() {
        let rects = [rect(100, 100, 200, 150)];
        let output = rect(0, 0, 1920, 1080);
        let scale = Scale::from(1.0);
        let elems = render_elements(&rects, output, scale);
        // Should produce multiple elements (4 solid edges)
        assert!(!elems.is_empty());
    }

    #[test]
    fn render_elements_empty_for_offscreen_rect() {
        let rects = [rect(2000, 2000, 100, 100)];
        let output = rect(0, 0, 1920, 1080);
        let scale = Scale::from(1.0);
        let elems = render_elements(&rects, output, scale);
        assert!(elems.is_empty());
    }

    #[test]
    fn render_elements_dashes_at_screen_edge() {
        // Rect partially off the left/top edge of the output
        let rects = [rect(0, 0, 50, 50)];
        let output = rect(0, 0, 1920, 1080);
        let scale = Scale::from(1.0);
        let elems = render_elements(&rects, output, scale);
        // Expanded rect goes to (-4, -4) which is clipped — top and left should be dashed.
        // Dashed edges produce more elements than solid (multiple dash segments).
        // With 4 solid edges: 4 elements. With dashing: more.
        assert!(elems.len() > 4);
    }

    #[test]
    fn logical_rectangle_rejects_non_finite() {
        assert!(logical_rectangle(f64::NAN, 0.0, 100.0, 100.0).is_none());
        assert!(logical_rectangle(0.0, 0.0, f64::INFINITY, 100.0).is_none());
    }

    #[test]
    fn logical_rectangle_rejects_non_positive_size() {
        assert!(logical_rectangle(0.0, 0.0, 0.0, 100.0).is_none());
        assert!(logical_rectangle(0.0, 0.0, 100.0, -1.0).is_none());
    }

    #[test]
    fn logical_rectangle_valid() {
        let r = logical_rectangle(10.0, 20.0, 100.0, 50.0).unwrap();
        assert_eq!(r.loc.x, 10);
        assert_eq!(r.loc.y, 20);
        assert_eq!(r.size.w, 100);
        assert_eq!(r.size.h, 50);
    }
}
