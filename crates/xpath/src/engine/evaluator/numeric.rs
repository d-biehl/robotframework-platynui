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
///
/// Handles all 13 integer subtypes (`xs:long`, `xs:short`, `xs:byte`,
/// `xs:unsigned*`, etc.) by promoting them to `i64` for arithmetic.
pub(crate) fn classify(v: &XdmAtomicValue) -> Option<NumKind> {
    use XdmAtomicValue::*;
    match v {
        Integer(i) | Long(i) | NonPositiveInteger(i) | NegativeInteger(i) => Some(NumKind::Int(*i)),
        Int(i) => Some(NumKind::Int(*i as i64)),
        Short(i) => Some(NumKind::Int(*i as i64)),
        Byte(i) => Some(NumKind::Int(*i as i64)),
        UnsignedInt(i) => Some(NumKind::Int(*i as i64)),
        UnsignedShort(i) => Some(NumKind::Int(*i as i64)),
        UnsignedByte(i) => Some(NumKind::Int(*i as i64)),
        UnsignedLong(i) | NonNegativeInteger(i) | PositiveInteger(i) => {
            // u64 may overflow i64; clamp via try_into so arithmetic on
            // huge unsigned values still works (falls back to None only if
            // truly out of range, but i64::MAX = 9.2e18 which covers most
            // practical cases).
            Some(NumKind::Int((*i).try_into().ok()?))
        }
        Decimal(d) => Some(NumKind::Dec(*d)),
        Float(f) => Some(NumKind::Float(*f)),
        Double(d) => Some(NumKind::Double(*d)),
        _ => None,
    }
}

/// Promote two [`NumKind`] values to a common type following XPath numeric
/// promotion rules (minimal promotion: integer+integer stays integer,
/// integer+decimal→decimal, any+double→double, etc.).
pub(crate) fn unify_numeric(a: NumKind, b: NumKind) -> (NumKind, NumKind) {
    use NumKind::*;
    use rust_decimal::prelude::ToPrimitive;
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

    // Handle all integer subtypes via centralized as_i128()
    if let Some(i) = a.as_i128() {
        return Ok(Some((NumericKind::Integer, i as f64)));
    }

    Ok(match a {
        Decimal(d) => {
            use rust_decimal::prelude::ToPrimitive;
            Some((NumericKind::Decimal, d.to_f64().unwrap_or(0.0)))
        }
        Float(f) => Some((NumericKind::Float, *f as f64)),
        Double(d) => Some((NumericKind::Double, *d)),
        UntypedAtomic(s) => {
            if let Ok(parsed) = s.parse::<f64>() {
                Some((NumericKind::Double, parsed))
            } else {
                None
            }
        }
        String(_) | AnyUri(_) => None,
        Boolean(b) => Some((NumericKind::Integer, if *b { 1.0 } else { 0.0 })),
        _ => None,
    })
}

/// Extract an integer value as `i128` from an XDM atomic, if possible.
///
/// Delegates to [`XdmAtomicValue::as_i128()`] for all integer subtypes,
/// and additionally handles `Boolean` (true → 1, false → 0).
pub(crate) fn a_as_i128(a: &XdmAtomicValue) -> Option<i128> {
    if let Some(v) = a.as_i128() {
        return Some(v);
    }
    if let XdmAtomicValue::Boolean(b) = a {
        return Some(if *b { 1 } else { 0 });
    }
    None
}
