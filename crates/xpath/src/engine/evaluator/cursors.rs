//! Cursor types for streaming XPath evaluation.

use super::*;

use core::cmp::Ordering;
use smallvec::SmallVec;
use std::collections::{HashSet, VecDeque};
use string_cache::DefaultAtom;

use crate::compiler::ir::{AxisIR, ComparisonOp, InstrSeq, NameOrWildcard, NodeTestIR, OpCode, QuantifierKind};
use crate::engine::runtime::{Error, ErrorCode};
use crate::model::{NodeKind, XdmNode};
use crate::xdm::{ExpandedName, SequenceCursor, XdmAtomicValue, XdmItem, XdmItemResult, XdmSequenceStream};

pub(super) struct AxisStepCursor<N> {
    vm: VmHandle<N>,
    axis: AxisIR,
    test: NodeTestIR,
    input_cursor: Box<dyn SequenceCursor<N>>,
    // Stream of results for the current input node (axis evaluation)
    current_output: Option<Box<dyn SequenceCursor<N>>>,
}

impl<N: 'static + XdmNode + Clone> AxisStepCursor<N> {
    pub(super) fn new(vm: VmHandle<N>, input: XdmSequenceStream<N>, axis: AxisIR, test: NodeTestIR) -> Self {
        // For descendant/descendant-or-self, minimize overlapping contexts to avoid duplicates
        let base_cursor = input.cursor();
        let input_cursor: Box<dyn SequenceCursor<N>> = match axis {
            AxisIR::Descendant | AxisIR::DescendantOrSelf => Box::new(ContextMinCursor::new(base_cursor)),
            AxisIR::Following => Box::new(ContextMinFollowingCursor::new(base_cursor)),
            AxisIR::FollowingSibling => Box::new(ContextMinFollowingSiblingCursor::new(base_cursor)),
            _ => base_cursor,
        };
        Self { vm, axis, test, input_cursor, current_output: None }
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
    // attribute:: axis true streaming without buffering
    AttributeIter {
        current: Option<N>,
        initialized: bool,
    },
    // Depth-first traversal for descendant/descendant-or-self using document-order successors.
    // `last` holds the last emitted node in pre-order; the next candidate is its doc_successor.
    // We stop once we reach the first node after the anchor's subtree (precomputed boundary).
    Descend {
        anchor: N,
        last: Option<N>,
        include_self: bool,
        started: bool,
        // First node after the subtree rooted at `anchor` (doc_successor(last_descendant(anchor)))
        // If None, there is no node after the subtree in this document.
        after: Option<N>,
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
        Self { vm, axis, test, node, state: AxisState::Init }
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
            AxisIR::Attribute => AxisState::AttributeIter { current: None, initialized: false },
            // replaced later by AttributeQueue in init_state refactor
            AxisIR::Parent => AxisState::Parent { done: false },
            AxisIR::Ancestor => AxisState::Ancestors { current: self.node.parent(), include_self: false },
            AxisIR::AncestorOrSelf => AxisState::Ancestors { current: Some(self.node.clone()), include_self: true },
            AxisIR::Descendant => AxisState::Descend {
                anchor: self.node.clone(),
                last: None,
                include_self: false,
                started: false,
                after: None,
            },
            AxisIR::DescendantOrSelf => AxisState::Descend {
                anchor: self.node.clone(),
                last: None,
                include_self: true,
                started: false,
                after: None,
            },
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
            AxisState::AttributeIter { current, initialized } => {
                if !*initialized {
                    *initialized = true;
                    *current = Self::first_attribute(&self.node);
                    // fast-skip non-matching attributes for common tests without borrowing self
                    let test = self.test.clone();
                    while let Some(cur) = current.as_ref() {
                        match Self::attribute_test_fast_match_static(&test, cur) {
                            Some(true) | None => break,
                            Some(false) => {
                                let next = Self::next_attribute_in_doc(&self.node, cur);
                                *current = next;
                                continue;
                            }
                        }
                    }
                }
                if let Some(cur) = current.take() {
                    // Determine if this test can only match one attribute at most
                    let single = matches!(
                        &self.test,
                        NodeTestIR::Name(_)
                            | NodeTestIR::KindAttribute { name: Some(NameOrWildcard::Name(_)), ty: None }
                    );
                    if single {
                        // No further matches possible for exact QName
                        *current = None;
                    } else {
                        // Pre-compute next matching attribute
                        let test = self.test.clone();
                        let mut next = Self::next_attribute_in_doc(&self.node, &cur);
                        while let Some(ref c2) = next {
                            match Self::attribute_test_fast_match_static(&test, c2) {
                                Some(true) | None => break,
                                Some(false) => {
                                    next = Self::next_attribute_in_doc(&self.node, c2);
                                }
                            }
                        }
                        *current = next;
                    }
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
            AxisState::Descend { anchor, last, include_self, started, after } => {
                if !*started {
                    *started = true;
                    // Precompute boundary: node after the subtree rooted at `anchor`.
                    let end = Self::last_descendant_in_doc(anchor.clone());
                    *after = Self::doc_successor(&end);
                    if *include_self {
                        let n = self.node.clone();
                        *last = Some(n.clone());
                        return Ok(Some(n));
                    } else {
                        // Start with the first child in pre-order
                        if let Some(first) = Self::first_child_in_doc(&self.node) {
                            *last = Some(first.clone());
                            return Ok(Some(first));
                        } else {
                            return Ok(None);
                        }
                    }
                }
                // Advance to the next document-order successor; stop when reaching boundary `after`.
                if let Some(prev) = last.take() {
                    if let Some(succ) = Self::doc_successor(&prev) {
                        if let Some(a) = after.as_ref()
                            && &succ == a
                        {
                            return Ok(None);
                        }
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
                    let start = Self::last_descendant_in_doc(self.node.clone());
                    *anchor = Some(start.clone());
                    *next = Self::doc_successor(&start);
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
                    } else {
                        *current = cur.parent();
                        continue;
                    }
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
            (AxisIR::Child, NodeTestIR::WildcardAny) => {
                return Ok(matches!(node.kind(), NodeKind::Element));
            }
            (AxisIR::Attribute, NodeTestIR::WildcardAny) => {
                return Ok(matches!(node.kind(), NodeKind::Attribute));
            }
            (AxisIR::Namespace, NodeTestIR::WildcardAny) => {
                return Ok(matches!(node.kind(), NodeKind::Namespace));
            }
            // element() with no constraints
            (_, NodeTestIR::KindElement { name: None, ty: None, nillable: false }) => {
                return Ok(matches!(node.kind(), NodeKind::Element));
            }
            // attribute() with no constraints
            (_, NodeTestIR::KindAttribute { name: None, ty: None }) => {
                return Ok(matches!(node.kind(), NodeKind::Attribute));
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
                return Ok(n.as_ref().map(|q| &q.local == target).unwrap_or(false));
            }
            // QName and namespace wildcards require effective-namespace resolution
            // Delegate to the full resolver to honor prefix/default namespace semantics and namespace-axis rules.
            (_, NodeTestIR::Name(_)) => {
                return self.vm.with_vm(|vm| Ok(vm.node_test(node, &self.test)));
            }
            (_, NodeTestIR::NsWildcard(_)) => {
                return self.vm.with_vm(|vm| Ok(vm.node_test(node, &self.test)));
            }
            (_, NodeTestIR::LocalWildcard(_)) => {
                return self.vm.with_vm(|vm| Ok(vm.node_test(node, &self.test)));
            }
            (AxisIR::DescendantOrSelf, NodeTestIR::WildcardAny) => {
                // For descendant-or-self element()/"*" we only pass elements
                return Ok(matches!(node.kind(), NodeKind::Element));
            }
            (AxisIR::SelfAxis, NodeTestIR::WildcardAny) => {
                // self::* should only match elements (not document nodes)
                return Ok(matches!(node.kind(), NodeKind::Element));
            }
            (AxisIR::Ancestor, NodeTestIR::WildcardAny) => {
                // ancestor::* should only match elements (not document nodes)
                return Ok(matches!(node.kind(), NodeKind::Element));
            }
            (AxisIR::AncestorOrSelf, NodeTestIR::WildcardAny) => {
                // ancestor-or-self::* should only match elements (not document nodes)
                return Ok(matches!(node.kind(), NodeKind::Element));
            }
            (AxisIR::Descendant, NodeTestIR::WildcardAny) => {
                // descendant::* should only match elements (not document nodes)
                return Ok(matches!(node.kind(), NodeKind::Element));
            }
            (AxisIR::Following, NodeTestIR::WildcardAny) => {
                // following::* should only match elements (not document nodes)
                return Ok(matches!(node.kind(), NodeKind::Element));
            }
            (AxisIR::Preceding, NodeTestIR::WildcardAny) => {
                // preceding::* should only match elements (not document nodes)
                return Ok(matches!(node.kind(), NodeKind::Element));
            }
            (AxisIR::FollowingSibling, NodeTestIR::WildcardAny) => {
                // following-sibling::* should only match elements (not document nodes)
                return Ok(matches!(node.kind(), NodeKind::Element));
            }
            (AxisIR::PrecedingSibling, NodeTestIR::WildcardAny) => {
                // preceding-sibling::* should only match elements (not document nodes)
                return Ok(matches!(node.kind(), NodeKind::Element));
            }
            (AxisIR::Parent, NodeTestIR::WildcardAny) => {
                // parent::* should only match elements (not document nodes)
                return Ok(matches!(node.kind(), NodeKind::Element));
            }
            _ => {}
        }
        // Fallback: use the full resolver through the VM to ensure correct namespace handling
        self.vm.with_vm(|vm| Ok(vm.node_test(node, &self.test)))
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

impl<N: XdmNode + Clone + 'static> SequenceCursor<N> for ContextMinCursor<N> {
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

// Minimize contexts for following:: by keeping only the earliest context per root.
struct ContextMinFollowingCursor<N> {
    inner: Box<dyn SequenceCursor<N>>,
    anchors: SmallVec<[(N, N); 4]>, // (root, earliest_anchor)
}

impl<N> ContextMinFollowingCursor<N> {
    fn new(inner: Box<dyn SequenceCursor<N>>) -> Self {
        Self { inner, anchors: SmallVec::new() }
    }
}

impl<N: XdmNode + Clone + 'static> ContextMinFollowingCursor<N> {
    fn root_of(n: &N) -> N {
        let mut cur = n.clone();
        while let Some(p) = cur.parent() {
            cur = p;
        }
        cur
    }
}

impl<N: XdmNode + Clone + 'static> SequenceCursor<N> for ContextMinFollowingCursor<N> {
    fn next_item(&mut self) -> Option<XdmItemResult<N>> {
        loop {
            let item = self.inner.next_item()?;
            match item {
                Ok(XdmItem::Node(n)) => {
                    let root = Self::root_of(&n);
                    if let Some((_, earliest)) = self.anchors.iter().find(|(r, _)| *r == root) {
                        // if candidate is after earliest (or equal), its following:: is subset → drop
                        if n == *earliest {
                            continue;
                        }
                        // We lack total order here without compare; conservatively drop only if n is after earliest.
                        // Determine by checking if earliest is ancestor of n or comes before n.
                        // Use document order comparator via Eq-based path: walk up to compare ancestry
                        // Fallback: treat as after if earliest is not after n.
                        // Simpler and safe: if n is a descendant of earliest, then n is after earliest.
                        let mut cur = Some(n.clone());
                        let mut is_desc = false;
                        let mut guard = 0usize;
                        while let Some(p) = cur {
                            if p == *earliest {
                                is_desc = true;
                                break;
                            }
                            let next = p.parent();
                            if next.as_ref().is_some_and(|q| q == &p) {
                                break;
                            }
                            cur = next;
                            guard = guard.saturating_add(1);
                            if guard > 1_000_000 {
                                break;
                            }
                        }
                        if is_desc {
                            continue;
                        }
                        // Otherwise keep it (could be before due to unsorted input)
                    } else {
                        self.anchors.push((root, n.clone()));
                        return Some(Ok(XdmItem::Node(n)));
                    }
                    // For existing root: only keep if not after earliest
                    return Some(Ok(XdmItem::Node(n)));
                }
                other => return Some(other),
            }
        }
    }

    fn boxed_clone(&self) -> Box<dyn SequenceCursor<N>> {
        Box::new(Self { inner: self.inner.boxed_clone(), anchors: self.anchors.clone() })
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
                        } else {
                            self.leftmost.push((parent, n.clone()));
                            return Some(Ok(XdmItem::Node(n)));
                        }
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
            let cand = match self.next_candidate() {
                Ok(opt) => opt,
                Err(err) => return Some(Err(err)),
            }?;
            match self.matches_test(&cand) {
                Ok(true) => return Some(Ok(XdmItem::Node(cand))),
                Ok(false) => continue,
                Err(err) => return Some(Err(err)),
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
    // attribute helpers for streaming without buffering
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
            NT::WildcardAny => Some(true),
            NT::KindAttribute { name: None, ty: None } => Some(true),
            NT::KindAttribute { name: Some(NameOrWildcard::Any), ty: None } => Some(true),
            NT::KindAttribute { name: Some(NameOrWildcard::Name(q)), ty: None } => {
                let n = attr.name()?;
                let matches_local = n.local == q.original.local;
                let matches_ns = match (&n.ns_uri, &q.original.ns_uri) {
                    (None, None) => true,
                    (Some(a), Some(b)) => a == b,
                    _ => false,
                };
                Some(matches_local && matches_ns)
            }
            NT::KindAttribute { name: Some(_), ty: Some(_) } | NT::KindAttribute { name: None, ty: Some(_) } => None,
            NT::Name(q) => {
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
                Some(n.ns_uri.as_deref().is_some_and(|u| u == ns.as_ref()))
            }
            NT::LocalWildcard(local) => {
                let n = attr.name()?;
                Some(n.local == local.as_ref())
            }
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
            let candidate = match self.input_cursor.next_item()? {
                Ok(item) => item,
                Err(err) => return Some(Err(err)),
            };
            let node = if let XdmItem::Node(n) = candidate { n } else { continue };
            // Build streaming axis cursor for this node (no extra clone)
            let cursor = NodeAxisCursor::new(self.vm.clone(), node, self.axis.clone(), self.test.clone());
            self.current_output = Some(Box::new(cursor));
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
            PredicateFastKind::None => {}
        }
        self.vm.with_vm(|vm| {
            let stream =
                vm.eval_subprogram_stream(&self.predicate, Some(item.clone()), Some(Frame { last, pos }), None)?;
            vm.predicate_truth_value_stream(stream, pos, last)
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

        while let Some(candidate) = self.input.next_item() {
            match candidate {
                Ok(item) => {
                    let pos = self.position + 1;
                    self.position = pos;
                    match self.evaluate_predicate(&item, pos, last) {
                        Ok(true) => return Some(Ok(item)),
                        Ok(false) => continue,
                        Err(err) => return Some(Err(err)),
                    }
                }
                Err(err) => return Some(Err(err)),
            }
        }
        None
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
    PositionLe(usize), // position() <= K (common in slices)
}

fn classify_predicate_fast(code: &InstrSeq) -> PredicateFastKind {
    use OpCode::*;
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
                PushAtomic(av) => {
                    if number.is_none() {
                        number = atomic_to_usize(av);
                    }
                }
                CompareValue(c) | CompareGeneral(c) => {
                    if cmp.is_none() {
                        cmp = Some(*c);
                    }
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

fn atomic_to_usize(av: &XdmAtomicValue) -> Option<usize> {
    match av {
        XdmAtomicValue::Integer(i) if *i >= 1 => Some(*i as usize),
        XdmAtomicValue::Long(i) if *i >= 1 => Some(*i as usize),
        XdmAtomicValue::Int(i) if *i >= 1 => Some(*i as usize),
        XdmAtomicValue::UnsignedInt(u) if *u >= 1 => Some(*u as usize),
        XdmAtomicValue::UnsignedLong(u) if *u >= 1 && *u <= usize::MAX as u64 => Some(*u as usize),
        XdmAtomicValue::Double(d) if *d >= 1.0 && d.fract() == 0.0 => Some(*d as usize),
        XdmAtomicValue::Decimal(d) if *d >= rust_decimal::Decimal::ONE && d.fract().is_zero() => {
            use rust_decimal::prelude::ToPrimitive;
            d.to_usize()
        }
        XdmAtomicValue::Float(f) if *f >= 1.0 && (*f as f64).fract() == 0.0 => Some(*f as usize),
        _ => None,
    }
}

// Cheap static analysis: does the predicate program reference `last()`?
pub(super) fn instr_seq_uses_last(code: &InstrSeq) -> bool {
    use OpCode::*;
    for op in &code.0 {
        match op {
            Last => return true,
            // Recurse into nested sequences where predicates might hide
            PathExprStep(inner) => {
                if instr_seq_uses_last(inner) {
                    return true;
                }
            }
            ApplyPredicates(preds) => {
                for p in preds {
                    if instr_seq_uses_last(p) {
                        return true;
                    }
                }
            }
            AxisStep(_, _, preds) => {
                for p in preds {
                    if instr_seq_uses_last(p) {
                        return true;
                    }
                }
            }
            ForLoop { body, .. } => {
                if instr_seq_uses_last(body) {
                    return true;
                }
            }
            QuantLoop { body, .. } => {
                if instr_seq_uses_last(body) {
                    return true;
                }
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
    pub(super) fn new(vm: VmHandle<N>, input_stream: XdmSequenceStream<N>, code: InstrSeq) -> Self {
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
    pub(super) fn new(vm: VmHandle<N>, input_stream: XdmSequenceStream<N>, var: ExpandedName, body: InstrSeq) -> Self {
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
        input_stream: XdmSequenceStream<N>,
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
            let truth = self.vm.with_vm(|vm| vm.ebv_stream(body_stream.cursor()))?;
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
    // streaming state
    seen_keys: HashSet<u64>,
    seen_fallback: SmallVec<[N; 16]>,
}

impl<N: 'static + XdmNode + Clone> DistinctCursor<N> {
    pub(super) fn new(vm: VmHandle<N>, input: Box<dyn SequenceCursor<N>>) -> Self {
        Self { vm, input: Some(input), seen_keys: HashSet::new(), seen_fallback: SmallVec::new() }
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
                    if let Some(k) = n.doc_order_key() {
                        if self.seen_keys.insert(k) {
                            return Some(Ok(XdmItem::Node(n)));
                        }
                    } else if !self.seen_fallback.iter().any(|m| m == &n) {
                        self.seen_fallback.push(n.clone());
                        return Some(Ok(XdmItem::Node(n)));
                    }
                }
                Ok(other) => return Some(Ok(other)),
                Err(e) => return Some(Err(e)),
            }
        }
    }

    fn boxed_clone(&self) -> Box<dyn SequenceCursor<N>> {
        Box::new(Self {
            vm: self.vm.clone(),
            input: self.input.as_ref().map(|c| c.boxed_clone()),
            seen_keys: self.seen_keys.clone(),
            seen_fallback: self.seen_fallback.clone(),
        })
    }
}

// EnsureOrderCursor: streams if input is already in document order; otherwise falls back
// to buffering and sorting the remaining items. Implements a one-item lookahead to avoid
// emitting an out-of-order item before detecting disorder.
pub(super) struct EnsureOrderCursor<N> {
    vm: VmHandle<N>,
    input: Box<dyn SequenceCursor<N>>,
    pending: Option<XdmItem<N>>, // last unconfirmed item
    last_key: Option<u64>,
    last_node: Option<N>,
    // fallback buffer once disorder is detected
    buffer: VecDeque<XdmItem<N>>,
    in_fallback: bool,
}

impl<N: 'static + XdmNode + Clone> EnsureOrderCursor<N> {
    pub(super) fn new(vm: VmHandle<N>, input: Box<dyn SequenceCursor<N>>) -> Self {
        Self { vm, input, pending: None, last_key: None, last_node: None, buffer: VecDeque::new(), in_fallback: false }
    }

    fn cmp_doc_order(&self, a: &N, b: &N) -> Ordering {
        self.vm.with_vm(|vm| vm.node_compare(a, b)).unwrap_or(Ordering::Equal)
    }

    fn switch_to_fallback(&mut self, first: XdmItem<N>, second: XdmItem<N>) -> Result<(), Error> {
        // Collect remaining, then order nodes only via doc_order_only
        let mut seq: Vec<XdmItem<N>> = vec![first, second];
        while let Some(item) = self.input.next_item() {
            seq.push(item?);
        }
        let ordered = self.vm.with_vm(|vm| vm.doc_order_only(seq))?;
        self.buffer = VecDeque::from(ordered);
        self.in_fallback = true;
        Ok(())
    }
}

impl<N: 'static + XdmNode + Clone> SequenceCursor<N> for EnsureOrderCursor<N> {
    fn next_item(&mut self) -> Option<XdmItemResult<N>> {
        use crate::xdm::XdmItem;
        if self.in_fallback {
            return self.buffer.pop_front().map(Ok);
        }

        loop {
            let next = match self.input.next_item() {
                Some(Ok(it)) => it,
                Some(Err(e)) => return Some(Err(e)),
                None => {
                    // No more input; flush pending
                    if let Some(item) = self.pending.take() {
                        return Some(Ok(item));
                    }
                    return None;
                }
            };

            match (&self.pending, &next) {
                (None, _) => {
                    // prime the window, but do not emit yet
                    self.pending = Some(next.clone());
                    if let XdmItem::Node(n) = &next {
                        self.last_key = n.doc_order_key();
                        self.last_node = Some(n.clone());
                    }
                    continue;
                }
                (Some(XdmItem::Node(_prev_n)), XdmItem::Node(cur_n)) => {
                    // Check monotonicity
                    let ok = if let (Some(pk), Some(ck)) = (self.last_key, cur_n.doc_order_key()) {
                        ck >= pk
                    } else if let Some(pn) = &self.last_node {
                        self.cmp_doc_order(pn, cur_n) != Ordering::Greater
                    } else {
                        true
                    };

                    if ok {
                        // emit previous, shift window
                        let to_emit =
                            self.pending.replace(next.clone()).expect("pending must be Some in monotonic node branch");
                        self.last_key = cur_n.doc_order_key();
                        self.last_node = Some(cur_n.clone());
                        return Some(Ok(to_emit));
                    } else {
                        // Try local adjacent-swap repair (single inversion): emit `cur` first if it
                        // still maintains global monotonicity relative to the last emitted item.
                        let can_swap = self
                            .last_node
                            .as_ref()
                            .map(|ln| self.cmp_doc_order(ln, cur_n) != Ordering::Greater)
                            .unwrap_or(true);
                        if can_swap {
                            // Emit current (`next`) immediately; keep `pending` (prev) for next round.
                            self.last_key = cur_n.doc_order_key();
                            self.last_node = Some(cur_n.clone());
                            return Some(Ok(next));
                        } else {
                            // disorder beyond simple adjacent inversion → fallback
                            let first = self.pending.take().expect("pending must be Some before fallback switch");
                            if let Err(e) = self.switch_to_fallback(first, next) {
                                return Some(Err(e));
                            }
                            return self.buffer.pop_front().map(Ok);
                        }
                    }
                }
                (Some(_prev), _) => {
                    // Non-node items: emit previous, shift window
                    let to_emit = self.pending.replace(next).expect("pending must be Some in non-node emit branch");
                    self.last_key = None;
                    self.last_node = None;
                    return Some(Ok(to_emit));
                }
            }
        }
    }

    fn boxed_clone(&self) -> Box<dyn SequenceCursor<N>> {
        Box::new(Self {
            vm: self.vm.clone(),
            input: self.input.boxed_clone(),
            pending: self.pending.clone(),
            last_key: self.last_key,
            last_node: self.last_node.clone(),
            buffer: self.buffer.clone(),
            in_fallback: self.in_fallback,
        })
    }
}

pub(super) struct AtomizeCursor<N> {
    input: Box<dyn SequenceCursor<N>>,
    pending: VecDeque<XdmAtomicValue>,
}

impl<N: 'static + XdmNode + Clone> AtomizeCursor<N> {
    pub(super) fn new(stream: XdmSequenceStream<N>) -> Self {
        Self { input: stream.cursor(), pending: VecDeque::new() }
    }
}

impl<N: 'static + XdmNode + Clone> SequenceCursor<N> for AtomizeCursor<N> {
    fn next_item(&mut self) -> Option<XdmItemResult<N>> {
        use XdmItem::*;
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
                    continue;
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
    pub(super) fn new(vm: VmHandle<N>, stream: XdmSequenceStream<N>, t: crate::compiler::ir::SeqTypeIR) -> Self {
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
                let ok = self.vm.with_vm(|vm| vm.item_matches_type(&it, &self.item_type)).unwrap_or(false);
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
