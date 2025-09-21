use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter, Result};
use std::ops::{Add, Sub};

#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    x: f64,
    y: f64,
}

impl Point {
    /// Create a new point
    pub fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }

    /// Convert to tuple
    pub fn to_tuple(&self) -> (f64, f64) {
        (self.x, self.y)
    }

    pub fn x(&self) -> f64 {
        self.x
    }
    pub fn y(&self) -> f64 {
        self.y
    }

    pub fn with_x(&self, x: f64) -> Self {
        Point { x, y: self.y }
    }
    pub fn with_y(&self, y: f64) -> Self {
        Point { x: self.x, y }
    }

    /// Translate and return a new Point
    pub fn translate(&self, dx: f64, dy: f64) -> Self {
        Point::new(self.x + dx, self.y + dy)
    }

    pub fn is_finite(&self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl Add for Point {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self { x: self.x + other.x, y: self.y + other.y }
    }
}
impl Sub for Point {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self { x: self.x - other.x, y: self.y - other.y }
    }
}

impl From<(f64, f64)> for Point {
    fn from(t: (f64, f64)) -> Self {
        Point::new(t.0, t.1)
    }
}

impl Display for Point {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        // default plain representation
        write!(f, "({}, {})", self.x, self.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_works() {
        let p1 = Point::new(1.0, 2.0);
        assert_eq!(p1.x(), 1.0);
        assert_eq!(p1.y(), 2.0);

        let p2 = Point::new(1.0, 2.0);
        assert_eq!(p1, p2);
        assert!(p1 == p2);
    }

    #[test]
    fn default_works() {
        let p: Point = Default::default();
        assert_eq!(p.x(), 0.0);
        assert_eq!(p.y(), 0.0);
    }

    #[test]
    fn add_works() {
        let p1 = Point::new(1.0, 2.0);
        let p2 = Point::new(3.0, 4.0);
        let p3 = p1 + p2;
        assert_eq!(p3.x(), 4.0);
        assert_eq!(p3.y(), 6.0);
    }

    #[test]
    fn add_assign_works() {
        let p1 = Point::new(1.0, 2.0);
        let p2 = Point::new(3.0, 4.0);
        let p1 = p1 + p2;
        assert_eq!(p1.x(), 4.0);
        assert_eq!(p1.y(), 6.0);
    }

    #[test]
    fn sub_works() {
        let p1 = Point::new(3.0, 4.0);
        let p2 = Point::new(1.0, 2.0);
        let p3 = p1 - p2;
        assert_eq!(p3.x(), 2.0);
        assert_eq!(p3.y(), 2.0);
    }

    #[test]
    fn sub_assign_works() {
        let p1 = Point::new(3.0, 4.0);
        let p2 = Point::new(1.0, 2.0);
        let p1 = p1 - p2;
        assert_eq!(p1.x(), 2.0);
        assert_eq!(p1.y(), 2.0);
    }

    #[test]
    fn serialize_to_json_should_work() {
        let p1 = Point::new(1.0, 2.0);
        let serialized = serde_json::to_string(&p1).unwrap();
        assert_eq!(serialized, r#"{"x":1.0,"y":2.0}"#);
    }

    #[test]
    fn new_and_from_tuple_and_to_tuple() {
        let p = Point::new(3.0, 4.0);
        assert_eq!(p.to_tuple(), (3.0, 4.0));
        let q: Point = (3.0, 4.0).into();
        assert_eq!(p, q);
    }

    #[test]
    fn display_to_string() {
        let p = Point::new(1.0, 2.0);
        assert_eq!(p.to_string(), "(1, 2)");
    }
}
