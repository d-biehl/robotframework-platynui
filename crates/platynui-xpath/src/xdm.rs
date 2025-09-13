use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime};
use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExpandedName {
    pub ns_uri: Option<String>,
    pub local: String,
}

impl ExpandedName {
    pub fn new(ns_uri: Option<String>, local: impl Into<String>) -> Self {
        Self {
            ns_uri,
            local: local.into(),
        }
    }
}

/// Subset + extensions of the XDM 2.0 atomic type universe.
/// Rationale:
/// - Numeric subtypes stored distinctly to allow precise instance-of checks later
///   without lossy coercion.
/// - String-derived subtypes keep lexical form only (no extra invariants enforced yet).
/// - g* date fragments + durations retained for potential future function support.
/// - Binary types keep original lexical encoding; decoding deferred until required.
#[derive(Debug, Clone, PartialEq)]
pub enum XdmAtomicValue {
    Boolean(bool),
    String(String),
    Integer(i64),
    Decimal(f64),
    Double(f64),
    Float(f32),
    AnyUri(String),
    QName {
        ns_uri: Option<String>,
        prefix: Option<String>,
        local: String,
    },
    UntypedAtomic(String),
    DateTime(DateTime<FixedOffset>),
    Date {
        date: NaiveDate,
        tz: Option<FixedOffset>,
    },
    Time {
        time: NaiveTime,
        tz: Option<FixedOffset>,
    },
    YearMonthDuration(i32),
    DayTimeDuration(i64),
    // Additional numeric subtypes (stored losslessly or mapped onto existing primitives)
    Long(i64),
    Int(i32),
    Short(i16),
    Byte(i8),
    UnsignedLong(u64),
    UnsignedInt(u32),
    UnsignedShort(u16),
    UnsignedByte(u8),
    NonPositiveInteger(i64),
    NegativeInteger(i64),
    NonNegativeInteger(u64),
    PositiveInteger(u64),
    // Binary types (base64/hex retain original lexical form; decode deferred)
    Base64Binary(String),
    HexBinary(String),
    // g* date/time fragment types
    GYear {
        year: i32,
        tz: Option<FixedOffset>,
    },
    GYearMonth {
        year: i32,
        month: u8,
        tz: Option<FixedOffset>,
    },
    GMonth {
        month: u8,
        tz: Option<FixedOffset>,
    },
    GMonthDay {
        month: u8,
        day: u8,
        tz: Option<FixedOffset>,
    },
    GDay {
        day: u8,
        tz: Option<FixedOffset>,
    },
    // String-derived subtypes (no separate storage; kept as canonical string)
    NormalizedString(String),
    Token(String),
    Language(String),
    Name(String),
    NCName(String),
    NMTOKEN(String),
    Id(String),
    IdRef(String),
    Entity(String),
    Notation(String),
}

pub type XdmSequence<N> = Vec<XdmItem<N>>;

#[derive(Debug, Clone, PartialEq)]
pub enum XdmItem<N> {
    Node(N),
    Atomic(XdmAtomicValue),
}

// Convenience conversion: allow passing a node directly where an XdmItem<N> is expected.
impl<N> From<N> for XdmItem<N> {
    fn from(n: N) -> Self {
        XdmItem::Node(n)
    }
}

impl<N> fmt::Display for XdmItem<N>
where
    N: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XdmItem::Node(_) => write!(f, "<node>"),
            XdmItem::Atomic(a) => write!(f, "{:?}", a),
        }
    }
}
