//! Test utilities for extracting atomic values without relying on Debug formatting.
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};

pub fn as_bool<N>(it: &XdmItem<N>) -> Option<bool> {
    if let XdmItem::Atomic(XdmAtomicValue::Boolean(b)) = it { Some(*b) } else { None }
}
pub fn as_string<N>(it: &XdmItem<N>) -> Option<&str> {
    if let XdmItem::Atomic(XdmAtomicValue::String(s)) = it { Some(s) } else { None }
}
pub fn as_int<N>(it: &XdmItem<N>) -> Option<i64> {
    if let XdmItem::Atomic(XdmAtomicValue::Integer(i)) = it { Some(*i) } else { None }
}
pub fn as_double<N>(it: &XdmItem<N>) -> Option<f64> {
    if let XdmItem::Atomic(XdmAtomicValue::Double(d)) = it { Some(*d) } else { None }
}
