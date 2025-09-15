use crate::model::{NodeKind, QName, XdmNode};
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
            XdmItem::Node(n) => write!(f, "{:?}", n),
            // Use pretty Display for atomics (quoted strings, humanized numerics, etc.)
            XdmItem::Atomic(a) => write!(f, "{}", a),
        }
    }
}

impl fmt::Display for ExpandedName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.ns_uri {
            Some(ns) if !ns.is_empty() => write!(f, "{{{}}}{}", ns, self.local),
            _ => write!(f, "{}", self.local),
        }
    }
}

impl fmt::Display for XdmAtomicValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use XdmAtomicValue::*;
        match self {
            Boolean(b) => write!(f, "{}", b),
            String(s) => write!(f, "\"{}\"", s),
            Integer(i) => write!(f, "{}", i),
            Decimal(d) => write!(f, "{}D", d),
            Double(d) => write!(f, "{}E", d),
            Float(fl) => write!(f, "{}F", fl),
            AnyUri(u) => write!(f, "anyURI(\"{}\")", u),
            QName {
                ns_uri,
                prefix,
                local,
            } => match (ns_uri, prefix) {
                (Some(ns), Some(p)) => write!(f, "QName(ns='{}', {}:{})", ns, p, local),
                (Some(ns), None) => write!(f, "QName(ns='{}', {})", ns, local),
                (None, Some(p)) => write!(f, "QName({}:{})", p, local),
                (None, None) => write!(f, "QName({})", local),
            },
            UntypedAtomic(s) => write!(f, "untyped(\"{}\")", s),
            DateTime(dt) => write!(f, "dateTime({})", dt.to_rfc3339()),
            Date { date, tz } => match tz {
                Some(tz) => write!(f, "date({} {})", date, tz),
                None => write!(f, "date({})", date),
            },
            Time { time, tz } => match tz {
                Some(tz) => write!(f, "time({} {})", time, tz),
                None => write!(f, "time({})", time),
            },
            YearMonthDuration(m) => write!(f, "ymDur({}m)", m),
            DayTimeDuration(s) => write!(f, "dtDur({}s)", s),
            Long(v) => write!(f, "{}L", v),
            Int(v) => write!(f, "{}i", v),
            Short(v) => write!(f, "{}s", v),
            Byte(v) => write!(f, "{}b", v),
            UnsignedLong(v) => write!(f, "{}UL", v),
            UnsignedInt(v) => write!(f, "{}Ui", v),
            UnsignedShort(v) => write!(f, "{}Us", v),
            UnsignedByte(v) => write!(f, "{}Ub", v),
            NonPositiveInteger(v) => write!(f, "{}(<=0)", v),
            NegativeInteger(v) => write!(f, "{}(<0)", v),
            NonNegativeInteger(v) => write!(f, "{}(>=0)", v),
            PositiveInteger(v) => write!(f, "{}(>0)", v),
            Base64Binary(s) => write!(f, "base64Binary(len={})", s.len()),
            HexBinary(s) => write!(f, "hexBinary(len={})", s.len()),
            GYear { year, tz } => match tz {
                Some(tz) => write!(f, "gYear({} {})", year, tz),
                None => write!(f, "gYear({})", year),
            },
            GYearMonth { year, month, tz } => match tz {
                Some(tz) => write!(f, "gYearMonth({}-{} {})", year, month, tz),
                None => write!(f, "gYearMonth({}-{})", year, month),
            },
            GMonth { month, tz } => match tz {
                Some(tz) => write!(f, "gMonth({} {})", month, tz),
                None => write!(f, "gMonth({})", month),
            },
            GMonthDay { month, day, tz } => match tz {
                Some(tz) => write!(f, "gMonthDay({}-{} {})", month, day, tz),
                None => write!(f, "gMonthDay({}-{})", month, day),
            },
            GDay { day, tz } => match tz {
                Some(tz) => write!(f, "gDay({} {})", day, tz),
                None => write!(f, "gDay({})", day),
            },
            NormalizedString(s) => write!(f, "normalizedString(\"{}\")", s),
            Token(s) => write!(f, "token(\"{}\")", s),
            Language(s) => write!(f, "language(\"{}\")", s),
            Name(s) => write!(f, "name(\"{}\")", s),
            NCName(s) => write!(f, "NCName(\"{}\")", s),
            NMTOKEN(s) => write!(f, "NMTOKEN(\"{}\")", s),
            Id(s) => write!(f, "ID(\"{}\")", s),
            IdRef(s) => write!(f, "IDREF(\"{}\")", s),
            Entity(s) => write!(f, "ENTITY(\"{}\")", s),
            Notation(s) => write!(f, "NOTATION(\"{}\")", s),
        }
    }
}

/// Optional pretty-print wrapper for XdmItem that uses Display for atomics
/// (e.g., strings quoted) while keeping node items compact.
pub struct PrettyItem<'a, N>(pub &'a XdmItem<N>);

impl<'a, N: fmt::Debug> fmt::Display for PrettyItem<'a, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            // Default: prefer Debug for nodes (always available via bound)
            XdmItem::Node(n) => write!(f, "{:?}", n),
            XdmItem::Atomic(a) => write!(f, "{}", a),
        }
    }
}

