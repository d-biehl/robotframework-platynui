//! Equality Kernel (EqKey) – central canonicalization for atomic & node values.
//! Provides a reusable hash/equality key used by
//! distinct-values, deep-equal, index-of, compare, codepoint-equal, min/max.
//!
//! Scope (initial): numeric, string (with collation key), boolean, QName, date/time,
//! durations, nodes (identity), NaN sentinel, and a conservative fallback bucket
//! for yet-unhandled atomic kinds (Other). This avoids sprinkling ad-hoc matching
//! logic across functions.rs and evaluator.rs.
//!
//! Design notes:
//! * Numeric normalization collapses the integer tower and decimal into a lossless
//!   (mantissa, scale) pair when derived from Decimal; float/double retain IEEE value
//!   except that -0.0 normalizes to 0.0 and all NaN collapse into EqKey::NaN.
//! * Promotion during equality already happened at higher layers (XPath rules). For
//!   distinct-values / deep-equal we nevertheless require stable hashing across
//!   representations that should compare equal; hence Integer(10) and Decimal(10.0)
//!   map to the same NumericKey (mantissa=10, scale=0).
//! * Date/Time values are converted to an absolute instant in nanoseconds (i128) plus
//!   a kind discriminator preventing cross-kind equality (Date vs DateTime vs Time).
//!   (Current engine only stores timezone-aware DateTime; pure Date/Time hold tz opt.)
//! * Durations: yearMonthDuration normalized to total months (i64); dayTimeDuration to
//!   total nanoseconds (i128). Kinds are not considered equal cross-wise per spec.
//! * Node identity uses the pointer address (stable for Arc-backed SimpleNode). For
//!   user supplied adapters we expect Eq/Hash semantics consistent with pointer/address.
//! * Collations: we store both original string and a key produced by Collation::key.
//!   Equality uses the key; original preserved for potential reconstruction.
//! * Other bucket: packs a short type tag and a cheap lexical representation for
//!   future types (binary, g* fragments, NOTATION) until specialized variants land.
//!
//! No panics / unwraps on user data paths – failures surface as Error.

use crate::engine::collation::Collation;
use crate::engine::runtime::Error;
use crate::model::XdmNode;
use crate::xdm::XdmAtomicValue;
use crate::xdm::XdmItem;

use chrono::TimeZone;
use compact_str::CompactString;
use core::hash::{Hash, Hasher};

