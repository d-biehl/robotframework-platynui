//! Unified numeric classification and promotion helpers for the XPath evaluator.
//!
//! Provides [`NumKind`] which classifies XDM atomic values into the four XPath
//! numeric types, carrying the promoted value. Used by both the arithmetic and
//! comparison paths in the evaluator to avoid code duplication.
//!
//! Also provides [`NumericKind`], a pure type-tag variant (without carried
//! values) used by aggregate functions (`sum`, `avg`) that maintain separate
//! accumulators while tracking the promoted result type.

use crate::engine::runtime::Error;
use crate::xdm::XdmAtomicValue;

/// Numeric classification carrying the promoted value.
#[derive(Clone, Copy)]
pub(crate) enum NumKind {
    Int(i64),
    Dec(rust_decimal::Decimal),
    Float(f32),
    Double(f64),
}

impl NumKind {
    /// Convert any numeric kind to f64 (lossy for Decimal).
    pub(crate) fn to_f64(self) -> f64 {
        use rust_decimal::prelude::ToPrimitive;
        match self {
            NumKind::Int(i) => i as f64,
            NumKind::Dec(d) => d.to_f64().unwrap_or(f64::NAN),
            NumKind::Float(f) => f as f64,
            NumKind::Double(d) => d,
        }
    }
}

/// Classify an XDM atomic value into a [`NumKind`], if it is numeric.
pub(crate) fn classify(v: &XdmAtomicValue) -> Option<NumKind> {
    match v {
        XdmAtomicValue::Integer(i) => Some(NumKind::Int(*i)),
        XdmAtomicValue::Decimal(d) => Some(NumKind::Dec(*d)),
        XdmAtomicValue::Float(f) => Some(NumKind::Float(*f)),
        XdmAtomicValue::Double(d) => Some(NumKind::Double(*d)),
        _ => None,
    }
}

/// Promote two [`NumKind`] values to a common type following XPath numeric
/// promotion rules (minimal promotion: integer+integer stays integer,
/// integer+decimal→decimal, any+double→double, etc.).
pub(crate) fn unify_numeric(a: NumKind, b: NumKind) -> (NumKind, NumKind) {
    use rust_decimal::prelude::ToPrimitive;
    use NumKind::*;
    match (a, b) {
        (Double(x), y) => (Double(x), Double(y.to_f64())),
        (y, Double(x)) => (Double(y.to_f64()), Double(x)),
        (Float(x), Float(y)) => (Float(x), Float(y)),
        (Float(x), Int(y)) => (Float(x), Float(y as f32)),
        (Int(x), Float(y)) => (Float(x as f32), Float(y)),
        (Float(x), Dec(y)) => (Float(x), Float(y.to_f32().unwrap_or(f32::NAN))),
        (Dec(x), Float(y)) => (Float(x.to_f32().unwrap_or(f32::NAN)), Float(y)),
        (Dec(x), Dec(y)) => (Dec(x), Dec(y)),
        (Dec(x), Int(y)) => (Dec(x), Dec(rust_decimal::Decimal::from(y))),
        (Int(x), Dec(y)) => (Dec(rust_decimal::Decimal::from(x)), Dec(y)),
        (Int(x), Int(y)) => (Int(x), Int(y)),
    }
}

// ---------------------------------------------------------------------------
// NumericKind — pure type tag for aggregate operations (sum, avg, min, max)
// ---------------------------------------------------------------------------

/// Pure numeric type tag without a carried value.
///
/// Used by aggregate functions (`sum`, `avg`) that maintain separate
/// accumulators (`i128`, `Decimal`) while tracking the promoted result type.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum NumericKind {
    Integer,
    Decimal,
    Float,
    Double,
}

impl NumericKind {
    /// Promote two numeric kinds to their common supertype.
    pub(crate) fn promote(self, other: NumericKind) -> NumericKind {
        use NumericKind::*;
        match (self, other) {
            (Double, _) | (_, Double) => Double,
            (Float, _) | (_, Float) => {
                if matches!(self, Double) || matches!(other, Double) {
                    Double
                } else {
                    Float
                }
            }
            (Decimal, _) | (_, Decimal) => match (self, other) {
                (Integer, Decimal) | (Decimal, Integer) | (Decimal, Decimal) => Decimal,
                (Decimal, Float) | (Float, Decimal) => Float,
                (Decimal, Double) | (Double, Decimal) => Double,
                _ => Decimal,
            },
            (Integer, Integer) => Integer,
        }
    }
}

/// Classify an XDM atomic value into a [`NumericKind`] tag and its `f64`
/// representation. Returns `None` for non-numeric types.
pub(crate) fn classify_numeric(a: &XdmAtomicValue) -> Result<Option<(NumericKind, f64)>, Error> {
    use XdmAtomicValue::*;
    Ok(match a {
        Integer(i) => Some((NumericKind::Integer, *i as f64)),
        Long(i) => Some((NumericKind::Integer, *i as f64)),
        Int(i) => Some((NumericKind::Integer, *i as f64)),
        Short(i) => Some((NumericKind::Integer, *i as f64)),
        Byte(i) => Some((NumericKind::Integer, *i as f64)),
        UnsignedLong(i) => Some((NumericKind::Integer, *i as f64)),
        UnsignedInt(i) => Some((NumericKind::Integer, *i as f64)),
        UnsignedShort(i) => Some((NumericKind::Integer, *i as f64)),
        UnsignedByte(i) => Some((NumericKind::Integer, *i as f64)),
        NonPositiveInteger(i) => Some((NumericKind::Integer, *i as f64)),
        NegativeInteger(i) => Some((NumericKind::Integer, *i as f64)),
        NonNegativeInteger(i) => Some((NumericKind::Integer, *i as f64)),
        PositiveInteger(i) => Some((NumericKind::Integer, *i as f64)),
        Decimal(d) => {
            use rust_decimal::prelude::ToPrimitive;
            Some((NumericKind::Decimal, d.to_f64().unwrap_or(0.0)))
        }
        Float(f) => Some((NumericKind::Float, *f as f64)),
        Double(d) => Some((NumericKind::Double, *d)),
        UntypedAtomic(s) => {
            if let Ok(parsed) = s.parse::<f64>() { Some((NumericKind::Double, parsed)) } else { None }
        }
        String(_) | AnyUri(_) => None,
        Boolean(b) => Some((NumericKind::Integer, if *b { 1.0 } else { 0.0 })),
        _ => None,
    })
}

/// Extract an integer value as `i128` from an XDM atomic, if possible.
pub(crate) fn a_as_i128(a: &XdmAtomicValue) -> Option<i128> {
    use XdmAtomicValue::*;
    Some(match a {
        Integer(i) => *i as i128,
        Long(i) => *i as i128,
        Int(i) => *i as i128,
        Short(i) => *i as i128,
        Byte(i) => *i as i128,
        UnsignedLong(i) => *i as i128,
        UnsignedInt(i) => *i as i128,
        UnsignedShort(i) => *i as i128,
        UnsignedByte(i) => *i as i128,
        NonPositiveInteger(i) => *i as i128,
        NegativeInteger(i) => *i as i128,
        NonNegativeInteger(i) => *i as i128,
        PositiveInteger(i) => *i as i128,
        Boolean(b) => {
            if *b {
                1
            } else {
                0
            }
        }
        _ => return None,
    })
}
