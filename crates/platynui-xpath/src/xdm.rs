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
    // M8b: Date/Time/Duration families (minimal modeling for XPath 2.0)
    DateTime(DateTime<FixedOffset>),
    Date {
        date: NaiveDate,
        tz: Option<FixedOffset>,
    },
    Time {
        time: NaiveTime,
        tz: Option<FixedOffset>,
    },
    // Duration types. For simplicity, store canonical values:
    // - YearMonthDuration: total months (can be negative)
    // - DayTimeDuration: total seconds (can be negative)
    YearMonthDuration(i32),
    DayTimeDuration(i64),
}

pub type XdmSequence<N> = Vec<XdmItem<N>>;

#[derive(Debug, Clone, PartialEq)]
pub enum XdmItem<N> {
    Node(N),
    Atomic(XdmAtomicValue),
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
