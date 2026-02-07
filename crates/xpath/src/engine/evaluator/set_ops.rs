//! Set operations and document-order utilities for XPath evaluation.

use core::cmp::Ordering;

use smallvec::SmallVec;
use std::collections::HashSet;

use crate::engine::runtime::{Error, ErrorCode};
use crate::model::XdmNode;
use crate::xdm::{XdmItem, XdmSequence, XdmSequenceStream};

use super::Vm;

impl<N: 'static + XdmNode + Clone> Vm<N> {
    pub(crate) fn doc_order_only(&self, seq: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
        use crate::xdm::XdmItem;
        let mut keyed: SmallVec<[(u64, N); 16]> = SmallVec::new();
        let mut fallback: SmallVec<[N; 16]> = SmallVec::new();
        let mut others: Vec<XdmItem<N>> = Vec::new();

        for item in seq {
            match item {
                XdmItem::Node(n) => {
                    if let Some(k) = n.doc_order_key() {
                        keyed.push((k, n));
                    } else {
                        fallback.push(n);
                    }
                }
                other => others.push(other),
            }
        }

        if fallback.is_empty() {
            keyed.sort_by_key(|(k, _)| *k);
            let mut out = others;
            out.extend(keyed.into_iter().map(|(_, n)| XdmItem::Node(n)));
            return Ok(out);
        }
        if keyed.is_empty() {
            fallback.sort_by(|a, b| self.node_compare(a, b).unwrap_or(Ordering::Equal));
            let mut out = others;
            out.extend(fallback.into_iter().map(XdmItem::Node));
            return Ok(out);
        }

        keyed.sort_by_key(|(k, _)| *k);
        fallback.extend(keyed.into_iter().map(|(_, n)| n));
        fallback.sort_by(|a, b| self.node_compare(a, b).unwrap_or(Ordering::Equal));
        let mut out = others;
        out.extend(fallback.into_iter().map(XdmItem::Node));
        Ok(out)
    }

    /// Stream variant of union: consumes streams, collects nodes, then sorts/dedups.
    pub(crate) fn set_union_stream(&mut self, a: XdmSequenceStream<N>, b: XdmSequenceStream<N>) -> Result<XdmSequence<N>, Error> {
        // Pre-size using a conservative guess (streams may not expose exact len)
        let mut nodes: Vec<N> = Vec::new();

        // Helper to drain a stream into node vec or error on atomic
        let mut drain = |s: XdmSequenceStream<N>| -> Result<(), Error> {
            let mut c = s.cursor();
            while let Some(item) = c.next_item() {
                match item? {
                    XdmItem::Node(n) => nodes.push(n),
                    _ => {
                        return Err(Error::from_code(ErrorCode::XPTY0004, "union operator requires node sequences"));
                    }
                }
            }
            Ok(())
        };

        drain(a)?;
        drain(b)?;

        // Sort and dedup as in non-stream variant
        nodes.sort_by(|x, y| self.node_compare(x, y).unwrap_or(Ordering::Equal));
        nodes.dedup();
        Ok(nodes.into_iter().map(XdmItem::Node).collect())
    }

    /// Stream variant of intersect: consumes streams, sorts/dedups, then computes intersection.
    pub(crate) fn set_intersect_stream(
        &mut self,
        a: XdmSequenceStream<N>,
        b: XdmSequenceStream<N>,
    ) -> Result<XdmSequence<N>, Error> {
        let a_nodes = self.collect_nodes_from_stream(a)?;
        let lhs = self.sorted_distinct_nodes_vec(a_nodes)?;
        let b_nodes = self.collect_nodes_from_stream(b)?;
        let rhs = self.sorted_distinct_nodes_vec(b_nodes)?;

        let mut rhs_keys: HashSet<u64> = HashSet::with_capacity(rhs.len());
        let mut rhs_fallback = core::mem::take(&mut self.set_fallback);
        rhs_fallback.clear();
        for node in rhs {
            if let Some(k) = node.doc_order_key() {
                rhs_keys.insert(k);
            } else {
                rhs_fallback.push(node);
            }
        }
        let mut out: Vec<N> = Vec::with_capacity(lhs.len().min(rhs_keys.len() + rhs_fallback.len()));
        for node in lhs {
            if let Some(k) = node.doc_order_key() {
                if rhs_keys.contains(&k) {
                    out.push(node);
                }
            } else if rhs_fallback.iter().any(|n| n == &node) {
                out.push(node);
            }
        }
        let result = out.into_iter().map(XdmItem::Node).collect();
        rhs_fallback.clear();
        self.set_fallback = rhs_fallback;
        Ok(result)
    }

    /// Stream variant of except: consumes streams, sorts/dedups, then computes difference.
    pub(crate) fn set_except_stream(&mut self, a: XdmSequenceStream<N>, b: XdmSequenceStream<N>) -> Result<XdmSequence<N>, Error> {
        let a_nodes = self.collect_nodes_from_stream(a)?;
        let lhs = self.sorted_distinct_nodes_vec(a_nodes)?;
        let b_nodes = self.collect_nodes_from_stream(b)?;
        let rhs = self.sorted_distinct_nodes_vec(b_nodes)?;

        let mut rhs_keys: HashSet<u64> = HashSet::with_capacity(rhs.len());
        let mut rhs_fallback = core::mem::take(&mut self.set_fallback);
        rhs_fallback.clear();
        for node in rhs {
            if let Some(k) = node.doc_order_key() {
                rhs_keys.insert(k);
            } else {
                rhs_fallback.push(node);
            }
        }
        let mut out: Vec<N> = Vec::with_capacity(lhs.len());
        for node in lhs {
            if let Some(k) = node.doc_order_key() {
                if !rhs_keys.contains(&k) {
                    out.push(node);
                }
            } else if !rhs_fallback.iter().any(|n| n == &node) {
                out.push(node);
            }
        }
        let result = out.into_iter().map(XdmItem::Node).collect();
        rhs_fallback.clear();
        self.set_fallback = rhs_fallback;
        Ok(result)
    }

    /// Sort and deduplicate a homogeneous node vector using document order.
    pub(crate) fn sorted_distinct_nodes_vec(&self, nodes: Vec<N>) -> Result<Vec<N>, Error> {
        // Split keyed and fallback for efficiency
        let mut keyed: SmallVec<[(u64, N); 16]> = SmallVec::new();
        let mut fallback: SmallVec<[N; 16]> = SmallVec::new();
        for n in nodes.into_iter() {
            if let Some(k) = n.doc_order_key() {
                keyed.push((k, n));
            } else {
                fallback.push(n);
            }
        }
        if fallback.is_empty() {
            keyed.sort_by_key(|(k, _)| *k);
            keyed.dedup_by(|a, b| a.0 == b.0);
            return Ok(keyed.into_iter().map(|(_, n)| n).collect());
        }
        if keyed.is_empty() {
            fallback.sort_by(|a, b| self.node_compare(a, b).unwrap_or(Ordering::Equal));
            fallback.dedup();
            return Ok(fallback.into_vec());
        }
        keyed.sort_by_key(|(k, _)| *k);
        keyed.dedup_by(|a, b| a.0 == b.0);
        let mut merged: Vec<N> = Vec::with_capacity(fallback.len() + keyed.len());
        merged.extend(fallback);
        merged.extend(keyed.into_iter().map(|(_, n)| n));
        merged.sort_by(|a, b| self.node_compare(a, b).unwrap_or(Ordering::Equal));
        merged.dedup();
        Ok(merged)
    }

    /// Collect nodes from a stream, erroring on atomic items as set ops are nodes-only.
    pub(crate) fn collect_nodes_from_stream(&self, s: XdmSequenceStream<N>) -> Result<Vec<N>, Error> {
        let mut nodes: Vec<N> = Vec::new();
        let mut c = s.cursor();
        while let Some(item) = c.next_item() {
            match item? {
                XdmItem::Node(n) => nodes.push(n),
                _ => {
                    return Err(Error::from_code(ErrorCode::XPTY0004, "set operation requires node sequences"));
                }
            }
        }
        Ok(nodes)
    }

    pub(crate) fn node_compare(&self, a: &N, b: &N) -> Result<Ordering, Error> {
        match (a.doc_order_key(), b.doc_order_key()) {
            (Some(ak), Some(bk)) => Ok(ak.cmp(&bk)),
            _ => a.compare_document_order(b),
        }
    }
}
