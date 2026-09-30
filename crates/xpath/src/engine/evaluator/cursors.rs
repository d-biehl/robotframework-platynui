//! Cursor types for streaming `XPath` evaluation.

use super::order::{self, DocOrder, NodeSet};
use super::{Frame, Vm, VmHandle};

use smallvec::SmallVec;
use std::collections::VecDeque;
use string_cache::DefaultAtom;

use crate::compiler::ir::{AxisIR, ComparisonOp, InstrSeq, NameOrWildcard, NodeTestIR, OpCode, QuantifierKind};
use crate::engine::runtime::{Error, ErrorCode};
use crate::model::{NodeKind, QName, XdmNode};
use crate::xdm::{Before, ExpandedName, SequenceCursor, XdmAtomicValue, XdmItem, XdmItemResult, XdmSequenceStream};

pub(super) struct AxisStepCursor<N> {
    vm: VmHandle<N>,
    axis: AxisIR,
    test: NodeTestIR,
    input_cursor: Box<dyn SequenceCursor<N>>,
    // Stream of results for the current input node (axis evaluation)
    current_output: Option<Box<dyn SequenceCursor<N>>>,
    // The step's predicates when one of them may count positions: they then run once per context
    // node, over that node's axis stream. Empty otherwise; the VM filters the whole stream.
    per_context: Vec<InstrSeq>,
    // A context node a bounded advance pulled and left, because it does not precede the bound.
    pending_context: Option<N>,
    // Compares context nodes with a bound; made on the first bounded advance that needs it.
    order: Option<DocOrder<N>>,
}

impl<N: 'static + XdmNode + Clone> AxisStepCursor<N> {
    pub(super) fn new(
        vm: VmHandle<N>,
        input: &XdmSequenceStream<N>,
        axis: AxisIR,
        test: NodeTestIR,
        per_context: Vec<InstrSeq>,
    ) -> Self {
        let base_cursor = input.cursor();
        // Minimizing drops a context node whose results another context already covers, which only
        // holds when no predicate counts positions: with one, every context counts on its own.
        let input_cursor: Box<dyn SequenceCursor<N>> = match axis {
            _ if !per_context.is_empty() => base_cursor,
            AxisIR::Descendant | AxisIR::DescendantOrSelf => Box::new(ContextMinCursor::new(base_cursor)),
            AxisIR::Following => Box::new(ContextMinFollowingCursor::new(base_cursor)),
            AxisIR::FollowingSibling => Box::new(ContextMinFollowingSiblingCursor::new(base_cursor)),
            AxisIR::Preceding => Box::new(LastContextCursor::new(base_cursor)),
            _ => base_cursor,
        };
        Self { vm, axis, test, input_cursor, current_output: None, per_context, pending_context: None, order: None }
    }

    /// The axis of one context node, with the step's per-context predicates on it.
    fn output_for(&self, node: N) -> Box<dyn SequenceCursor<N>> {
        let axis_cursor: Box<dyn SequenceCursor<N>> =
            Box::new(NodeAxisCursor::new(self.vm.clone(), node, self.axis.clone(), self.test.clone()));
        self.per_context.iter().fold(axis_cursor, |input, predicate| {
            Box::new(PredicateCursor::new(self.vm.clone(), predicate.clone(), input))
        })
    }
}

// A streaming cursor that evaluates a single axis/test against one context node
struct NodeAxisCursor<N> {
    vm: VmHandle<N>,
    axis: AxisIR,
    test: NodeTestIR,
    // The context node this axis is applied to
    node: N,
    // Internal state machine
    state: AxisState<N>,
    // A candidate a bounded advance looked at and left, because it does not precede the bound.
    pending: Option<N>,
    // Compares candidates with a bound; made on the first bounded advance.
    order: Option<DocOrder<N>>,
}

enum AxisState<N> {
    // Uninitialized; will be set to a more specific variant on first next_item
    Init,
    // Emit self once (with test)
    SelfOnce {
        emitted: bool,
    },
    // Stream child:: axis without pre-buffering
    ChildIter {
        current: Option<N>,
        initialized: bool,
    },
    // attribute:: axis — exact-name lookup (single result via attribute_by_name)
    AttributeDirect {
        result: Option<N>,
        emitted: bool,
    },
    // attribute:: axis — lazy streaming for wildcard / multi-match tests
    AttributeIter {
        current: Option<N>,
        initialized: bool,
    },
    // Depth-first traversal for descendant/descendant-or-self using document-order successors.
    // `last` holds the last emitted node in pre-order; the next candidate is its
    // doc_successor *within* the anchor's subtree. Stops lazily when the walk-up
    // reaches the anchor — no eager full-subtree traversal needed.
    Descend {
        anchor: N,
        last: Option<N>,
        include_self: bool,
        started: bool,
    },
    // Parent/ancestor chains
    Parent {
        done: bool,
    },
    Ancestors {
        current: Option<N>,
        include_self: bool,
    },
    // Sibling scans (streaming)
    FollowingSiblingIter {
        current: Option<N>,
        initialized: bool,
    },
    PrecedingSiblingIter {
        current: Option<N>,
        initialized: bool,
    },
    // Following/Preceding document order
    Following {
        anchor: Option<N>,
        next: Option<N>,
        initialized: bool,
    },
    Preceding {
        // path from root to context (inclusive) to filter ancestors
        path: SmallVec<[N; 16]>,
        current: Option<N>,
        initialized: bool,
    },
    // Namespace axis
    Namespaces {
        seen: SmallVec<[DefaultAtom; 8]>,
        current: Option<N>,
        buf: SmallVec<[N; 8]>,
        idx: usize,
    },
}