impl<N> XdmItem<N> {
    /// Return a Display wrapper that renders atomics prettily (quoted strings, etc.).
    pub fn pretty(&self) -> PrettyItem<'_, N> {
        PrettyItem(self)
    }
}

/// Optional pretty-print wrapper for a sequence of XdmItem values.
pub struct PrettySeq<'a, N>(pub &'a [XdmItem<N>]);

impl<'a, N: fmt::Debug> fmt::Display for PrettySeq<'a, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for (i, item) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", PrettyItem(item))?;
        }
        write!(f, "]")
    }
}

impl<'a, N> PrettySeq<'a, N> {
    pub fn new(slice: &'a [XdmItem<N>]) -> Self {
        PrettySeq(slice)
    }
}

/// Optional pretty-print wrapper that prefers Display for node items when available.
pub struct PrettyItemDisplay<'a, N>(pub &'a XdmItem<N>);

impl<'a, N> fmt::Display for PrettyItemDisplay<'a, N>
where
    N: fmt::Display + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            XdmItem::Node(n) => write!(f, "{}", n),
            XdmItem::Atomic(a) => write!(f, "{}", a),
        }
    }
}

impl<N> XdmItem<N> {
    /// Prefer Display for node items if `N: Display`.
    pub fn pretty_display(&self) -> PrettyItemDisplay<'_, N>
    where
        N: fmt::Display,
    {
        PrettyItemDisplay(self)
    }
}

/// Pretty printer for sequences that prefers Display for node items when available.
pub struct PrettySeqDisplay<'a, N>(pub &'a [XdmItem<N>]);

impl<'a, N> PrettySeqDisplay<'a, N> {
    pub fn new(slice: &'a [XdmItem<N>]) -> Self {
        PrettySeqDisplay(slice)
    }
}

impl<'a, N> fmt::Display for PrettySeqDisplay<'a, N>
where
    N: fmt::Display + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for (i, item) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", PrettyItemDisplay(item))?;
        }
        write!(f, "]")
    }
}

/// Pretty printer for a single XdmItem using the XdmNode trait (no need for node Display).
pub struct PrettyNodeItem<'a, N>(pub &'a XdmItem<N>);

impl<'a, N: XdmNode> fmt::Display for PrettyNodeItem<'a, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn qname_to_string(q: &QName) -> String {
            match (&q.prefix, &q.local) {
                (Some(p), local) if !p.is_empty() => format!("{}:{}", p, local),
                _ => q.local.clone(),
            }
        }
        fn clip(s: &str) -> String {
            const MAX: usize = 32;
            if s.len() > MAX {
                let mut out = s.chars().take(MAX).collect::<String>();
                out.push_str("…");
                out
            } else {
                s.to_string()
            }
        }
        match self.0 {
            XdmItem::Atomic(a) => write!(f, "{}", a),
            XdmItem::Node(n) => match n.kind() {
                NodeKind::Document => {
                    let ch = n.children().len();
                    write!(f, "document(children={})", ch)
                }
                NodeKind::Element => {
                    let name = n
                        .name()
                        .map(|q| qname_to_string(&q))
                        .unwrap_or_else(|| "<unnamed>".to_string());
                    let attrs = n.attributes().len();
                    let ch = n.children().len();
                    write!(f, "<{} attrs={} children={}>", name, attrs, ch)
                }
                NodeKind::Attribute => {
                    let name = n
                        .name()
                        .map(|q| qname_to_string(&q))
                        .unwrap_or_else(|| "?".to_string());
                    let val = clip(&n.string_value());
                    write!(f, "@{}=\"{}\"", name, val)
                }
                NodeKind::Text => {
                    let val = clip(&n.string_value());
                    write!(f, "\"{}\"", val)
                }
                NodeKind::Comment => {
                    let val = clip(&n.string_value());
                    write!(f, "<!--{}-->", val)
                }
                NodeKind::ProcessingInstruction => {
                    let target = n.name().map(|q| q.local).unwrap_or_else(|| "".to_string());
                    let data = clip(&n.string_value());
                    if target.is_empty() {
                        write!(f, "<?{}?>", data)
                    } else {
                        write!(f, "<?{} {}?>", target, data)
                    }
                }
                NodeKind::Namespace => {
                    let prefix = n.name().and_then(|q| q.prefix).unwrap_or_default();
                    let uri = n.string_value();
                    if prefix.is_empty() {
                        write!(f, "xmlns=\"{}\"", uri)
                    } else {
                        write!(f, "xmlns:{}=\"{}\"", prefix, uri)
                    }
                }
            },
        }
    }
}

/// Pretty printer for a sequence using the XdmNode trait (no need for node Display).
pub struct PrettyNodeSeq<'a, N>(pub &'a [XdmItem<N>]);

impl<'a, N> PrettyNodeSeq<'a, N> {
    pub fn new(slice: &'a [XdmItem<N>]) -> Self {
        PrettyNodeSeq(slice)
    }
}

impl<'a, N: XdmNode> fmt::Display for PrettyNodeSeq<'a, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for (i, item) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", PrettyNodeItem(item))?;
        }
        write!(f, "]")
    }
}
