use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter, Result};

use super::Point;
use super::Size;

#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Rect {
    /// Create a new Rect from explicit components.
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        // Ensure width and height are non-negative by adjusting origin when necessary.
        let mut x = x;
        let mut y = y;
        let mut w = width;
        let mut h = height;
        if w < 0.0 {
            x += w;
            w = -w;
        }
        if h < 0.0 {
            y += h;
            h = -h;
        }
        Rect { x, y, width: w, height: h }
    }

    pub fn left(&self) -> f64 {
        self.x
    }

    pub fn top(&self) -> f64 {
        self.y
    }

    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    pub fn center(&self) -> Point {
        Point::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    pub fn contains(&self, p: Point) -> bool {
        // Inclusive left/top, exclusive right/bottom: [x, x+width) × [y, y+height)
        p.x() >= self.x && p.x() < self.x + self.width && p.y() >= self.y && p.y() < self.y + self.height
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }

    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = (self.x + self.width).min(other.x + other.width);
        let y1 = (self.y + self.height).min(other.y + other.height);
        let w = x1 - x0;
        let h = y1 - y0;
        if w > 0.0 && h > 0.0 { Some(Rect::new(x0, y0, w, h)) } else { None }
    }

    pub fn union(&self, other: &Rect) -> Rect {
        let x0 = self.x.min(other.x);
        let y0 = self.y.min(other.y);
        let x1 = (self.x + self.width).max(other.x + other.width);
        let y1 = (self.y + self.height).max(other.y + other.height);
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }

    pub fn translate(&self, dx: f64, dy: f64) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.width, self.height)
    }

    pub fn inflate(&self, dw: f64, dh: f64) -> Rect {
        Rect::new(self.x - dw, self.y - dh, self.width + 2.0 * dw, self.height + 2.0 * dh)
    }

    pub fn deflate(&self, dw: f64, dh: f64) -> Rect {
        self.inflate(-dw, -dh)
    }

    pub fn is_empty(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }
    pub fn x(&self) -> f64 {
        self.x
    }
    pub fn y(&self) -> f64 {
        self.y
    }
    pub fn width(&self) -> f64 {
        self.width
    }
    pub fn height(&self) -> f64 {
        self.height
    }
    pub fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    pub fn position(&self) -> Point {
        Point::new(self.x, self.y)
    }
}

impl From<(f64, f64, f64, f64)> for Rect {
    fn from(t: (f64, f64, f64, f64)) -> Self {
        Rect::new(t.0, t.1, t.2, t.3)
    }
}

impl Display for Rect {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        // plain tuple-like representation consistent with Point
        write!(f, "({}, {}, {}, {})", self.x, self.y, self.width, self.height)
    }
}

impl std::ops::Add<Point> for Rect {
    type Output = Rect;
    fn add(self, rhs: Point) -> Rect {
        Rect::new(self.x + rhs.x(), self.y + rhs.y(), self.width, self.height)
    }
}

impl std::ops::AddAssign<Point> for Rect {
    fn add_assign(&mut self, rhs: Point) {
        self.x += rhs.x();
        self.y += rhs.y();
    }
}

impl std::ops::Sub<Point> for Rect {
    type Output = Rect;
    fn sub(self, rhs: Point) -> Rect {
        Rect::new(self.x - rhs.x(), self.y - rhs.y(), self.width, self.height)
    }
}

impl std::ops::SubAssign<Point> for Rect {
    fn sub_assign(&mut self, rhs: Point) {
        self.x -= rhs.x();
        self.y -= rhs.y();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_works() {
        let r = Rect::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(r.x(), 1.0);
        assert_eq!(r.y(), 2.0);
        assert_eq!(r.width(), 3.0);
        assert_eq!(r.height(), 4.0);
    }

    #[test]
    fn equality_works() {
        let r1 = Rect::new(1.0, 2.0, 3.0, 4.0);
        let r2 = Rect::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(r1, r2);
        assert!(r1 == r2);
    }

    #[test]
    fn default_works() {
        let r: Rect = Default::default();
        assert_eq!(r.x(), 0.0);
        assert_eq!(r.y(), 0.0);
        assert_eq!(r.width(), 0.0);
        assert_eq!(r.height(), 0.0);
    }

    #[test]
    fn serialize_to_json_should_work() {
        let r1 = Rect::new(1.0, 2.0, 3.0, 4.0);
        let serialized = serde_json::to_string(&r1).unwrap();
        assert_eq!(serialized, r#"{"x":1.0,"y":2.0,"width":3.0,"height":4.0}"#);
    }

    #[test]
    fn new_and_from_tuple() {
        let r = Rect::new(0.0, 1.0, 2.0, 3.0);
        assert_eq!(r, Rect::from((0.0, 1.0, 2.0, 3.0)));
    }

    #[test]
    fn normalized_handles_negative() {
        // Rect::new normalizes negative width/height by adjusting origin.
        let r = Rect::new(5.0, 6.0, -2.0, -3.0);
        assert_eq!(r.x, 3.0);
        assert_eq!(r.y, 3.0);
        assert_eq!(r.width, 2.0);
        assert_eq!(r.height, 3.0);
    }

    #[test]
    fn contains_and_center() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(r.contains(Point::new(0.0, 0.0)));
        assert!(r.contains(Point::new(9.9999, 5.0)));
        assert!(!r.contains(Point::new(10.0, 5.0)));
        let c = r.center();
        assert_eq!(c, Point::new(5.0, 5.0));
    }

    #[test]
    fn intersects_and_intersection() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 10.0, 10.0);
        assert!(a.intersects(&b));
        let inter = a.intersection(&b).unwrap();
        assert_eq!(inter, Rect::new(5.0, 5.0, 5.0, 5.0));

        let c = Rect::new(20.0, 20.0, 1.0, 1.0);
        assert!(!a.intersects(&c));
        assert!(a.intersection(&c).is_none());
    }

    #[test]
    fn union_works() {
        let a = Rect::new(0.0, 0.0, 2.0, 2.0);
        let b = Rect::new(1.0, 1.0, 2.0, 2.0);
        let u = a.union(&b);
        assert_eq!(u, Rect::new(0.0, 0.0, 3.0, 3.0));
    }

    #[test]
    fn translate_inflate_empty() {
        let r = Rect::new(1.0, 2.0, 3.0, 4.0);

        assert_eq!(r.translate(1.0, -1.0), Rect::new(2.0, 1.0, 3.0, 4.0));
        assert_eq!(r.inflate(1.0, 2.0), Rect::new(0.0, 0.0, 5.0, 8.0));

        assert!(!r.is_empty());
        assert!(Rect::new(0.0, 0.0, 0.0, 5.0).is_empty());
    }
}