impl<N: 'static + XdmNode + Clone> NodeAxisCursor<N> {
    fn new(vm: VmHandle<N>, node: N, axis: AxisIR, test: NodeTestIR) -> Self {
        Self { vm, axis, test, node, state: AxisState::Init, pending: None, order: None }
    }

    /// The next candidate: the one a bounded advance left, or the axis's next.
    fn take_candidate(&mut self) -> Result<Option<N>, Error> {
        match self.pending.take() {
            Some(candidate) => Ok(Some(candidate)),
            None => self.next_candidate(),
        }
    }

    #[inline]
    fn is_attr_or_namespace(node: &N) -> bool {
        matches!(node.kind(), NodeKind::Attribute | NodeKind::Namespace)
    }

    fn init_state(&mut self) {
        self.state = match self.axis {
            AxisIR::SelfAxis => AxisState::SelfOnce { emitted: false },
            // Stream children lazily to avoid building large buffers
            AxisIR::Child => AxisState::ChildIter { current: None, initialized: false },
            AxisIR::Attribute => {
                // Decide at init time whether this is an exact-name or wildcard test.
                let exact_name = match &self.test {
                    NodeTestIR::Name(q)
                    | NodeTestIR::KindAttribute { name: Some(NameOrWildcard::Name(q)), ty: None } => Some(&q.original),
                    _ => None,
                };
                if let Some(en) = exact_name {
                    let lookup = QName { prefix: None, local: en.local.clone(), ns_uri: en.ns_uri.clone() };
                    let result = self.node.attribute_by_name(&lookup);
                    AxisState::AttributeDirect { result, emitted: false }
                } else {
                    AxisState::AttributeIter { current: None, initialized: false }
                }
            }
            // replaced later by AttributeQueue in init_state refactor
            AxisIR::Parent => AxisState::Parent { done: false },
            AxisIR::Ancestor => AxisState::Ancestors { current: self.node.parent(), include_self: false },
            AxisIR::AncestorOrSelf => AxisState::Ancestors { current: Some(self.node.clone()), include_self: true },
            AxisIR::Descendant => {
                AxisState::Descend { anchor: self.node.clone(), last: None, include_self: false, started: false }
            }
            AxisIR::DescendantOrSelf => {
                AxisState::Descend { anchor: self.node.clone(), last: None, include_self: true, started: false }
            }
            AxisIR::FollowingSibling => AxisState::FollowingSiblingIter { current: None, initialized: false },
            AxisIR::PrecedingSibling => AxisState::PrecedingSiblingIter { current: None, initialized: false },
            AxisIR::Following => AxisState::Following { anchor: None, next: None, initialized: false },
            AxisIR::Preceding => {
                let path = Self::path_to_root(self.node.clone());
                AxisState::Preceding { path, current: None, initialized: false }
            }
            AxisIR::Namespace => {
                let cur = if matches!(self.node.kind(), NodeKind::Element) { Some(self.node.clone()) } else { None };
                AxisState::Namespaces { seen: SmallVec::new(), current: cur, buf: SmallVec::new(), idx: 0 }
            }
        };
    }

    // One state machine over all axes; splitting it would scatter the per-axis state handling.
    #[allow(clippy::too_many_lines)]
    fn next_candidate(&mut self) -> Result<Option<N>, Error> {
        if matches!(self.state, AxisState::Init) {
            self.init_state();
        }
        match &mut self.state {
            AxisState::SelfOnce { emitted } => {
                if *emitted {
                    return Ok(None);
                }
                *emitted = true;
                Ok(Some(self.node.clone()))
            }
            AxisState::ChildIter { current, initialized } => {
                if !*initialized {
                    *current = Self::first_child_in_doc(&self.node);
                    *initialized = true;
                }
                if let Some(cur) = current.take() {
                    // Pre-compute next for subsequent call
                    *current = Self::next_sibling_in_doc(&cur);
                    Ok(Some(cur))
                } else {
                    Ok(None)
                }
            }
            // Exact-name attribute: emit the single result from attribute_by_name
            AxisState::AttributeDirect { result, emitted } => {
                if *emitted {
                    return Ok(None);
                }
                *emitted = true;
                Ok(result.take())
            }
            // Wildcard / multi-match attribute: lazy streaming via
            // first_attribute / next_attribute_in_doc helpers.  The underlying
            // RuntimeXdmNode caches attributes internally, so the O(N²)
            // equality scans only hit in-memory Vec lookups, not COM calls.
            AxisState::AttributeIter { current, initialized } => {
                if !*initialized {
                    *initialized = true;
                    *current = Self::first_attribute(&self.node);
                    // fast-skip non-matching attributes for common tests
                    while let Some(cur) = current.as_ref() {
                        match Self::attribute_test_fast_match_static(&self.test, cur) {
                            Some(true) | None => break,
                            Some(false) => {
                                let next = Self::next_attribute_in_doc(&self.node, cur);
                                *current = next;
                            }
                        }
                    }
                }
                if let Some(cur) = current.take() {
                    // Pre-compute next matching attribute
                    let mut next = Self::next_attribute_in_doc(&self.node, &cur);
                    while let Some(ref c2) = next {
                        match Self::attribute_test_fast_match_static(&self.test, c2) {
                            Some(true) | None => break,
                            Some(false) => {
                                next = Self::next_attribute_in_doc(&self.node, c2);
                            }
                        }
                    }
                    *current = next;
                    Ok(Some(cur))
                } else {
                    Ok(None)
                }
            }
            // no generic buffer state anymore
            AxisState::Parent { done } => {
                if *done {
                    return Ok(None);
                }
                *done = true;
                Ok(self.node.parent())
            }
            AxisState::Ancestors { current, include_self } => {
                if let Some(cur) = current.take() {
                    let parent = cur.parent();
                    let is_self = !*include_self && cur == self.node;
                    *current = parent;
                    if is_self {
                        return self.next_candidate();
                    }
                    Ok(Some(cur))
                } else {
                    Ok(None)
                }
            }
            AxisState::Descend { anchor, last, include_self, started } => {
                if !*started {
                    *started = true;
                    if *include_self {
                        let n = self.node.clone();
                        *last = Some(n.clone());
                        return Ok(Some(n));
                    }
                    // Start with the first child in pre-order
                    if let Some(first) = Self::first_child_in_doc(&self.node) {
                        *last = Some(first.clone());
                        return Ok(Some(first));
                    }
                    return Ok(None);
                }
                // Advance to the next document-order successor *within* the
                // anchor's subtree. Stops when the walk-up reaches the anchor.
                if let Some(prev) = last.take() {
                    if let Some(succ) = Self::doc_successor_within(&prev, anchor) {
                        *last = Some(succ.clone());
                        return Ok(Some(succ));
                    }
                    Ok(None)
                } else {
                    Ok(None)
                }
            }
            AxisState::FollowingSiblingIter { current, initialized } => {
                if !*initialized {
                    *current = Self::next_sibling_in_doc(&self.node);
                    *initialized = true;
                }
                if let Some(cur) = current.take() {
                    let next = Self::next_sibling_in_doc(&cur);
                    *current = next;
                    Ok(Some(cur))
                } else {
                    Ok(None)
                }
            }
            AxisState::PrecedingSiblingIter { current, initialized } => {
                if !*initialized {
                    *current = Self::prev_sibling_in_doc(&self.node);
                    *initialized = true;
                }
                if let Some(cur) = current.take() {
                    let next = Self::prev_sibling_in_doc(&cur);
                    *current = next;
                    Ok(Some(cur))
                } else {
                    Ok(None)
                }
            }
            AxisState::Following { anchor, next, initialized } => {
                if !*initialized {
                    *initialized = true;
                    // First node after the context node's subtree: walk up
                    // ancestors until one has a following sibling — avoids
                    // eagerly traversing the entire subtree.
                    *next = Self::first_node_after_subtree(&self.node);
                    anchor.clone_from(next);
                }
                while let Some(n) = next.take() {
                    *anchor = Some(n.clone());
                    *next = Self::doc_successor(&n);
                    if !Self::is_attr_or_namespace(&n) {
                        return Ok(Some(n));
                    }
                    // continue loop to skip attr/ns without recursion
                }
                Ok(None)
            }
            AxisState::Preceding { path, current, initialized } => {
                if !*initialized {
                    *initialized = true;
                    *current = Self::doc_predecessor(&self.node);
                }
                while let Some(cur) = current.take() {
                    // advance for next call now
                    let next_prev = Self::doc_predecessor(&cur);
                    *current = next_prev;
                    // Skip attributes/namespaces and ancestors of the context node
                    if Self::is_attr_or_namespace(&cur) {
                        continue;
                    }
                    // Is ancestor? compare against path (which includes context at the end)
                    let mut is_ancestor = false;
                    for a in path.iter() {
                        if &cur == a {
                            is_ancestor = true;
                            break;
                        }
                    }
                    if is_ancestor {
                        continue;
                    }
                    return Ok(Some(cur));
                }
                Ok(None)
            }
            AxisState::Namespaces { seen, current, buf, idx } => {
                // if buffer has items, return them
                if *idx < buf.len() {
                    let n = buf[*idx].clone();
                    *idx += 1;
                    return Ok(Some(n));
                }
                // Refill buffer from current element, then advance to parent
                while let Some(cur) = current.take() {
                    if matches!(cur.kind(), NodeKind::Element) {
                        buf.clear();
                        *idx = 0;
                        for ns in cur.namespaces() {
                            if let Some(q) = ns.name() {
                                let atom = DefaultAtom::from(q.prefix.unwrap_or_default().as_str());
                                if !seen.iter().any(|a| a == &atom) {
                                    seen.push(atom);
                                    buf.push(ns.clone());
                                }
                            }
                        }
                        *current = cur.parent();
                        if !buf.is_empty() {
                            let n = buf[*idx].clone();
                            *idx += 1;
                            return Ok(Some(n));
                        }
                        continue;
                    }
                    *current = cur.parent();
                }
                Ok(None)
            }
            AxisState::Init => unreachable!("axis cursor used before initialization"),
        }
    }

    fn matches_test(&self, node: &N) -> Result<bool, Error> {
        // Fast paths for common patterns to skip VM roundtrip
        match (&self.axis, &self.test) {
            // node() matches any node kind
            (_, NodeTestIR::AnyKind) => return Ok(true),
            // `*` matches the principal node kind of the axis: attribute:: matches attributes,
            // namespace:: matches namespace nodes, every other axis matches elements only (not
            // document nodes). element() with no constraints also matches elements only.
            (
                AxisIR::Child
                | AxisIR::DescendantOrSelf
                | AxisIR::SelfAxis
                | AxisIR::Ancestor
                | AxisIR::AncestorOrSelf
                | AxisIR::Descendant
                | AxisIR::Following
                | AxisIR::Preceding
                | AxisIR::FollowingSibling
                | AxisIR::PrecedingSibling
                | AxisIR::Parent,
                NodeTestIR::WildcardAny,
            )
            | (_, NodeTestIR::KindElement { name: None, ty: None, nillable: false }) => {
                return Ok(matches!(node.kind(), NodeKind::Element));
            }
            // attribute::* or attribute() with no constraints
            (AxisIR::Attribute, NodeTestIR::WildcardAny) | (_, NodeTestIR::KindAttribute { name: None, ty: None }) => {
                return Ok(matches!(node.kind(), NodeKind::Attribute));
            }
            (AxisIR::Namespace, NodeTestIR::WildcardAny) => {
                return Ok(matches!(node.kind(), NodeKind::Namespace));
            }
            // text(), comment(), processing-instruction()
            (_, NodeTestIR::KindText) => return Ok(matches!(node.kind(), NodeKind::Text)),
            (_, NodeTestIR::KindComment) => return Ok(matches!(node.kind(), NodeKind::Comment)),
            (_, NodeTestIR::KindProcessingInstruction(None)) => {
                return Ok(matches!(node.kind(), NodeKind::ProcessingInstruction));
            }
            (_, NodeTestIR::KindProcessingInstruction(Some(target))) => {
                if !matches!(node.kind(), NodeKind::ProcessingInstruction) {
                    return Ok(false);
                }
                let n = node.name();
                return Ok(n.as_ref().is_some_and(|q| &q.local == target));
            }
            // QName and namespace wildcards require effective-namespace resolution
            // Delegate to the full resolver to honor prefix/default namespace semantics and namespace-axis rules.
            // Going through `with_vm` keeps its cancellation check.
            (_, NodeTestIR::Name(_)) => {
                return self.vm.with_vm(|_| Ok(Vm::node_test(node, &self.test)));
            }
            (_, NodeTestIR::NsWildcard(_)) => {
                return self.vm.with_vm(|_| Ok(Vm::node_test(node, &self.test)));
            }
            (_, NodeTestIR::LocalWildcard(_)) => {
                return self.vm.with_vm(|_| Ok(Vm::node_test(node, &self.test)));
            }
            _ => {}
        }
        // Fallback: use the full resolver to ensure correct namespace handling. Going through
        // `with_vm` keeps its cancellation check.
        self.vm.with_vm(|_| Ok(Vm::node_test(node, &self.test)))
    }
}

