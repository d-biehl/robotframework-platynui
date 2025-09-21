use crate::types::{Point, Rect, Size};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Runtime value representation used for attributes and pattern data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UiValue {
    Null,
    Bool(bool),
    Integer(i64),
    Number(f64),
    String(String),
    Array(Vec<UiValue>),
    Object(BTreeMap<String, UiValue>),
    Point(Point),
    Size(Size),
    Rect(Rect),
}

impl UiValue {
    pub fn object() -> Self {
        UiValue::Object(BTreeMap::new())
    }

    pub fn is_null(&self) -> bool {
        matches!(self, UiValue::Null)
    }
}

impl From<bool> for UiValue {
    fn from(value: bool) -> Self {
        UiValue::Bool(value)
    }
}

impl From<i64> for UiValue {
    fn from(value: i64) -> Self {
        UiValue::Integer(value)
    }
}

impl From<f64> for UiValue {
    fn from(value: f64) -> Self {
        UiValue::Number(value)
    }
}

impl From<&str> for UiValue {
    fn from(value: &str) -> Self {
        UiValue::String(value.to_owned())
    }
}

impl From<String> for UiValue {
    fn from(value: String) -> Self {
        UiValue::String(value)
    }
}

impl From<Point> for UiValue {
    fn from(value: Point) -> Self {
        UiValue::Point(value)
    }
}

impl From<Size> for UiValue {
    fn from(value: Size) -> Self {
        UiValue::Size(value)
    }
}

impl From<Rect> for UiValue {
    fn from(value: Rect) -> Self {
        UiValue::Rect(value)
    }
}

impl<T> From<Vec<T>> for UiValue
where
    T: Into<UiValue>,
{
    fn from(value: Vec<T>) -> Self {
        UiValue::Array(value.into_iter().map(Into::into).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Point, Rect, Size};

    #[test]
    fn stores_point_variant() {
        if let UiValue::Point(point) = UiValue::from(Point::new(10.0, 20.0)) {
            assert_eq!(point.x(), 10.0);
            assert_eq!(point.y(), 20.0);
        } else {
            panic!("expected point variant");
        }
    }

    #[test]
    fn stores_rect_variant() {
        if let UiValue::Rect(rect) = UiValue::from(Rect::new(0.0, 1.0, 2.0, 3.0)) {
            assert_eq!(rect.width(), 2.0);
            assert_eq!(rect.height(), 3.0);
        } else {
            panic!("expected rect variant");
        }
    }

    #[test]
    fn stores_size_variant() {
        if let UiValue::Size(size) = UiValue::from(Size::new(5.0, 6.0)) {
            assert_eq!(size.width(), 5.0);
            assert_eq!(size.height(), 6.0);
        } else {
            panic!("expected size variant");
        }
    }
}