const NANOS_PER_SECOND: i128 = 1_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DateTimeKind {
    Date,
    DateTime,
    Time,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DurationKind {
    YearMonth,
    DayTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DecimalKey {
    pub mantissa: i128,
    pub scale: i16,
}

#[derive(Debug, Clone)]
pub enum NumericKey {
    // Whole number (any integer-subtype) stored as i128
    Integer(i128),
    // Decimal in canonical (mantissa, scale) with no trailing zeros in mantissa (except 0)
    Decimal(DecimalKey),
    // IEEE floats/doubles – NaN excluded (handled as EqKey::NaN); -0.0 coerced to 0.0
    Float(f32),
    Double(f64),
}

impl Hash for NumericKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            NumericKey::Integer(i) => {
                0u8.hash(state);
                i.hash(state);
            }
            NumericKey::Decimal(d) => {
                1u8.hash(state);
                d.mantissa.hash(state);
                d.scale.hash(state);
            }
            NumericKey::Float(f) => {
                2u8.hash(state);
                f.to_bits().hash(state);
            }
            NumericKey::Double(d) => {
                3u8.hash(state);
                d.to_bits().hash(state);
            }
        }
    }
}
impl Eq for NumericKey {}
impl PartialEq for NumericKey {
    fn eq(&self, other: &Self) -> bool {
        use NumericKey::*;
        match (self, other) {
            (Integer(a), Integer(b)) => a == b,
            (Decimal(a), Decimal(b)) => a.mantissa == b.mantissa && a.scale == b.scale,
            (Float(a), Float(b)) => a.to_bits() == b.to_bits(),
            (Double(a), Double(b)) => a.to_bits() == b.to_bits(),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QNameKey {
    pub ns: Option<CompactString>,
    pub local: CompactString,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DurationKey {
    pub kind: DurationKind,
    pub months: i64,
    /// DayTimeDuration values are stored as nanoseconds to align with Date/Time keys.
    pub nanos: i128,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DateTimeKey {
    pub kind: DateTimeKind,
    pub instant_ns: i128,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StringKey {
    pub key: CompactString,
    pub original: CompactString,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OtherKey {
    pub type_tag: u16,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum EqKey {
    Numeric(NumericKey),
    String(StringKey),
    QName(QNameKey),
    Boolean(bool),
    DateTime(DateTimeKey),
    Duration(DurationKey),
    Node(u64),
    NaN, // collapses all NaN representations
    Other(OtherKey),
}
impl Eq for EqKey {}
impl PartialEq for EqKey {
    fn eq(&self, other: &Self) -> bool {
        use EqKey::*;
        match (self, other) {
            (Numeric(a), Numeric(b)) => a == b,
            (String(a), String(b)) => a.key == b.key,
            (QName(a), QName(b)) => a.ns == b.ns && a.local == b.local,
            (Boolean(a), Boolean(b)) => a == b,
            (DateTime(a), DateTime(b)) => a.kind == b.kind && a.instant_ns == b.instant_ns,
            (Duration(a), Duration(b)) => a.kind == b.kind && a.months == b.months && a.nanos == b.nanos,
            (Node(a), Node(b)) => a == b,
            (NaN, NaN) => true,
            (Other(a), Other(b)) => a.type_tag == b.type_tag && a.bytes == b.bytes,
            _ => false,
        }
    }
}

impl core::hash::Hash for EqKey {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        use EqKey::*;
        match self {
            Numeric(n) => {
                0u8.hash(state);
                n.hash(state);
            }
            String(s) => {
                1u8.hash(state);
                s.key.hash(state);
            }
            QName(q) => {
                2u8.hash(state);
                q.ns.hash(state);
                q.local.hash(state);
            }
            Boolean(b) => {
                3u8.hash(state);
                b.hash(state);
            }
            DateTime(dt) => {
                4u8.hash(state);
                dt.kind.hash(state);
                dt.instant_ns.hash(state);
            }
            Duration(d) => {
                5u8.hash(state);
                d.kind.hash(state);
                d.months.hash(state);
                d.nanos.hash(state);
            }
            Node(id) => {
                6u8.hash(state);
                id.hash(state);
            }
            NaN => {
                7u8.hash(state);
            }
            Other(o) => {
                8u8.hash(state);
                o.type_tag.hash(state);
                o.bytes.hash(state);
            }
        }
    }
}

// Helper: strip trailing zeros from a decimal mantissa; scale adjusted accordingly.
fn normalize_decimal(mut mantissa: i128, mut scale: i16) -> (i128, i16) {
    if mantissa == 0 {
        return (0, 0);
    }
    while scale > 0 && mantissa % 10 == 0 {
        mantissa /= 10;
        scale -= 1;
    }
    (mantissa, scale)
}

fn canonicalize_decimal(v: f64) -> DecimalKey {
    // NOTE: Current Decimal storage uses f64 upstream. We approximate a mantissa/scale by
    // formatting then stripping trailing zeros; this is a stop-gap until a BigDecimal /
    // exact decimal representation is introduced. Accept minor risk of floating artifacts.
    if v == 0.0 {
        return DecimalKey { mantissa: 0, scale: 0 };
    }
    let s = format!("{v:.15}"); // high precision string
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    if let Some(dot) = trimmed.find('.') {
        // unlikely after trimming but safe guard
        let int_part = &trimmed[..dot];
        let frac_part = &trimmed[dot + 1..];
        let scale = frac_part.len() as i16;
        let digits = format!("{int_part}{frac_part}");
        let mantissa: i128 = digits.parse().unwrap_or(0);
        let (m, s) = normalize_decimal(mantissa, scale);
        DecimalKey { mantissa: m, scale: s }
    } else {
        DecimalKey { mantissa: trimmed.parse().unwrap_or(0), scale: 0 }
    }
}

fn float_norm(f: f32) -> f32 {
    if f == -0.0 { 0.0 } else { f }
}
fn double_norm(d: f64) -> f64 {
    if d == -0.0 { 0.0 } else { d }
}

// Convert date/time forms to instant nanoseconds (UTC). For now we rely on stored chrono
// values; Date with tz -> midnight that date + offset; Time with tz -> reference date 1970-01-01.
fn date_time_instant_ns(dt: &XdmAtomicValue) -> Option<(DateTimeKind, i128)> {
    use XdmAtomicValue::*;
    match dt {
        DateTime(d) => Some((DateTimeKind::DateTime, safe_nanos(d))),
        Date { date, tz } => {
            // Only produce a key for timezone-aware dates; tz-less dates remain as Other
            if let Some(off) = tz {
                let naive = date.and_hms_opt(0, 0, 0)?;
                let dt = off
                    .from_local_datetime(&naive)
                    .single()
                    .unwrap_or_else(|| chrono::DateTime::from_naive_utc_and_offset(naive, *off));
                Some((DateTimeKind::Date, safe_nanos(&dt)))
            } else {
                None
            }
        }
        Time { time, tz } => {
            // Only produce a key for timezone-aware times; anchor to 1970-01-01
            if let Some(off) = tz {
                let base = chrono::NaiveDate::from_ymd_opt(1970, 1, 1)?.and_time(*time);
                let dt = off
                    .from_local_datetime(&base)
                    .single()
                    .unwrap_or_else(|| chrono::DateTime::from_naive_utc_and_offset(base, *off));
                Some((DateTimeKind::Time, safe_nanos(&dt)))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn safe_nanos(dt: &chrono::DateTime<chrono::FixedOffset>) -> i128 {
    (dt.timestamp() as i128) * 1_000_000_000 + dt.timestamp_subsec_nanos() as i128
}

fn duration_key(a: &XdmAtomicValue) -> Option<DurationKey> {
    use XdmAtomicValue::*;
    match a {
        YearMonthDuration(m) => Some(DurationKey { kind: DurationKind::YearMonth, months: *m as i64, nanos: 0 }),
        DayTimeDuration(d) => {
            Some(DurationKey { kind: DurationKind::DayTime, months: 0, nanos: i128::from(*d) * NANOS_PER_SECOND })
        }
        _ => None,
    }
}

fn numeric_key(a: &XdmAtomicValue) -> Option<NumericKey> {
    use XdmAtomicValue::*;
    Some(match a {
        Integer(i) => NumericKey::Integer(*i as i128),
        Long(i) => NumericKey::Integer(*i as i128),
        Int(i) => NumericKey::Integer(*i as i128),
        Short(i) => NumericKey::Integer(*i as i128),
        Byte(i) => NumericKey::Integer(*i as i128),
        UnsignedLong(i) => NumericKey::Integer(*i as i128),
        UnsignedInt(i) => NumericKey::Integer(*i as i128),
        UnsignedShort(i) => NumericKey::Integer(*i as i128),
        UnsignedByte(i) => NumericKey::Integer(*i as i128),
        NonPositiveInteger(i) => NumericKey::Integer(*i as i128),
        NegativeInteger(i) => NumericKey::Integer(*i as i128),
        NonNegativeInteger(i) => NumericKey::Integer(*i as i128),
        PositiveInteger(i) => NumericKey::Integer(*i as i128),
        Decimal(d) => {
            let dk = canonicalize_decimal(*d);
            if dk.scale == 0 { NumericKey::Integer(dk.mantissa) } else { NumericKey::Decimal(dk) }
        }
        Float(f) if f.is_nan() => return None,
        Float(f) => NumericKey::Float(float_norm(*f)),
        Double(d) if d.is_nan() => return None,
        Double(d) => NumericKey::Double(double_norm(*d)),
        Boolean(b) => NumericKey::Integer(if *b { 1 } else { 0 }), // boolean numeric casting context
        UntypedAtomic(s) => {
            if let Ok(parsed) = s.parse::<f64>() {
                if parsed.is_nan() {
                    return None;
                }
                NumericKey::Double(double_norm(parsed))
            } else {
                return None;
            }
        }
        String(_) | AnyUri(_) => return None,
        _ => return None,
    })
}

/// Build EqKey for an item, using an optional collation for string values.
pub fn build_eq_key<N: XdmNode>(item: &XdmItem<N>, coll: Option<&dyn Collation>) -> Result<EqKey, Error> {
    use XdmItem::*;
    Ok(match item {
        Node(n) => EqKey::Node(ptr_as_u64(n)),
        Atomic(a) => atomic_eq_key(a, coll),
    })
}

fn atomic_eq_key(a: &XdmAtomicValue, coll: Option<&dyn Collation>) -> EqKey {
    use XdmAtomicValue::*;
    if let Some(num) = numeric_key(a) {
        return EqKey::Numeric(num);
    }
    match a {
        Float(f) if f.is_nan() => EqKey::NaN,
        Double(f) if f.is_nan() => EqKey::NaN,
        Boolean(b) => EqKey::Boolean(*b),
        String(s) | AnyUri(s) | UntypedAtomic(s) => {
            let key = if let Some(c) = coll { c.key(s) } else { s.clone() };
            EqKey::String(StringKey { key: key.into(), original: s.clone().into() })
        }
        QName { ns_uri, local, .. } => {
            EqKey::QName(QNameKey { ns: ns_uri.as_ref().map(|s| s.clone().into()), local: local.clone().into() })
        }
        DateTime(_) | Date { .. } | Time { .. } => {
            if let Some((kind, ns)) = date_time_instant_ns(a) {
                EqKey::DateTime(DateTimeKey { kind, instant_ns: ns })
            } else {
                EqKey::Other(OtherKey { type_tag: 1, bytes: format!("{:?}", a).into_bytes() })
            }
        }
        YearMonthDuration(_) | DayTimeDuration(_) => {
            if let Some(dk) = duration_key(a) {
                EqKey::Duration(dk)
            } else {
                EqKey::Other(OtherKey { type_tag: 2, bytes: format!("{:?}", a).into_bytes() })
            }
        }
        Base64Binary(b) | HexBinary(b) => EqKey::Other(OtherKey { type_tag: 10, bytes: b.as_bytes().to_vec() }),
        // g* and string derived types collapse to their string value (spec: value space maps)
        NormalizedString(s) | Token(s) | Language(s) | Name(s) | NCName(s) | NMTOKEN(s) | Id(s) | IdRef(s)
        | Entity(s) | Notation(s) => {
            let key = if let Some(c) = coll { c.key(s) } else { s.clone() };
            EqKey::String(StringKey { key: key.into(), original: s.clone().into() })
        }
        // Fallback – pack debug; will be replaced by specialized handling later.
        _ => EqKey::Other(OtherKey { type_tag: 255, bytes: format!("{:?}", a).into_bytes() }),
    }
}

fn ptr_as_u64<N: XdmNode>(n: &N) -> u64 {
    // Safety: relying on Eq/Hash semantics of underlying node types; for SimpleNode Arc ptr is stable.

    (n as *const N) as usize as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::collation::SimpleCaseCollation;
    use crate::model::simple::{elem, text};
    use crate::xdm::XdmItem;

    #[test]
    fn integer_decimal_unify() {
        let k1 = atomic_eq_key(&XdmAtomicValue::Integer(10), None);
        let k2 = atomic_eq_key(&XdmAtomicValue::Decimal(10.0), None);
        assert_eq!(k1, k2, "integer and decimal 10 should share key");
    }

    #[test]
    fn nan_collapse() {
        let k1 = atomic_eq_key(&XdmAtomicValue::Double(f64::NAN), None);
        let k2 = atomic_eq_key(&XdmAtomicValue::Float(f32::NAN), None);
        assert_eq!(k1, EqKey::NaN);
        assert_eq!(k2, EqKey::NaN);
    }

    #[test]
    fn node_identity_stable() {
        let n = elem("r").child(text("hi")).build();
        let item = XdmItem::from(n.clone());
        let k1 = build_eq_key(&item, None).unwrap();
        let k2 = build_eq_key(&item, None).unwrap();
        assert_eq!(k1, k2);
        if let EqKey::Node(id1) = k1 {
            if let EqKey::Node(id2) = k2 {
                assert_eq!(id1, id2);
            } else {
                panic!("expected node");
            }
        } else {
            panic!("expected node");
        }
    }

    #[test]
    fn collation_string_keys_lowercase() {
        let coll = SimpleCaseCollation; // lowercase implementation
        let k1 = build_eq_key::<crate::model::simple::SimpleNode>(
            &XdmItem::Atomic(XdmAtomicValue::String("Ab".into())),
            Some(&coll),
        )
        .unwrap();
        let k2 = build_eq_key::<crate::model::simple::SimpleNode>(
            &XdmItem::Atomic(XdmAtomicValue::String("ab".into())),
            Some(&coll),
        )
        .unwrap();
        assert_eq!(k1, k2, "case-insensitive collation should unify keys");
    }

    #[test]
    fn day_time_duration_key_scales_to_nanos() {
        let key = atomic_eq_key(&XdmAtomicValue::DayTimeDuration(5), None);
        match key {
            EqKey::Duration(DurationKey { kind, months, nanos }) => {
                assert_eq!(kind, DurationKind::DayTime);
                assert_eq!(months, 0);
                assert_eq!(nanos, 5 * super::NANOS_PER_SECOND);
            }
            other => panic!("expected duration key, got {other:?}"),
        }
    }
}