// Filters items to drop context nodes that are descendants of the last kept node.
// Assumes (as typically produced by the compiler) document‑ordered input;
// otherwise correctness is preserved but fewer duplicates may be removed up front
// (EnsureDistinct handles the remainder).
struct ContextMinCursor<N> {
    inner: Box<dyn SequenceCursor<N>>,
    last_kept: Option<N>,
}

impl<N> ContextMinCursor<N> {
    fn new(inner: Box<dyn SequenceCursor<N>>) -> Self {
        Self { inner, last_kept: None }
    }
}

impl<N: XdmNode + Clone + 'static> ContextMinCursor<N> {
    fn is_descendant_of(node: &N, ancestor: &N) -> bool {
        let mut cur = node.parent();
        let mut guard = 0usize;
        while let Some(p) = cur {
            if &p == ancestor {
                return true;
            }
            let next = p.parent();
            // Cycle guards: parent() returns self or path too deep
            if next.as_ref().is_some_and(|q| q == &p) {
                break;
            }
            cur = next;
            guard = guard.saturating_add(1);
            if guard > 1_000_000 {
                break;
            }
        }
        false
    }
}

impl<N: XdmNode + Clone + 'static> ContextMinCursor<N> {
    /// Whether `node` is covered by the last kept context; otherwise it becomes the kept one.
    fn covered(&mut self, node: &N) -> bool {
        if let Some(last) = &self.last_kept
            && Self::is_descendant_of(node, last)
        {
            return true;
        }
        self.last_kept = Some(node.clone());
        false
    }
}

impl<N: XdmNode + Clone + 'static> SequenceCursor<N> for ContextMinCursor<N> {
    fn next_item_before(&mut self, bound: &N) -> Before<N> {
        loop {
            match self.inner.next_item_before(bound) {
                Before::Taken(Ok(XdmItem::Node(node))) => {
                    if !self.covered(&node) {
                        return Before::Taken(Ok(XdmItem::Node(node)));
                    }
                }
                Before::Pulled(Ok(XdmItem::Node(node))) => {
                    if !self.covered(&node) {
                        return Before::Pulled(Ok(XdmItem::Node(node)));
                    }
                }
                other => return other,
            }
        }
    }

    fn next_item(&mut self) -> Option<XdmItemResult<N>> {
        loop {
            let item = self.inner.next_item()?;
            match item {
                Ok(XdmItem::Node(n)) => {
                    if let Some(last) = &self.last_kept
                        && Self::is_descendant_of(&n, last)
                    {
                        continue; // skip overlapping context
                    }
                    self.last_kept = Some(n.clone());
                    return Some(Ok(XdmItem::Node(n)));
                }
                other => return Some(other),
            }
        }
    }

    fn boxed_clone(&self) -> Box<dyn SequenceCursor<N>> {
        Box::new(Self { inner: self.inner.boxed_clone(), last_kept: self.last_kept.clone() })
    }
}

/// Minimizes the context nodes of `following::` to one: the context whose subtree ends first, as
/// its following nodes include those of every other context. Over input in document order, a later
/// context inside the kept one ends no later and replaces it; the first context outside it ends
/// later, as does every context after that one, so the scan stops there.
struct ContextMinFollowingCursor<N> {
    inner: Option<Box<dyn SequenceCursor<N>>>,
}

impl<N> ContextMinFollowingCursor<N> {
    fn new(inner: Box<dyn SequenceCursor<N>>) -> Self {
        Self { inner: Some(inner) }
    }
}

impl<N: XdmNode + Clone + 'static> SequenceCursor<N> for ContextMinFollowingCursor<N> {
    fn next_item(&mut self) -> Option<XdmItemResult<N>> {
        let mut inner = self.inner.take()?;
        let mut kept: Option<N> = None;
        while let Some(item) = inner.next_item() {
            match item {
                Ok(XdmItem::Node(node)) => match &kept {
                    Some(earlier) if !ContextMinCursor::is_descendant_of(&node, earlier) => break,
                    _ => kept = Some(node),
                },
                Ok(_) => {}
                Err(err) => return Some(Err(err)),
            }
        }
        kept.map(|node| Ok(XdmItem::Node(node)))
    }

    fn boxed_clone(&self) -> Box<dyn SequenceCursor<N>> {
        Box::new(Self { inner: self.inner.as_ref().map(|inner| inner.boxed_clone()) })
    }
}

/// Keeps only the last context node, for `preceding::` without positional predicates: over input
/// in document order, the preceding nodes of the last one include those of every other.
struct LastContextCursor<N> {
    inner: Option<Box<dyn SequenceCursor<N>>>,
}

impl<N> LastContextCursor<N> {
    fn new(inner: Box<dyn SequenceCursor<N>>) -> Self {
        Self { inner: Some(inner) }
    }
}

impl<N: XdmNode + Clone + 'static> SequenceCursor<N> for LastContextCursor<N> {
    fn next_item(&mut self) -> Option<XdmItemResult<N>> {
        let mut inner = self.inner.take()?;
        let mut last = None;
        while let Some(item) = inner.next_item() {
            match item {
                Ok(item) => last = Some(item),
                Err(err) => return Some(Err(err)),
            }
        }
        last.map(Ok)
    }

    fn boxed_clone(&self) -> Box<dyn SequenceCursor<N>> {
        Box::new(Self { inner: self.inner.as_ref().map(|inner| inner.boxed_clone()) })
    }
}

// Minimize contexts for following-sibling:: by keeping only the leftmost sibling per parent.
struct ContextMinFollowingSiblingCursor<N> {
    inner: Box<dyn SequenceCursor<N>>,
    leftmost: SmallVec<[(N, N); 8]>, // (parent, leftmost_child_seen)
}

impl<N> ContextMinFollowingSiblingCursor<N> {
    fn new(inner: Box<dyn SequenceCursor<N>>) -> Self {
        Self { inner, leftmost: SmallVec::new() }
    }
}

impl<N: XdmNode + Clone + 'static> SequenceCursor<N> for ContextMinFollowingSiblingCursor<N> {
    fn next_item(&mut self) -> Option<XdmItemResult<N>> {
        'outer: loop {
            let item = self.inner.next_item()?;
            match item {
                Ok(XdmItem::Node(n)) => {
                    if let Some(parent) = n.parent() {
                        if let Some((_, left)) = self.leftmost.iter().find(|(p, _)| *p == parent) {
                            // If `n` is after `left` among siblings → redundant
                            let mut seen_left = false;
                            for s in parent.children() {
                                if s == *left {
                                    seen_left = true;
                                    continue;
                                }
                                if seen_left && s == n {
                                    // n is after left → drop
                                    continue 'outer;
                                }
                            }
                            // If we got here, either n is before left (unsorted) → keep
                            return Some(Ok(XdmItem::Node(n)));
                        }
                        self.leftmost.push((parent, n.clone()));
                        return Some(Ok(XdmItem::Node(n)));
                    }
                    // No parent? Not a normal element; just pass through
                    return Some(Ok(XdmItem::Node(n)));
                }
                other => return Some(other),
            }
        }
    }

    fn boxed_clone(&self) -> Box<dyn SequenceCursor<N>> {
        Box::new(Self { inner: self.inner.boxed_clone(), leftmost: self.leftmost.clone() })
    }
}

