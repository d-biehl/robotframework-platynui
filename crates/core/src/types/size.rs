use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter, Result};
use std::ops::{Add, Div, Mul, Sub};

#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Size {
    width: f64,
    height: f64,
}

impl Size {
    /// Create a new Size. Negative dimensions are clamped to 0.0.
    pub fn new(width: f64, height: f64) -> Self {
        Size {
            width: if width.is_finite() { width.max(0.0) } else { width },
            height: if height.is_finite() { height.max(0.0) } else { height },
        }
    }

    pub fn width(&self) -> f64 {
        self.width
    }
    pub fn height(&self) -> f64 {
        self.height
    }

    pub fn to_tuple(&self) -> (f64, f64) {
        (self.width, self.height)
    }

    pub fn area(&self) -> f64 {
        if self.width > 0.0 && self.height > 0.0 { self.width * self.height } else { 0.0 }
    }

    pub fn is_empty(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    pub fn is_finite(&self) -> bool {
        self.width.is_finite() && self.height.is_finite()
    }
}

impl Add for Size {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self { width: self.width + other.width, height: self.height + other.height }
    }
}

impl Sub for Size {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self { width: self.width - other.width, height: self.height - other.height }
    }
}

// Note: assign traits removed to keep Size immutable; use non-mutating operators instead.

impl From<(f64, f64)> for Size {
    fn from(t: (f64, f64)) -> Self {
        Size::new(t.0, t.1)
    }
}

impl Display for Size {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "({}, {})", self.width, self.height)
    }
}

impl Mul<f64> for Size {
    type Output = Size;
    fn mul(self, rhs: f64) -> Size {
        Size::new(self.width * rhs, self.height * rhs)
    }
}

impl Div<f64> for Size {
    type Output = Size;
    fn div(self, rhs: f64) -> Size {
        Size::new(self.width / rhs, self.height / rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_works() {
        let s1 = Size::new(1.0, 2.0);
        assert_eq!(s1.width(), 1.0);
        assert_eq!(s1.height(), 2.0);
    }

    #[test]
    fn equality_works() {
        let s1 = Size::new(1.0, 2.0);
        let s2 = Size::new(1.0, 2.0);
        assert_eq!(s1, s2);
        assert!(s1 == s2);
    }

    #[test]
    fn default_works() {
        let s: Size = Default::default();
        assert_eq!(s.width(), 0.0);
        assert_eq!(s.height(), 0.0);
    }

    #[test]
    fn add_works() {
        let s1 = Size::new(1.0, 2.0);
        let s2 = Size::new(3.0, 4.0);
        let s3 = s1 + s2;
        assert_eq!(s3.width(), 4.0);
        assert_eq!(s3.height(), 6.0);
    }

    #[test]
    fn add_assign_works() {
        let s1 = Size::new(1.0, 2.0);
        let s2 = Size::new(3.0, 4.0);
        let s1 = s1 + s2;
        assert_eq!(s1.width(), 4.0);
        assert_eq!(s1.height(), 6.0);
    }

    #[test]
    fn sub_works() {
        let s1 = Size::new(3.0, 4.0);
        let s2 = Size::new(1.0, 2.0);
        let s3 = s1 - s2;
        assert_eq!(s3.width(), 2.0);
        assert_eq!(s3.height(), 2.0);
    }

    #[test]
    fn sub_assign_works() {
        let s1 = Size::new(3.0, 4.0);
        let s2 = Size::new(1.0, 2.0);
        let s1 = s1 - s2;
        assert_eq!(s1.width(), 2.0);
        assert_eq!(s1.height(), 2.0);
    }

    #[test]
    fn serialize_to_json_should_work() {
        let s1 = Size::new(1.0, 2.0);
        let serialized = serde_json::to_string(&s1).unwrap();
        assert_eq!(serialized, r#"{"width":1.0,"height":2.0}"#);
    }

    #[test]
    fn new_clamps_negatives() {
        let s = Size::new(-1.0, -2.0);
        assert_eq!(s.width(), 0.0);
        assert_eq!(s.height(), 0.0);
    }

    #[test]
    fn from_and_to_tuple_and_area_empty() {
        let s: Size = (3.0, 4.0).into();
        assert_eq!(s.to_tuple(), (3.0, 4.0));
        assert_eq!(s.area(), 12.0);
        assert!(!s.is_empty());
        let e = Size::new(0.0, 5.0);
        assert!(e.is_empty());
    }

    #[test]
    fn mul_div_and_assign() {
        let s = Size::new(2.0, 4.0);
        let m = s * 2.0;
        assert_eq!(m, Size::new(4.0, 8.0));
        let d = m / 2.0;
        assert_eq!(d, s);
        let t = s * 3.0;
        assert_eq!(t, Size::new(6.0, 12.0));
        let t = t / 3.0;
        assert_eq!(t, s);
    }

    #[test]
    fn is_finite_checks() {
        let s = Size::new(1.0, 2.0);
        assert!(s.is_finite());
        let q = Size::new(f64::INFINITY, 1.0);
        assert!(!q.is_finite());
    }
}
