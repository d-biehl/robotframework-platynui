//! Set operations for `XPath` evaluation: union, intersect and except over node sequences, in
//! document order and without duplicates.

use crate::engine::runtime::{Error, ErrorCode};
use crate::model::XdmNode;
use crate::xdm::{XdmItem, XdmSequence, XdmSequenceStream};

use super::Vm;
use super::order::{self, NodeSet};

impl<N: 'static + XdmNode + Clone> Vm<N> {
    /// Union: the nodes of both operands, each once, in document order.
    pub(crate) fn set_union_stream(
        a: &XdmSequenceStream<N>,
        b: &XdmSequenceStream<N>,
    ) -> Result<XdmSequence<N>, Error> {
        let mut nodes = Self::collect_nodes_from_stream(a)?;
        nodes.extend(Self::collect_nodes_from_stream(b)?);
        Ok(Self::sorted_distinct_nodes_vec(nodes).into_iter().map(XdmItem::Node).collect())
    }

    /// Intersect: the nodes of the left operand that the right one has too, in document order.
    pub(crate) fn set_intersect_stream(
        a: &XdmSequenceStream<N>,
        b: &XdmSequenceStream<N>,
    ) -> Result<XdmSequence<N>, Error> {
        let lhs = Self::sorted_distinct_nodes_vec(Self::collect_nodes_from_stream(a)?);
        let rhs = Self::node_set(&Self::collect_nodes_from_stream(b)?);
        Ok(lhs.into_iter().filter(|node| rhs.contains(node)).map(XdmItem::Node).collect())
    }

    /// Except: the nodes of the left operand that the right one lacks, in document order.
    pub(crate) fn set_except_stream(
        a: &XdmSequenceStream<N>,
        b: &XdmSequenceStream<N>,
    ) -> Result<XdmSequence<N>, Error> {
        let lhs = Self::sorted_distinct_nodes_vec(Self::collect_nodes_from_stream(a)?);
        let rhs = Self::node_set(&Self::collect_nodes_from_stream(b)?);
        Ok(lhs.into_iter().filter(|node| !rhs.contains(node)).map(XdmItem::Node).collect())
    }

    fn node_set(nodes: &[N]) -> NodeSet<N> {
        let mut set = NodeSet::default();
        for node in nodes {
            set.insert(node);
        }
        set
    }

    /// Sort and deduplicate a homogeneous node vector using document order.
    pub(crate) fn sorted_distinct_nodes_vec(nodes: Vec<N>) -> Vec<N> {
        let mut nodes = order::distinct_nodes(nodes);
        order::sort_nodes(&mut nodes);
        nodes
    }

    /// Collect nodes from a stream, erroring on atomic items as set ops are nodes-only.
    pub(crate) fn collect_nodes_from_stream(s: &XdmSequenceStream<N>) -> Result<Vec<N>, Error> {
        let mut nodes: Vec<N> = Vec::new();
        let mut c = s.cursor();
        while let Some(item) = c.next_item() {
            match item? {
                XdmItem::Node(n) => nodes.push(n),
                XdmItem::Atomic(_) => {
                    return Err(Error::from_code(ErrorCode::XPTY0004, "set operation requires node sequences"));
                }
            }
        }
        Ok(nodes)
    }
}