impl<N: 'static + XdmNode + Clone> SequenceCursor<N> for NodeAxisCursor<N> {
    fn next_item(&mut self) -> Option<XdmItemResult<N>> {
        loop {
            let cand = match self.take_candidate() {
                Ok(opt) => opt,
                Err(err) => return Some(Err(err)),
            }?;
            match self.matches_test(&cand) {
                Ok(true) => return Some(Ok(XdmItem::Node(cand))),
                Ok(false) => {}
                Err(err) => return Some(Err(err)),
            }
        }
    }

    /// A forward axis walks in document order, so it stops at the first candidate that does not
    /// precede the bound: every list it read belongs to a node before it. A reverse axis runs the
    /// other way and pulls.
    fn next_item_before(&mut self, bound: &N) -> Before<N> {
        if matches!(
            self.axis,
            AxisIR::Parent
                | AxisIR::Ancestor
                | AxisIR::AncestorOrSelf
                | AxisIR::Preceding
                | AxisIR::PrecedingSibling
                | AxisIR::Namespace
        ) {
            return match self.next_item() {
                Some(item) => Before::Pulled(item),
                None => Before::End,
            };
        }
        loop {
            let cand = match self.take_candidate() {
                Ok(Some(cand)) => cand,
                Ok(None) => return Before::End,
                Err(err) => return Before::Taken(Err(err)),
            };
            if !self.order.get_or_insert_with(DocOrder::default).precedes(&cand, bound) {
                self.pending = Some(cand);
                return Before::NotBefore;
            }
            match self.matches_test(&cand) {
                Ok(true) => return Before::Taken(Ok(XdmItem::Node(cand))),
                Ok(false) => {}
                Err(err) => return Before::Taken(Err(err)),
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }

    fn boxed_clone(&self) -> Box<dyn SequenceCursor<N>> {
        Box::new(Self {
            vm: self.vm.clone(),
            axis: self.axis.clone(),
            test: self.test.clone(),
            node: self.node.clone(),
            state: AxisState::Init, // fresh cursor
            pending: None,
            order: None,
        })
    }
}

impl<N: 'static + XdmNode + Clone> NodeAxisCursor<N> {
    fn path_to_root(n: N) -> SmallVec<[N; 16]> {
        let mut p: SmallVec<[N; 16]> = SmallVec::new();
        let mut cur: Option<N> = Some(n);
        while let Some(x) = cur {
            p.push(x.clone());
            cur = x.parent();
        }
        p.reverse();
        p
    }

    fn first_child_in_doc(node: &N) -> Option<N> {
        node.children().find(|c| !Self::is_attr_or_namespace(c))
    }
    // attribute helpers for lazy streaming
    fn first_attribute(node: &N) -> Option<N> {
        node.attributes().next()
    }
    fn next_attribute_in_doc(parent: &N, prev: &N) -> Option<N> {
        let mut seen = false;
        for a in parent.attributes() {
            if seen {
                return Some(a);
            }
            if &a == prev {
                seen = true;
            }
        }
        None
    }
    fn next_sibling_in_doc(node: &N) -> Option<N> {
        let parent = node.parent()?;
        let mut seen = false;
        for s in parent.children() {
            if seen && !Self::is_attr_or_namespace(&s) {
                return Some(s);
            }
            if s == *node {
                seen = true;
            }
        }
        None
    }
    fn last_descendant_in_doc(mut node: N) -> N {
        loop {
            let mut last: Option<N> = None;
            for c in node.children() {
                if !Self::is_attr_or_namespace(&c) {
                    last = Some(c);
                }
            }
            if let Some(n) = last {
                node = n;
            } else {
                return node;
            }
        }
    }
    fn doc_successor(node: &N) -> Option<N> {
        if let Some(c) = Self::first_child_in_doc(node) {
            return Some(c);
        }
        let mut cur = node.clone();
        while let Some(p) = cur.parent() {
            if let Some(sib) = Self::next_sibling_in_doc(&cur) {
                return Some(sib);
            }
            cur = p;
        }
        None
    }
    /// Like [`Self::doc_successor`] but confined to the subtree rooted at
    /// `anchor`. Returns `None` when the walk-up reaches `anchor`, avoiding
    /// an eager full-subtree traversal to precompute a boundary node.
    fn doc_successor_within(node: &N, anchor: &N) -> Option<N> {
        if let Some(c) = Self::first_child_in_doc(node) {
            return Some(c);
        }
        let mut cur = node.clone();
        loop {
            if cur == *anchor {
                return None;
            }
            if let Some(sib) = Self::next_sibling_in_doc(&cur) {
                return Some(sib);
            }
            cur = cur.parent()?;
        }
    }
    /// Return the first node in document order that follows the entire
    /// subtree rooted at `node`. Equivalent to
    /// `doc_successor(last_descendant(node))` but without traversing the
    /// subtree — walks ancestors until one has a following sibling.
    fn first_node_after_subtree(node: &N) -> Option<N> {
        let mut cur = node.clone();
        loop {
            if let Some(sib) = Self::next_sibling_in_doc(&cur) {
                return Some(sib);
            }
            cur = cur.parent()?;
        }
    }
    fn prev_sibling_in_doc(node: &N) -> Option<N> {
        let parent = node.parent()?;
        let mut prev: Option<N> = None;
        for s in parent.children() {
            if s == *node {
                break;
            }
            if !Self::is_attr_or_namespace(&s) {
                prev = Some(s);
            }
        }
        prev
    }
    fn doc_predecessor(node: &N) -> Option<N> {
        // Predecessor in doc order (elements only for axes that use it):
        // 1) If there is a preceding sibling element, take its last descendant; else parent.
        if let Some(prev_sib) = Self::prev_sibling_in_doc(node) {
            return Some(Self::last_descendant_in_doc(prev_sib));
        }
        node.parent()
    }
    // Static variant that receives the test explicitly to avoid borrowing self in loops
    fn attribute_test_fast_match_static(test: &NodeTestIR, attr: &N) -> Option<bool> {
        use NodeTestIR as NT;
        if !matches!(attr.kind(), NodeKind::Attribute) {
            return Some(false);
        }
        match test {
            NT::WildcardAny | NT::KindAttribute { name: None | Some(NameOrWildcard::Any), ty: None } => Some(true),
            NT::KindAttribute { name: Some(NameOrWildcard::Name(q)), ty: None } | NT::Name(q) => {
                let n = attr.name()?;
                let matches_local = n.local == q.original.local;
                let matches_ns = match (&n.ns_uri, &q.original.ns_uri) {
                    (None, None) => true,
                    (Some(a), Some(b)) => a == b,
                    _ => false,
                };
                Some(matches_local && matches_ns)
            }
            NT::NsWildcard(ns) => {
                let n = attr.name()?;
                Some(n.ns_uri.as_deref().is_some_and(|u| u == ns.as_str()))
            }
            NT::LocalWildcard(local) => {
                let n = attr.name()?;
                Some(n.local == local.as_str())
            }
            // Typed attribute tests and every other test: no fast answer.
            _ => None,
        }
    }
}

impl<N: 'static + XdmNode + Clone> SequenceCursor<N> for AxisStepCursor<N> {
    fn next_item(&mut self) -> Option<XdmItemResult<N>> {
        loop {
            if let Some(ref mut current) = self.current_output {
                if let Some(item) = current.next_item() {
                    return Some(item);
                }
                self.current_output = None;
            }
            // Pull next context item
            let candidate = match self.pending_context.take() {
                Some(node) => XdmItem::Node(node),
                None => match self.input_cursor.next_item()? {
                    Ok(item) => item,
                    Err(err) => return Some(Err(err)),
                },
            };
            let XdmItem::Node(node) = candidate else { continue };
            self.current_output = Some(self.output_for(node));
        }
    }

    /// Every item of a context node's axis here comes after the node itself or is the node, and
    /// the context nodes come in document order, so a context node that does not precede the bound
    /// ends the search.
    fn next_item_before(&mut self, bound: &N) -> Before<N> {
        loop {
            if let Some(current) = self.current_output.as_mut() {
                match current.next_item_before(bound) {
                    Before::End => self.current_output = None,
                    found => return found,
                }
                continue;
            }
            let next = match self.pending_context.take() {
                Some(node) => Before::Pulled(Ok(XdmItem::Node(node))),
                None => self.input_cursor.next_item_before(bound),
            };
            let node = match next {
                Before::Taken(Ok(XdmItem::Node(node))) => node,
                Before::Pulled(Ok(XdmItem::Node(node))) => {
                    if !self.order.get_or_insert_with(DocOrder::default).precedes(&node, bound) {
                        self.pending_context = Some(node);
                        return Before::NotBefore;
                    }
                    node
                }
                Before::Taken(Ok(_)) | Before::Pulled(Ok(_)) => continue,
                Before::Taken(Err(err)) | Before::Pulled(Err(err)) => return Before::Taken(Err(err)),
                Before::NotBefore => return Before::NotBefore,
                Before::End => return Before::End,
            };
            self.current_output = Some(self.output_for(node));
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }

    fn boxed_clone(&self) -> Box<dyn SequenceCursor<N>> {
        Box::new(Self {
            vm: self.vm.clone(),
            axis: self.axis.clone(),
            test: self.test.clone(),
            input_cursor: self.input_cursor.boxed_clone(),
            current_output: self.current_output.as_ref().map(|c| c.boxed_clone()),
            per_context: self.per_context.clone(),
            pending_context: self.pending_context.clone(),
            order: None,
        })
    }
}

/// A child step over context nodes in document order, some of which may lie inside others: it
/// emits the selected children of all of them in document order, as it goes.
///
/// It keeps a stack of the context nodes it entered, each inside the one below it, with the next
/// selected child of each, its head. The top head comes first among everything entered, and it is
/// emitted once no context node still to come can precede it: such a node would lie in the subtree
/// of one of the head's preceding siblings, and a bounded advance of the input finds it without
/// reading past the head. The step's predicates run per context node. Over a single context node,
/// or context nodes none of which contains another, the stack holds one frame at a time and the
/// merge is a plain concatenation.
pub(super) struct ChildMergeCursor<N> {
    vm: VmHandle<N>,
    test: NodeTestIR,
    predicates: Vec<InstrSeq>,
    input: Box<dyn SequenceCursor<N>>,
    // A context node pulled from the input but not entered yet.
    pending: Option<N>,
    input_done: bool,
    stack: Vec<MergeFrame<N>>,
    order: DocOrder<N>,
}

