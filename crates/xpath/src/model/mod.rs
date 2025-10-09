use crate::engine::runtime::{Error, ErrorCode};
use crate::xdm::XdmAtomicValue;

pub mod simple;
use core::{cmp::Ordering, marker::PhantomData};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeKind {
    Document,
    Element,
    Attribute,
    Text,
    Comment,
    ProcessingInstruction,
    Namespace,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QName {
    pub prefix: Option<String>,
    pub local: String,
    pub ns_uri: Option<String>,
}

/// Compare two nodes by ancestry and stable sibling order (fallback algorithm).
///
/// Rules:
/// - If one node is ancestor of the other, the ancestor precedes the descendant.
/// - Among siblings, attributes (then namespaces) precede child nodes; within each group
///   retain the order provided by the adapter.
/// - Fallback comparator for document order based on ancestry and
///   stable sibling ordering.
///
/// Properties:
/// - If one node is an ancestor of the other, the ancestor precedes the descendant.
/// - Among siblings, attributes come first, then namespaces, then child nodes; within
///   each group the order provided by the adapter is preserved.
/// - If the nodes belong to different roots, returns an error (`err:FOER0000`) because
///   the fallback cannot establish a global order. Adapters with multi-root trees must
///   override `XdmNode::compare_document_order` and provide a total order
///   (e.g. `(tree_id, preorder_index)`).
pub fn try_compare_by_ancestry<N: XdmNode>(a: &N, b: &N) -> Result<Ordering, Error> {
    if a == b {
        return Ok(Ordering::Equal);
    }
    // Build paths from root to the node (inclusive)
    fn path_to_root<N: XdmNode>(mut n: N) -> Vec<N> {
        let mut p = vec![n.clone()];
        while let Some(parent) = n.parent() {
            p.push(parent.clone());
            n = parent;
        }
        p.reverse();
        p
    }
    let pa = path_to_root(a.clone());
    let pb = path_to_root(b.clone());
    let mut i = 0usize;
    let len = core::cmp::min(pa.len(), pb.len());
    while i < len && pa[i] == pb[i] {
        i += 1;
    }
    // One path is a prefix of the other → ancestor check
    if i == len {
        // shorter path is ancestor
        return Ok(if pa.len() < pb.len() { Ordering::Less } else { Ordering::Greater });
    }
    // Diverged at index i.
    if i == 0 {
        // Different roots. Default fallback cannot establish global order.
        return Err(Error::from_code(
            ErrorCode::FOER0000,
            "document order requires adapter: nodes from different roots",
        ));
    }
    // Compare the next nodes under the same parent (i-1)
    let parent = &pa[i - 1];
    // Sibling order: attributes, namespaces, then children
    let mut sibs: Vec<N> = Vec::new();
    sibs.extend(parent.attributes());
    sibs.extend(parent.namespaces());
    sibs.extend(parent.children());
    let na = &pa[i];
    let nb = &pb[i];
    let posa = sibs.iter().position(|n| n == na);
    let posb = sibs.iter().position(|n| n == nb);
    Ok(match (posa, posb) {
        (Some(aidx), Some(bidx)) => aidx.cmp(&bidx),
        // Fallback: if one is the parent itself (shouldn't happen here), treat parent before child
        _ => Ordering::Equal,
    })
}

#[derive(Clone, Debug, Default)]
pub struct EmptyAxis<N> {
    _marker: PhantomData<N>,
}

impl<N> Iterator for EmptyAxis<N> {
    type Item = N;

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

// Single-threaded evaluation: `EmptyAxis` does not need Send/Sync.

pub trait XdmNode: Clone + Eq + core::fmt::Debug {
    type Children<'a>: Iterator<Item = Self> + 'a
    where
        Self: 'a;
    type Attributes<'a>: Iterator<Item = Self> + 'a
    where
        Self: 'a;
    type Namespaces<'a>: Iterator<Item = Self> + 'a
    where
        Self: 'a;

    fn kind(&self) -> NodeKind;
    fn name(&self) -> Option<QName>;
    fn typed_value(&self) -> Vec<XdmAtomicValue>;

    fn string_value(&self) -> String {
        typed_value_to_string(&self.typed_value())
    }
    fn base_uri(&self) -> Option<String> {
        None
    }

    fn parent(&self) -> Option<Self>;
    fn children(&self) -> Self::Children<'_>;
    fn attributes(&self) -> Self::Attributes<'_>;
    fn namespaces(&self) -> Self::Namespaces<'_>;

    /// Optional hint for document order comparisons. If provided, the engine uses this
    /// value to avoid recomputing ancestry during ordering operations.
    fn doc_order_key(&self) -> Option<u64> {
        None
    }

    /// Default document order comparison uses ancestry and sibling order.
    /// Returns an error for multi-root comparisons unless overridden by adapter.
    fn compare_document_order(&self, other: &Self) -> Result<Ordering, Error> {
        try_compare_by_ancestry(self, other)
    }
}

fn typed_value_to_string(values: &[XdmAtomicValue]) -> String {
    if values.is_empty() {
        return String::new();
    }

    values.iter().map(lexical_cast).collect::<Vec<_>>().join(" ")
}

fn lexical_cast(value: &XdmAtomicValue) -> String {
    use XdmAtomicValue::*;
    match value {
        Boolean(b) => b.to_string(),
        String(s) | UntypedAtomic(s) | AnyUri(s) | NormalizedString(s) | Token(s) | Language(s) | Name(s)
        | NCName(s) | NMTOKEN(s) | Id(s) | IdRef(s) | Entity(s) | Notation(s) => s.clone(),
        Integer(i) => i.to_string(),
        Long(i) => i.to_string(),
        NonPositiveInteger(i) => i.to_string(),
        NegativeInteger(i) => i.to_string(),
        Int(i) => i.to_string(),
        Short(i) => i.to_string(),
        Byte(i) => i.to_string(),
        UnsignedLong(u) => u.to_string(),
        NonNegativeInteger(u) => u.to_string(),
        PositiveInteger(u) => u.to_string(),
        UnsignedInt(u) => u.to_string(),
        UnsignedShort(u) => u.to_string(),
        UnsignedByte(u) => u.to_string(),
        Decimal(d) | Double(d) => trim_float(*d),
        Float(f) => trim_float(*f as f64),
        QName { ns_uri, prefix, local } => match (ns_uri, prefix) {
            (Some(ns), Some(pref)) => format!("{{{}}}{}:{}", ns, pref, local),
            (Some(ns), None) => format!("{{{}}}{}", ns, local),
            (None, Some(pref)) => format!("{}:{}", pref, local),
            (None, None) => local.clone(),
        },
        DateTime(dt) => dt.to_rfc3339(),
        Date { date, tz } => match tz {
            Some(offset) => format!("{}{}", date, offset),
            None => date.to_string(),
        },
        Time { time, tz } => match tz {
            Some(offset) => format!("{}{}", time, offset),
            None => time.to_string(),
        },
        YearMonthDuration(months) => format!("P{}M", months),
        DayTimeDuration(secs) => format!("PT{}S", secs),
        Base64Binary(data) | HexBinary(data) => data.clone(),
        GYear { year, tz } => match tz {
            Some(offset) => format!("{}{}", year, offset),
            None => year.to_string(),
        },
        GYearMonth { year, month, tz } => match tz {
            Some(offset) => format!("{}-{:02}{}", year, month, offset),
            None => format!("{}-{:02}", year, month),
        },
        GMonth { month, tz } => match tz {
            Some(offset) => format!("{:02}{}", month, offset),
            None => format!("{:02}", month),
        },
        GMonthDay { month, day, tz } => match tz {
            Some(offset) => format!("{:02}-{:02}{}", month, day, offset),
            None => format!("{:02}-{:02}", month, day),
        },
        GDay { day, tz } => match tz {
            Some(offset) => format!("{:02}{}", day, offset),
            None => format!("{:02}", day),
        },
    }
}

fn trim_float(value: f64) -> String {
    let mut s = format!("{}", value);
    if s.contains('e') || s.contains('E') {
        return s;
    }
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    s
}