struct MergeFrame<N> {
    children: Box<dyn SequenceCursor<N>>,
    head: Option<N>,
}

impl<N: 'static + XdmNode + Clone> ChildMergeCursor<N> {
    pub(super) fn new(
        vm: VmHandle<N>,
        input: &XdmSequenceStream<N>,
        test: NodeTestIR,
        predicates: Vec<InstrSeq>,
    ) -> Self {
        Self {
            vm,
            test,
            predicates,
            input: input.cursor(),
            pending: None,
            input_done: false,
            stack: Vec::new(),
            order: DocOrder::default(),
        }
    }

    fn enter(&mut self, context: N) {
        let axis: Box<dyn SequenceCursor<N>> =
            Box::new(NodeAxisCursor::new(self.vm.clone(), context, AxisIR::Child, self.test.clone()));
        let children = self
            .predicates
            .iter()
            .fold(axis, |input, predicate| Box::new(PredicateCursor::new(self.vm.clone(), predicate.clone(), input)));
        self.stack.push(MergeFrame { children, head: None });
    }

    /// The next context node, when it precedes `limit` (any, without a limit).
    fn next_context(&mut self, limit: Option<&N>) -> Result<Option<N>, Error> {
        loop {
            let found = if let Some(node) = self.pending.take() {
                Before::Pulled(Ok(XdmItem::Node(node)))
            } else if self.input_done {
                return Ok(None);
            } else if let Some(limit) = limit {
                self.input.next_item_before(limit)
            } else {
                match self.input.next_item() {
                    Some(item) => Before::Pulled(item),
                    None => Before::End,
                }
            };
            match found {
                Before::Taken(Ok(XdmItem::Node(node))) => return Ok(Some(node)),
                Before::Pulled(Ok(XdmItem::Node(node))) => {
                    if let Some(limit) = limit
                        && !self.order.precedes(&node, limit)
                    {
                        self.pending = Some(node);
                        return Ok(None);
                    }
                    return Ok(Some(node));
                }
                // Only nodes have children.
                Before::Taken(Ok(_)) | Before::Pulled(Ok(_)) => {}
                Before::Taken(Err(err)) | Before::Pulled(Err(err)) => return Err(err),
                Before::NotBefore => return Ok(None),
                Before::End => {
                    self.input_done = true;
                    return Ok(None);
                }
            }
        }
    }

    /// The next item, or with a bound the next item if it precedes the bound.
    fn advance(&mut self, bound: Option<&N>) -> Before<N> {
        loop {
            let Some(frame) = self.stack.last_mut() else {
                match self.next_context(bound) {
                    Ok(Some(context)) => {
                        self.enter(context);
                        continue;
                    }
                    Ok(None) if self.input_done && self.pending.is_none() => return Before::End,
                    Ok(None) => return Before::NotBefore,
                    Err(err) => return Before::Taken(Err(err)),
                }
            };
            if frame.head.is_none() {
                match frame.children.next_item() {
                    Some(Ok(XdmItem::Node(child))) => frame.head = Some(child),
                    Some(Ok(_)) => continue,
                    Some(Err(err)) => return Before::Taken(Err(err)),
                    None => {
                        self.stack.pop();
                        continue;
                    }
                }
            }
            let Some(head) = frame.head.clone() else { continue };
            // A context node still to come that precedes the head (or the bound, if that comes
            // first) goes first: its children lie before the head.
            let limit = match bound {
                Some(bound) if self.order.precedes(bound, &head) => bound.clone(),
                _ => head.clone(),
            };
            match self.next_context(Some(&limit)) {
                Ok(Some(context)) => {
                    self.enter(context);
                    continue;
                }
                Ok(None) => {}
                Err(err) => return Before::Taken(Err(err)),
            }
            if let Some(bound) = bound
                && !self.order.precedes(&head, bound)
            {
                return Before::NotBefore;
            }
            if let Some(frame) = self.stack.last_mut() {
                frame.head = None;
            }
            return Before::Taken(Ok(XdmItem::Node(head)));
        }
    }
}

impl<N: 'static + XdmNode + Clone> SequenceCursor<N> for ChildMergeCursor<N> {
    fn next_item(&mut self) -> Option<XdmItemResult<N>> {
        match self.advance(None) {
            Before::Taken(item) | Before::Pulled(item) => Some(item),
            Before::NotBefore | Before::End => None,
        }
    }

    fn next_item_before(&mut self, bound: &N) -> Before<N> {
        self.advance(Some(bound))
    }

    fn boxed_clone(&self) -> Box<dyn SequenceCursor<N>> {
        Box::new(Self {
            vm: self.vm.clone(),
            test: self.test.clone(),
            predicates: self.predicates.clone(),
            input: self.input.boxed_clone(),
            pending: self.pending.clone(),
            input_done: self.input_done,
            stack: self
                .stack
                .iter()
                .map(|frame| MergeFrame { children: frame.children.boxed_clone(), head: frame.head.clone() })
                .collect(),
            order: DocOrder::default(),
        })
    }
}

pub(super) struct PredicateCursor<N> {
    vm: VmHandle<N>,
    predicate: InstrSeq,
    input: Box<dyn SequenceCursor<N>>,
    seed: Option<Box<dyn SequenceCursor<N>>>,
    position: usize,
    last_cache: Option<usize>,
    needs_last: bool,
    fast_kind: PredicateFastKind,
}

impl<N: 'static + XdmNode + Clone> PredicateCursor<N> {
    pub(super) fn new(vm: VmHandle<N>, predicate: InstrSeq, input: Box<dyn SequenceCursor<N>>) -> Self {
        let seed = Some(input.boxed_clone());
        let needs_last = instr_seq_uses_last(&predicate);
        let fast_kind = classify_predicate_fast(&predicate);
        Self { vm, predicate, input, seed, position: 0, last_cache: None, needs_last, fast_kind }
    }

    /// Whether no later item can match: `[1]`, `[k]` or `[position() <= k]` once `k` items were
    /// counted. The cursor ends then, before it reads another item from its input.
    fn exhausted(&self) -> bool {
        match self.fast_kind {
            PredicateFastKind::First => self.position >= 1,
            PredicateFastKind::Exact(k) | PredicateFastKind::PositionLe(k) => self.position >= k,
            PredicateFastKind::None | PredicateFastKind::PositionGe(_) => false,
        }
    }

    fn ensure_last(&mut self) -> Result<usize, Error> {
        if let Some(last) = self.last_cache {
            return Ok(last);
        }
        if !self.needs_last {
            // Predicate does not use last(); avoid expensive full pre-scan.
            self.last_cache = Some(0);
            return Ok(0);
        }
        let mut cursor = if let Some(seed) = self.seed.take() { seed } else { self.input.boxed_clone() };
        let mut count = 0usize;
        while let Some(item) = cursor.next_item() {
            match item {
                Ok(_) => count = count.saturating_add(1),
                Err(err) => return Err(err),
            }
        }
        let total = self.position.saturating_add(count);
        self.last_cache = Some(total);
        Ok(total)
    }

    fn evaluate_predicate(&self, item: &XdmItem<N>, pos: usize, last: usize) -> Result<bool, Error> {
        // Fast path evaluation for simple positional predicates
        match self.fast_kind {
            PredicateFastKind::First => return Ok(pos == 1),
            PredicateFastKind::Exact(k) => return Ok(pos == k),
            PredicateFastKind::PositionLe(k) => return Ok(pos <= k),
            PredicateFastKind::PositionGe(k) => return Ok(pos >= k),
            PredicateFastKind::None => {}
        }
        self.vm.with_vm(|vm| {
            let stream =
                vm.eval_subprogram_stream(&self.predicate, Some(item.clone()), Some(Frame { last, pos }), None)?;
            Vm::predicate_truth_value_stream(&stream, pos, last)
        })
    }
}

impl<N: 'static + XdmNode + Clone> SequenceCursor<N> for PredicateCursor<N> {
    fn next_item(&mut self) -> Option<XdmItemResult<N>> {
        // For fast positional predicates we never need last() unless original code truly referenced it.
        let last = if matches!(self.fast_kind, PredicateFastKind::None) || self.needs_last {
            match self.ensure_last() {
                Ok(v) => v,
                Err(err) => return Some(Err(err)),
            }
        } else {
            0
        };

        while !self.exhausted() {
            match self.input.next_item()? {
                Ok(item) => {
                    let pos = self.position + 1;
                    self.position = pos;
                    match self.evaluate_predicate(&item, pos, last) {
                        Ok(true) => return Some(Ok(item)),
                        Ok(false) => {}
                        Err(err) => return Some(Err(err)),
                    }
                }
                Err(err) => return Some(Err(err)),
            }
        }
        None
    }

    /// Counts only the items it takes, so a candidate left pending keeps its position for later. A
    /// predicate that reads `last()` has to count its whole input first, and pulls.
    fn next_item_before(&mut self, bound: &N) -> Before<N> {
        if self.needs_last {
            return match self.next_item() {
                Some(item) => Before::Pulled(item),
                None => Before::End,
            };
        }
        while !self.exhausted() {
            let (item, taken) = match self.input.next_item_before(bound) {
                Before::Taken(Ok(item)) => (item, true),
                Before::Pulled(Ok(item)) => (item, false),
                Before::Taken(Err(err)) | Before::Pulled(Err(err)) => return Before::Taken(Err(err)),
                Before::NotBefore => return Before::NotBefore,
                Before::End => return Before::End,
            };
            let pos = self.position + 1;
            self.position = pos;
            match self.evaluate_predicate(&item, pos, 0) {
                Ok(true) if taken => return Before::Taken(Ok(item)),
                Ok(true) => return Before::Pulled(Ok(item)),
                Ok(false) => {}
                Err(err) => return Before::Taken(Err(err)),
            }
        }
        Before::End
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let upper = self.last_cache.map(|last| last.saturating_sub(self.position));
        (0, upper)
    }

    fn boxed_clone(&self) -> Box<dyn SequenceCursor<N>> {
        Box::new(Self {
            vm: self.vm.clone(),
            predicate: self.predicate.clone(),
            input: self.input.boxed_clone(),
            seed: self.seed.as_ref().map(|cursor| cursor.boxed_clone()),
            position: self.position,
            last_cache: self.last_cache,
            needs_last: self.needs_last,
            fast_kind: self.fast_kind,
        })
    }
}

// Classification of simple positional predicate patterns to skip full VM evaluation.
#[derive(Copy, Clone, Debug)]
enum PredicateFastKind {
    None,
    First,             // [1] or position()=1
    Exact(usize),      // [K] or position()=K
    PositionLe(usize), // position() <= K or position() < K+1 (common in slices)
    PositionGe(usize), // position() >= K or position() > K-1
}

fn classify_predicate_fast(code: &InstrSeq) -> PredicateFastKind {
    use OpCode::{CompareGeneral, CompareValue, Position, PushAtomic};
    // Pattern: [K]  -> single PushAtomic numeric literal
    if code.0.len() == 1
        && let PushAtomic(ref av) = code.0[0]
        && let Some(k) = atomic_to_usize(av)
    {
        return if k == 1 { PredicateFastKind::First } else { PredicateFastKind::Exact(k) };
    }
    // Patterns: position() (=|<=) K   (CompareValue / CompareGeneral)
    if code.0.len() <= 3 {
        let mut saw_position = false;
        let mut number: Option<usize> = None;
        let mut cmp: Option<ComparisonOp> = None;
        for op in &code.0 {
            match op {
                Position => saw_position = true,
                PushAtomic(av) if number.is_none() => {
                    number = atomic_to_usize(av);
                }
                CompareValue(c) | CompareGeneral(c) if cmp.is_none() => {
                    cmp = Some(*c);
                }
                _ => {}
            }
        }
        if saw_position && let Some(k) = number {
            if let Some(c) = cmp {
                match c {
                    ComparisonOp::Eq => {
                        return if k == 1 { PredicateFastKind::First } else { PredicateFastKind::Exact(k) };
                    }
                    ComparisonOp::Le => return PredicateFastKind::PositionLe(k),
                    ComparisonOp::Lt if k > 1 => return PredicateFastKind::PositionLe(k - 1),
                    ComparisonOp::Ge => return PredicateFastKind::PositionGe(k),
                    ComparisonOp::Gt => return PredicateFastKind::PositionGe(k + 1),
                    _ => {}
                }
            } else if k == 1 {
                // Degenerate form
                return PredicateFastKind::First;
            }
        }
    }
    PredicateFastKind::None
}

// The guards ensure a positive (and, for floating point, integral) value; the `as` casts keep
// their truncating/saturating behaviour for values beyond `usize`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn atomic_to_usize(av: &XdmAtomicValue) -> Option<usize> {
    match av {
        XdmAtomicValue::Integer(i) | XdmAtomicValue::Long(i) if *i >= 1 => Some(*i as usize),
        XdmAtomicValue::Int(i) if *i >= 1 => Some(*i as usize),
        XdmAtomicValue::UnsignedInt(u) if *u >= 1 => Some(*u as usize),
        XdmAtomicValue::UnsignedLong(u) if *u >= 1 => usize::try_from(*u).ok(),
        XdmAtomicValue::Double(d) if *d >= 1.0 && d.fract() == 0.0 => Some(*d as usize),
        XdmAtomicValue::Decimal(d) if *d >= rust_decimal::Decimal::ONE && d.fract().is_zero() => {
            use rust_decimal::prelude::ToPrimitive;
            d.to_usize()
        }
        XdmAtomicValue::Float(f) if *f >= 1.0 && f64::from(*f).fract() == 0.0 => Some(*f as usize),
        _ => None,
    }
}

// Cheap static analysis: does the predicate program reference `last()`?
pub(super) fn instr_seq_uses_last(code: &InstrSeq) -> bool {
    use OpCode::{ApplyPredicates, AxisStep, ForLoop, Last, PathExprStep, QuantLoop};
    for op in &code.0 {
        match op {
            Last => return true,
            // Recurse into nested sequences where predicates might hide
            PathExprStep(inner) if instr_seq_uses_last(inner) => {
                return true;
            }
            ApplyPredicates(preds) | AxisStep(_, _, preds) => {
                for p in preds {
                    if instr_seq_uses_last(&p.code) {
                        return true;
                    }
                }
            }
            ForLoop { body, .. } if instr_seq_uses_last(body) => {
                return true;
            }
            QuantLoop { body, .. } if instr_seq_uses_last(body) => {
                return true;
            }
            _ => {}
        }
    }
    false
}

pub(super) struct PathStepCursor<N> {
    vm: VmHandle<N>,
    code: InstrSeq,
    input: Box<dyn SequenceCursor<N>>,
    seed: Option<Box<dyn SequenceCursor<N>>>,
    input_len: Option<usize>,
    position: usize,
    current_output: Option<Box<dyn SequenceCursor<N>>>,
    needs_last: bool,
}

impl<N: 'static + XdmNode + Clone> PathStepCursor<N> {
    pub(super) fn new(vm: VmHandle<N>, input_stream: &XdmSequenceStream<N>, code: InstrSeq) -> Self {
        let input = input_stream.cursor();
        let seed = Some(input.boxed_clone());
        let needs_last = instr_seq_uses_last(&code);
        Self { vm, code, input, seed, input_len: None, position: 0, current_output: None, needs_last }
    }

    fn ensure_input_len(&mut self) -> Result<usize, Error> {
        if let Some(len) = self.input_len {
            return Ok(len);
        }
        let mut cursor = if let Some(seed) = self.seed.take() { seed } else { self.input.boxed_clone() };
        let mut count = 0usize;
        while let Some(item) = cursor.next_item() {
            match item {
                Ok(_) => count = count.saturating_add(1),
                Err(err) => return Err(err),
            }
        }
        let total = self.position.saturating_add(count);
        self.input_len = Some(total);
        Ok(total)
    }
}

impl<N: 'static + XdmNode + Clone> SequenceCursor<N> for PathStepCursor<N> {
    fn next_item(&mut self) -> Option<XdmItemResult<N>> {
        loop {
            if let Some(ref mut current) = self.current_output {
                if let Some(item) = current.next_item() {
                    return Some(item);
                }
                self.current_output = None;
            }

            let candidate = match self.input.next_item()? {
                Ok(item) => item,
                Err(err) => return Some(Err(err)),
            };

            let last = if self.needs_last {
                match self.ensure_input_len() {
                    Ok(v) => v,
                    Err(err) => return Some(Err(err)),
                }
            } else {
                0
            };
            let pos = self.position + 1;
            self.position = pos;

            let stream = match self.vm.with_vm(|vm| {
                vm.eval_subprogram_stream(&self.code, Some(candidate.clone()), Some(Frame { last, pos }), None)
            }) {
                Ok(stream) => stream,
                Err(err) => return Some(Err(err)),
            };

            // Stream the subprogram output directly; if it happens to be empty, next loop will pull next input item.
            self.current_output = Some(stream.cursor());
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }

    fn boxed_clone(&self) -> Box<dyn SequenceCursor<N>> {
        Box::new(Self {
            vm: self.vm.clone(),
            code: self.code.clone(),
            input: self.input.boxed_clone(),
            seed: self.seed.as_ref().map(|cursor| cursor.boxed_clone()),
            input_len: self.input_len,
            position: self.position,
            current_output: self.current_output.as_ref().map(|cursor| cursor.boxed_clone()),
            needs_last: self.needs_last,
        })
    }
}

// (set operations handled via dedicated opcodes; no SetOpKind needed)

pub(super) struct ForLoopCursor<N> {
    vm: VmHandle<N>,
    var: ExpandedName,
    body: InstrSeq,
    input: Box<dyn SequenceCursor<N>>,
    seed: Option<Box<dyn SequenceCursor<N>>>,
    input_len: Option<usize>,
    position: usize,
    current_output: Option<Box<dyn SequenceCursor<N>>>,
    needs_last: bool,
}

impl<N: 'static + XdmNode + Clone> ForLoopCursor<N> {
    pub(super) fn new(vm: VmHandle<N>, input_stream: &XdmSequenceStream<N>, var: ExpandedName, body: InstrSeq) -> Self {
        let input = input_stream.cursor();
        let seed = Some(input.boxed_clone());
        let needs_last = instr_seq_uses_last(&body);
        Self { vm, var, body, input, seed, input_len: None, position: 0, current_output: None, needs_last }
    }

    fn ensure_input_len(&mut self) -> Result<usize, Error> {
        if let Some(len) = self.input_len {
            return Ok(len);
        }
        let total = if let Some(seed) = self.seed.as_ref() {
            let (lower, upper) = seed.size_hint();
            let total = if let Some(upper) = upper
                && lower == upper
            {
                upper
            } else {
                let mut cursor = self
                    .seed
                    .take()
                    .ok_or_else(|| Error::from_code(ErrorCode::FOER0000, "for-loop length seed missing"))?;
                let mut count = 0usize;
                while let Some(item) = cursor.next_item() {
                    match item {
                        Ok(_) => count = count.saturating_add(1),
                        Err(err) => return Err(err),
                    }
                }
                count
            };
            self.seed = None;
            total
        } else {
            let (lower, upper) = self.input.size_hint();
            if let Some(upper) = upper
                && lower == upper
            {
                self.position + 1 + upper
            } else {
                let mut cursor = self.input.boxed_clone();
                let mut remaining = 0usize;
                while let Some(item) = cursor.next_item() {
                    match item {
                        Ok(_) => remaining = remaining.saturating_add(1),
                        Err(err) => return Err(err),
                    }
                }
                self.position + 1 + remaining
            }
        };
        self.input_len = Some(total);
        Ok(total)
    }
}

impl<N: 'static + XdmNode + Clone> SequenceCursor<N> for ForLoopCursor<N> {
    fn next_item(&mut self) -> Option<XdmItemResult<N>> {
        loop {
            if self.vm.is_cancelled() {
                return Some(Err(Error::from_code(ErrorCode::FOER0000, "evaluation cancelled")));
            }

            if let Some(ref mut current) = self.current_output {
                if let Some(item) = current.next_item() {
                    return Some(item);
                }
                self.current_output = None;
            }

            let candidate = match self.input.next_item()? {
                Ok(item) => item,
                Err(err) => return Some(Err(err)),
            };

            let last = if self.needs_last {
                match self.ensure_input_len() {
                    Ok(v) => v,
                    Err(err) => return Some(Err(err)),
                }
            } else {
                0
            };

            let pos = self.position + 1;
            self.position = pos;

            let context_item = candidate.clone();
            let binding_stream = XdmSequenceStream::from_vec(vec![candidate]);
            let stream = match self.vm.with_vm(|vm| {
                vm.eval_subprogram_stream(
                    &self.body,
                    Some(context_item),
                    Some(Frame { last, pos }),
                    Some((self.var.clone(), binding_stream)),
                )
            }) {
                Ok(stream) => stream,
                Err(err) => return Some(Err(err)),
            };
            self.current_output = Some(stream.cursor());
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }

    fn boxed_clone(&self) -> Box<dyn SequenceCursor<N>> {
        Box::new(Self {
            vm: self.vm.clone(),
            var: self.var.clone(),
            body: self.body.clone(),
            input: self.input.boxed_clone(),
            seed: self.seed.as_ref().map(|cursor| cursor.boxed_clone()),
            input_len: self.input_len,
            position: self.position,
            current_output: self.current_output.as_ref().map(|cursor| cursor.boxed_clone()),
            needs_last: self.needs_last,
        })
    }
}

pub(super) struct QuantLoopCursor<N> {
    vm: VmHandle<N>,
    kind: QuantifierKind,
    var: ExpandedName,
    body: InstrSeq,
    input: Box<dyn SequenceCursor<N>>,
    seed: Option<Box<dyn SequenceCursor<N>>>,
    input_len: Option<usize>,
    position: usize,
    result: Option<bool>,
    emitted: bool,
    needs_last: bool,
}

impl<N: 'static + XdmNode + Clone> QuantLoopCursor<N> {
    pub(super) fn new(
        vm: VmHandle<N>,
        input_stream: &XdmSequenceStream<N>,
        kind: QuantifierKind,
        var: ExpandedName,
        body: InstrSeq,
    ) -> Self {
        let input = input_stream.cursor();
        let seed = Some(input.boxed_clone());
        let needs_last = instr_seq_uses_last(&body);
        Self {
            vm,
            kind,
            var,
            body,
            input,
            seed,
            input_len: None,
            position: 0,
            result: None,
            emitted: false,
            needs_last,
        }
    }

    fn ensure_input_len(&mut self) -> Result<usize, Error> {
        if let Some(len) = self.input_len {
            return Ok(len);
        }
        let total = if let Some(seed) = self.seed.as_ref() {
            let (lower, upper) = seed.size_hint();
            let total = if let Some(upper) = upper
                && lower == upper
            {
                upper
            } else {
                let mut cursor = self
                    .seed
                    .take()
                    .ok_or_else(|| Error::from_code(ErrorCode::FOER0000, "quant-loop length seed missing"))?;
                let mut count = 0usize;
                while let Some(item) = cursor.next_item() {
                    match item {
                        Ok(_) => count = count.saturating_add(1),
                        Err(err) => return Err(err),
                    }
                }
                count
            };
            self.seed = None;
            total
        } else {
            let (lower, upper) = self.input.size_hint();
            if let Some(upper) = upper
                && lower == upper
            {
                self.position + upper
            } else {
                let mut cursor = self.input.boxed_clone();
                let mut remaining = 0usize;
                while let Some(item) = cursor.next_item() {
                    match item {
                        Ok(_) => remaining = remaining.saturating_add(1),
                        Err(err) => return Err(err),
                    }
                }
                self.position + remaining
            }
        };
        self.input_len = Some(total);
        Ok(total)
    }

    fn evaluate(&mut self) -> Result<bool, Error> {
        if let Some(cached) = self.result {
            return Ok(cached);
        }
        let total = if self.needs_last { self.ensure_input_len()? } else { 0 };
        let mut quant_result = match self.kind {
            QuantifierKind::Some => false,
            QuantifierKind::Every => true,
        };

        while let Some(item) = self.input.next_item() {
            if self.vm.is_cancelled() {
                return Err(Error::from_code(ErrorCode::FOER0000, "evaluation cancelled"));
            }
            let candidate = item?;
            let pos = self.position + 1;
            self.position = pos;
            let context_item = candidate.clone();
            let binding_stream = XdmSequenceStream::from_item(candidate);
            let body_stream = self.vm.with_vm(|vm| {
                vm.eval_subprogram_stream(
                    &self.body,
                    Some(context_item),
                    Some(Frame { last: total, pos }),
                    Some((self.var.clone(), binding_stream)),
                )
            })?;
            // Stream EBV to avoid materialization
            let truth = crate::engine::ebv::ebv_of_stream(&mut *body_stream.cursor())?;
            match self.kind {
                QuantifierKind::Some => {
                    if truth {
                        quant_result = true;
                        break;
                    }
                }
                QuantifierKind::Every => {
                    if !truth {
                        quant_result = false;
                        break;
                    }
                }
            }
        }

        self.result = Some(quant_result);
        Ok(quant_result)
    }
}

impl<N: 'static + XdmNode + Clone> SequenceCursor<N> for QuantLoopCursor<N> {
    fn next_item(&mut self) -> Option<XdmItemResult<N>> {
        if self.emitted {
            return None;
        }
        match self.evaluate() {
            Ok(value) => {
                self.emitted = true;
                Some(Ok(XdmItem::Atomic(XdmAtomicValue::Boolean(value))))
            }
            Err(err) => {
                self.emitted = true;
                Some(Err(err))
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.emitted { (0, Some(0)) } else { (0, Some(1)) }
    }

    fn boxed_clone(&self) -> Box<dyn SequenceCursor<N>> {
        Box::new(Self {
            vm: self.vm.clone(),
            kind: self.kind,
            var: self.var.clone(),
            body: self.body.clone(),
            input: self.input.boxed_clone(),
            seed: self.seed.as_ref().map(|cursor| cursor.boxed_clone()),
            input_len: self.input_len,
            position: self.position,
            result: self.result,
            emitted: self.emitted,
            needs_last: self.needs_last,
        })
    }
}

pub(super) struct DistinctCursor<N> {
    vm: VmHandle<N>,
    input: Option<Box<dyn SequenceCursor<N>>>,
    // The nodes passed on so far, by identity.
    seen: NodeSet<N>,
}

impl<N: 'static + XdmNode + Clone> DistinctCursor<N> {
    pub(super) fn new(vm: VmHandle<N>, input: Box<dyn SequenceCursor<N>>) -> Self {
        Self { vm, input: Some(input), seen: NodeSet::default() }
    }
}

impl<N: 'static + XdmNode + Clone> SequenceCursor<N> for DistinctCursor<N> {
    fn next_item(&mut self) -> Option<XdmItemResult<N>> {
        use crate::xdm::XdmItem;
        let cursor = self.input.as_mut()?;
        loop {
            let item = cursor.next_item()?;
            match item {
                Ok(XdmItem::Node(n)) => {
                    if self.seen.insert(&n) {
                        return Some(Ok(XdmItem::Node(n)));
                    }
                }
                Ok(other) => return Some(Ok(other)),
                Err(e) => return Some(Err(e)),
            }
        }
    }

    fn next_item_before(&mut self, bound: &N) -> Before<N> {
        let Some(cursor) = self.input.as_mut() else { return Before::End };
        loop {
            match cursor.next_item_before(bound) {
                Before::Taken(Ok(XdmItem::Node(node))) => {
                    if self.seen.insert(&node) {
                        return Before::Taken(Ok(XdmItem::Node(node)));
                    }
                }
                Before::Pulled(Ok(XdmItem::Node(node))) => {
                    if self.seen.insert(&node) {
                        return Before::Pulled(Ok(XdmItem::Node(node)));
                    }
                }
                other => return other,
            }
        }
    }

    fn boxed_clone(&self) -> Box<dyn SequenceCursor<N>> {
        Box::new(Self {
            vm: self.vm.clone(),
            input: self.input.as_ref().map(|c| c.boxed_clone()),
            seen: self.seen.clone(),
        })
    }
}

/// Sorts its input into document order and removes duplicate nodes, for a stream whose order the
/// compiler cannot prove. It reads all of its input before its first item and checks for
/// cancellation while it does. Atomic values keep their order, ahead of the nodes.
pub(super) struct NormalizeCursor<N> {
    vm: VmHandle<N>,
    input: Option<Box<dyn SequenceCursor<N>>>,
    output: VecDeque<XdmItem<N>>,
}

impl<N: 'static + XdmNode + Clone> NormalizeCursor<N> {
    pub(super) fn new(vm: VmHandle<N>, input: Box<dyn SequenceCursor<N>>) -> Self {
        Self { vm, input: Some(input), output: VecDeque::new() }
    }

    fn drain(&mut self, mut input: Box<dyn SequenceCursor<N>>) -> Result<(), Error> {
        let mut atomics = Vec::new();
        let mut nodes = Vec::new();
        loop {
            self.vm.check_cancel()?;
            match input.next_item() {
                Some(Ok(XdmItem::Node(node))) => nodes.push(node),
                Some(Ok(atomic)) => atomics.push(atomic),
                Some(Err(err)) => return Err(err),
                None => break,
            }
        }
        let mut nodes = order::distinct_nodes(nodes);
        order::sort_nodes(&mut nodes);
        self.output = atomics.into_iter().chain(nodes.into_iter().map(XdmItem::Node)).collect();
        Ok(())
    }
}

impl<N: 'static + XdmNode + Clone> SequenceCursor<N> for NormalizeCursor<N> {
    fn next_item(&mut self) -> Option<XdmItemResult<N>> {
        if let Some(input) = self.input.take()
            && let Err(err) = self.drain(input)
        {
            return Some(Err(err));
        }
        self.output.pop_front().map(Ok)
    }

    fn boxed_clone(&self) -> Box<dyn SequenceCursor<N>> {
        Box::new(Self {
            vm: self.vm.clone(),
            input: self.input.as_ref().map(|cursor| cursor.boxed_clone()),
            output: self.output.clone(),
        })
    }
}

/// Reverses its input: the output of a reverse axis from one context node, nearest first, becomes
/// document order. It reads all of its input before its first item and checks for cancellation
/// while it does.
pub(super) struct ReverseCursor<N> {
    vm: VmHandle<N>,
    input: Option<Box<dyn SequenceCursor<N>>>,
    output: Vec<XdmItem<N>>,
}

impl<N: 'static + XdmNode + Clone> ReverseCursor<N> {
    pub(super) fn new(vm: VmHandle<N>, input: Box<dyn SequenceCursor<N>>) -> Self {
        Self { vm, input: Some(input), output: Vec::new() }
    }

    fn drain(&mut self, mut input: Box<dyn SequenceCursor<N>>) -> Result<(), Error> {
        loop {
            self.vm.check_cancel()?;
            match input.next_item() {
                Some(item) => self.output.push(item?),
                None => return Ok(()),
            }
        }
    }
}

impl<N: 'static + XdmNode + Clone> SequenceCursor<N> for ReverseCursor<N> {
    fn next_item(&mut self) -> Option<XdmItemResult<N>> {
        if let Some(input) = self.input.take()
            && let Err(err) = self.drain(input)
        {
            return Some(Err(err));
        }
        // Items were pushed in input order, so popping yields them reversed.
        self.output.pop().map(Ok)
    }

    fn boxed_clone(&self) -> Box<dyn SequenceCursor<N>> {
        Box::new(Self {
            vm: self.vm.clone(),
            input: self.input.as_ref().map(|cursor| cursor.boxed_clone()),
            output: self.output.clone(),
        })
    }
}

pub(super) struct AtomizeCursor<N> {
    input: Box<dyn SequenceCursor<N>>,
    pending: VecDeque<XdmAtomicValue>,
}

impl<N: 'static + XdmNode + Clone> AtomizeCursor<N> {
    pub(super) fn new(stream: &XdmSequenceStream<N>) -> Self {
        Self { input: stream.cursor(), pending: VecDeque::new() }
    }
}

impl<N: 'static + XdmNode + Clone> SequenceCursor<N> for AtomizeCursor<N> {
    fn next_item(&mut self) -> Option<XdmItemResult<N>> {
        use XdmItem::{Atomic, Node};
        if let Some(atom) = self.pending.pop_front() {
            return Some(Ok(Atomic(atom)));
        }
        loop {
            let item = self.input.next_item()?;
            match item {
                Ok(Atomic(a)) => return Some(Ok(Atomic(a))),
                Ok(Node(n)) => {
                    for a in n.typed_value() {
                        self.pending.push_back(a);
                    }
                    if let Some(atom) = self.pending.pop_front() {
                        return Some(Ok(Atomic(atom)));
                    }
                }
                Err(e) => return Some(Err(e)),
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }

    fn boxed_clone(&self) -> Box<dyn SequenceCursor<N>> {
        Box::new(Self { input: self.input.boxed_clone(), pending: self.pending.clone() })
    }
}

// Cursor that enforces treat as semantics while passing items through
pub(super) struct TreatCursor<N> {
    vm: VmHandle<N>,
    input: Box<dyn SequenceCursor<N>>,
    item_type: crate::compiler::ir::ItemTypeIR,
    min: usize,
    max: Option<usize>,
    seen: usize,
    pending_error: Option<Error>,
}

impl<N: 'static + XdmNode + Clone> TreatCursor<N> {
    pub(super) fn new(vm: VmHandle<N>, stream: &XdmSequenceStream<N>, t: crate::compiler::ir::SeqTypeIR) -> Self {
        use crate::compiler::ir::{OccurrenceIR, SeqTypeIR};
        let (min, max, item_type) = match t {
            SeqTypeIR::EmptySequence => (0, Some(0), crate::compiler::ir::ItemTypeIR::AnyItem),
            SeqTypeIR::Typed { item, occ } => {
                let (min, max) = match occ {
                    OccurrenceIR::One => (1, Some(1)),
                    OccurrenceIR::ZeroOrOne => (0, Some(1)),
                    OccurrenceIR::ZeroOrMore => (0, None),
                    OccurrenceIR::OneOrMore => (1, None),
                };
                (min, max, item)
            }
        };
        let pending_error = None;
        Self { vm, input: stream.cursor(), item_type, min, max, seen: 0, pending_error }
    }
}

impl<N: 'static + XdmNode + Clone> SequenceCursor<N> for TreatCursor<N> {
    fn next_item(&mut self) -> Option<XdmItemResult<N>> {
        if let Some(err) = self.pending_error.take() {
            return Some(Err(err));
        }
        match self.input.next_item() {
            None => {
                // End of input: verify min cardinality
                if self.seen < self.min {
                    return Some(Err(Error::from_code(
                        ErrorCode::XPTY0004,
                        format!("treat as failed: cardinality mismatch (expected min {} got {})", self.min, self.seen),
                    )));
                }
                None
            }
            Some(Ok(it)) => {
                self.seen += 1;
                if let Some(max) = self.max
                    && self.seen > max
                {
                    return Some(Err(Error::from_code(
                        ErrorCode::XPTY0004,
                        format!("treat as failed: cardinality mismatch (expected max {} got {})", max, self.seen),
                    )));
                }
                // Going through `with_vm` keeps its cancellation check.
                let ok = match self.vm.with_vm(|_| Ok(Vm::item_matches_type(&it, &self.item_type))) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                if !ok {
                    return Some(Err(Error::from_code(ErrorCode::XPTY0004, "treat as failed: type mismatch")));
                }
                Some(Ok(it))
            }
            Some(Err(e)) => Some(Err(e)),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
    fn boxed_clone(&self) -> Box<dyn SequenceCursor<N>> {
        Box::new(Self {
            vm: self.vm.clone(),
            input: self.input.boxed_clone(),
            item_type: self.item_type.clone(),
            min: self.min,
            max: self.max,
            seen: self.seen,
            pending_error: self.pending_error.clone(),
        })
    }
}
