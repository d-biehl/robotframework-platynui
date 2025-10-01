use crate::compiler::ir::{
    AxisIR, ComparisonOp, CompiledXPath, InstrSeq, NameOrWildcard, NodeTestIR, OpCode,
    QuantifierKind, SeqTypeIR, SingleTypeIR,
};
use crate::engine::functions::parse_qname_lexical;
use crate::engine::runtime::{
    CallCtx, DynamicContext, Error, ErrorCode, FunctionImplementations, ParamTypeSpec,
};
// fast_names_equal inlined: equality on interned atoms is direct O(1) comparison
use crate::model::{NodeKind, XdmNode};
use crate::util::temporal::{
    parse_g_day, parse_g_month, parse_g_month_day, parse_g_year, parse_g_year_month,
};
use crate::xdm::{
    ExpandedName, SequenceCursor, XdmAtomicValue, XdmItem, XdmItemResult, XdmSequence,
    XdmSequenceStream,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use chrono::Duration as ChronoDuration;
use chrono::{FixedOffset as ChronoFixedOffset, NaiveTime as ChronoNaiveTime, TimeZone};
use core::cmp::Ordering;
use core::mem;
use smallvec::SmallVec;
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex};
use string_cache::DefaultAtom;

/// Evaluate a compiled XPath program against a dynamic context.
pub fn evaluate<N: 'static + Send + Sync + XdmNode + Clone>(
    compiled: &CompiledXPath,
    dyn_ctx: &DynamicContext<N>,
) -> Result<XdmSequence<N>, Error> {
    evaluate_stream(compiled, dyn_ctx)?.materialize()
}

pub fn evaluate_stream<N: 'static + Send + Sync + XdmNode + Clone>(
    compiled: &CompiledXPath,
    dyn_ctx: &DynamicContext<N>,
) -> Result<XdmSequenceStream<N>, Error> {
    let compiled_arc = Arc::new(compiled.clone());
    let dyn_ctx_arc = Arc::new(dyn_ctx.clone());
    let mut vm = Vm::new(compiled_arc, dyn_ctx_arc);
    vm.run(&compiled.instrs)
}

/// Convenience: compile+evaluate a string using default static context.
pub fn evaluate_expr<N: 'static + Send + Sync + XdmNode + Clone>(
    expr: &str,
    dyn_ctx: &DynamicContext<N>,
) -> Result<XdmSequence<N>, Error> {
    let compiled = crate::compiler::compile(expr)?;
    evaluate(&compiled, dyn_ctx)
}

/// Convenience: compile+evaluate to a streaming sequence using default static context.
pub fn evaluate_stream_expr<N: 'static + Send + Sync + XdmNode + Clone>(
    expr: &str,
    dyn_ctx: &DynamicContext<N>,
) -> Result<XdmSequenceStream<N>, Error> {
    let compiled = crate::compiler::compile(expr)?;
    evaluate_stream(&compiled, dyn_ctx)
}

struct Vm<N> {
    compiled: Arc<CompiledXPath>,
    dyn_ctx: Arc<DynamicContext<N>>,
    stack: SmallVec<[XdmSequenceStream<N>; 16]>, // Keep short evaluation stacks inline to avoid heap churn
    local_vars: SmallVec<[(ExpandedName, XdmSequenceStream<N>); 12]>, // Small lexical scopes fit in the inline buffer
    // Frame stack for position()/last() support inside predicates / loops
    frames: SmallVec<[Frame; 12]>, // Mirrors typical nesting depth for predicates/loops
    // Cached default collation for this VM (dynamic overrides static)
    default_collation: Option<std::sync::Arc<dyn crate::engine::collation::Collation>>,
    functions: Arc<FunctionImplementations<N>>,
    current_context_item: Option<XdmItem<N>>,
    axis_buffer: SmallVec<[N; 32]>, // Shared scratch space for axis traversal results
    cancel_flag: Option<Arc<AtomicBool>>,
    set_fallback: SmallVec<[N; 16]>, // Scratch buffer reused by set operations
}

struct VmSnapshot<N> {
    compiled: Arc<CompiledXPath>,
    dyn_ctx: Arc<DynamicContext<N>>,
    local_vars: SmallVec<[(ExpandedName, XdmSequenceStream<N>); 12]>,
    frames: SmallVec<[Frame; 12]>,
    default_collation: Option<Arc<dyn crate::engine::collation::Collation>>,
    functions: Arc<FunctionImplementations<N>>,
    current_context_item: Option<XdmItem<N>>,
}

struct VmHandleInner<N> {
    snapshot: VmSnapshot<N>,
    cache: Mutex<Option<Vm<N>>>,
    cancel_flag: Option<Arc<AtomicBool>>,
}

#[derive(Clone)]
struct VmHandle<N> {
    inner: Arc<VmHandleInner<N>>,
}

impl<N: 'static + Send + Sync + XdmNode + Clone> VmHandle<N> {
    fn new(snapshot: VmSnapshot<N>, cancel_flag: Option<Arc<AtomicBool>>) -> Self {
        Self { inner: Arc::new(VmHandleInner { snapshot, cache: Mutex::new(None), cancel_flag }) }
    }

    fn with_vm<F, R>(&self, f: F) -> Result<R, Error>
    where
        F: FnOnce(&mut Vm<N>) -> Result<R, Error>,
    {
        if self.inner.cancel_flag.as_ref().is_some_and(|flag| flag.load(AtomicOrdering::Relaxed)) {
            return Err(Error::from_code(ErrorCode::FOER0000, "evaluation cancelled"));
        }
        let snapshot = &self.inner.snapshot;
        let mut vm = {
            let mut guard = self.inner.cache.lock().expect("vm cache poisoned");
            guard.take()
        }
        .unwrap_or_else(|| Vm::from_snapshot(snapshot));

        let result = f(&mut vm);

        vm.reset_to_snapshot(snapshot);

        let mut guard = self.inner.cache.lock().expect("vm cache poisoned");
        *guard = Some(vm);

        result
    }

    fn is_cancelled(&self) -> bool {
        self.inner.cancel_flag.as_ref().is_some_and(|flag| flag.load(AtomicOrdering::Relaxed))
    }
}

impl<N: XdmNode + Clone> Clone for VmSnapshot<N> {
    fn clone(&self) -> Self {
        Self {
            compiled: Arc::clone(&self.compiled),
            dyn_ctx: Arc::clone(&self.dyn_ctx),
            local_vars: self.local_vars.clone(),
            frames: self.frames.clone(),
            default_collation: self.default_collation.as_ref().map(Arc::clone),
            functions: Arc::clone(&self.functions),
            current_context_item: self.current_context_item.clone(),
        }
    }
}

struct AxisStepCursor<N> {
    vm: VmHandle<N>,
    axis: AxisIR,
    test: NodeTestIR,
    input_cursor: Box<dyn SequenceCursor<N>>,
    // Stream of results for the current input node (axis evaluation)
    current_output: Option<Box<dyn SequenceCursor<N>>>,
}

impl<N: 'static + Send + Sync + XdmNode + Clone> AxisStepCursor<N> {
    fn new(vm: VmHandle<N>, input: XdmSequenceStream<N>, axis: AxisIR, test: NodeTestIR) -> Self {
        Self { vm, axis, test, input_cursor: input.cursor(), current_output: None }
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
    SelfOnce { emitted: bool },
    // Iterate a buffered list with index (used for cheap axes)
    Buffer { buf: SmallVec<[N; 16]>, idx: usize },
    // Depth-first traversal stack for descendant/descendant-or-self
    Descend { stack: SmallVec<[N; 16]>, include_self: bool, started: bool },
    // Parent/ancestor chains
    Parent { done: bool },
    Ancestors { current: Option<N>, include_self: bool },
    // Sibling scans
    FollowingSibling { buf: SmallVec<[N; 16]>, idx: usize, initialized: bool },
    PrecedingSibling { buf: SmallVec<[N; 16]>, idx: usize, initialized: bool },
    // Following/Preceding document order
    Following { anchor: Option<N>, next: Option<N>, initialized: bool },
    Preceding {
        // path from root to context (inclusive)
        path: SmallVec<[N; 16]>,
        // index into path where parent=path[depth], child=path[depth+1]
        depth: usize,
        // siblings before the path child at current depth (left siblings)
        sibs: SmallVec<[N; 16]>,
        sib_idx: usize,
        // pre-order traversal stack for current sibling subtree
        subtree_stack: SmallVec<[N; 16]>,
    },
    // Namespace axis
    Namespaces { seen: SmallVec<[DefaultAtom; 8]>, current: Option<N>, buf: SmallVec<[N; 8]>, idx: usize },
}

impl<N: 'static + Send + Sync + XdmNode + Clone> NodeAxisCursor<N> {
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
            AxisIR::Child => AxisState::Buffer { buf: SmallVec::new(), idx: 0 },
            AxisIR::Attribute => AxisState::Buffer { buf: SmallVec::new(), idx: 0 },
            AxisIR::Parent => AxisState::Parent { done: false },
            AxisIR::Ancestor => AxisState::Ancestors { current: self.node.parent(), include_self: false },
            AxisIR::AncestorOrSelf => AxisState::Ancestors { current: Some(self.node.clone()), include_self: true },
            AxisIR::Descendant => AxisState::Descend { stack: SmallVec::new(), include_self: false, started: false },
            AxisIR::DescendantOrSelf => AxisState::Descend { stack: SmallVec::new(), include_self: true, started: false },
            AxisIR::FollowingSibling => AxisState::FollowingSibling { buf: SmallVec::new(), idx: 0, initialized: false },
            AxisIR::PrecedingSibling => AxisState::PrecedingSibling { buf: SmallVec::new(), idx: 0, initialized: false },
            AxisIR::Following => AxisState::Following { anchor: None, next: None, initialized: false },
            AxisIR::Preceding => {
                let path = Self::path_to_root(self.node.clone());
                AxisState::Preceding {
                    path,
                    depth: 0,
                    sibs: SmallVec::new(),
                    sib_idx: 0,
                    subtree_stack: SmallVec::new(),
                }
            }
            AxisIR::Namespace => {
                let cur = if matches!(self.node.kind(), NodeKind::Element) {
                    Some(self.node.clone())
                } else {
                    None
                };
                AxisState::Namespaces { seen: SmallVec::new(), current: cur, buf: SmallVec::new(), idx: 0 }
            },
        };
    }

    fn fill_buffer_via_vm(&mut self) -> Result<(), Error> {
        // One-shot: run the VM's axis iterator and filter
        let axis = self.axis.clone();
        let test = self.test.clone();
        let node = self.node.clone();
        let (filtered, _) = self.vm.with_vm(|vm| {
            vm.axis_iter(node.clone(), &axis);
            let filter_child_elements = matches!(axis, AxisIR::Child)
                && matches!(test, NodeTestIR::WildcardAny);
            let mut candidates = mem::take(&mut vm.axis_buffer);
            let mut out: SmallVec<[N; 16]> = SmallVec::with_capacity(candidates.len());
            for cand in candidates.iter() {
                let mut pass = vm.node_test(cand, &test);
                if pass && filter_child_elements {
                    pass = matches!(cand.kind(), NodeKind::Element);
                }
                if pass {
                    out.push(cand.clone());
                }
            }
            candidates.clear();
            vm.axis_buffer = candidates;
            Ok::<_, Error>((out, ()))
        })?;
        self.state = AxisState::Buffer { buf: filtered, idx: 0 };
        Ok(())
    }

    fn next_candidate(&mut self) -> Result<Option<N>, Error> {
        if matches!(self.state, AxisState::Init) {
            self.init_state();
        }
        match &mut self.state {
            AxisState::SelfOnce { emitted } => {
                if *emitted { return Ok(None); }
                *emitted = true;
                Ok(Some(self.node.clone()))
            }
            AxisState::Buffer { buf, idx: _ } => {
                if buf.is_empty() {
                    // First entry into this buffer: populate once
                    self.fill_buffer_via_vm()?;
                }
                match &mut self.state {
                    AxisState::Buffer { buf, idx } => {
                        if *idx < buf.len() {
                            let n = buf[*idx].clone();
                            *idx += 1;
                            Ok(Some(n))
                        } else {
                            Ok(None)
                        }
                    }
                    _ => Ok(None),
                }
            }
            AxisState::Parent { done } => {
                if *done { return Ok(None); }
                *done = true;
                Ok(self.node.parent())
            }
            AxisState::Ancestors { current, include_self } => {
                if let Some(cur) = current.take() {
                    let emit = cur.clone();
                    *current = cur.parent();
                    if !*include_self && emit == self.node { return self.next_candidate(); }
                    Ok(Some(emit))
                } else {
                    Ok(None)
                }
            }
            AxisState::Descend { stack, include_self, started } => {
                if !*started {
                    *started = true;
                    // Always prime the stack with initial children
                    for c in self.node.children() { if !Self::is_attr_or_namespace(&c) { stack.push(c); } }
                    if *include_self {
                        // For descendant-or-self include context node itself first
                        return Ok(Some(self.node.clone()));
                    }
                }
                if let Some(cur) = stack.pop() {
                    // Push children for further traversal
                    for c in cur.children() { if !Self::is_attr_or_namespace(&c) { stack.push(c.clone()); } }
                    Ok(Some(cur))
                } else {
                    Ok(None)
                }
            }
            AxisState::FollowingSibling { buf, idx, initialized } => {
                if !*initialized {
                    *initialized = true;
                    if let Some(parent) = self.node.parent() {
                        let mut seen = false;
                        for sib in parent.children() {
                            if seen {
                                if !Self::is_attr_or_namespace(&sib) { buf.push(sib); }
                            } else if sib == self.node { seen = true; }
                        }
                    }
                }
                if *idx < buf.len() { let n = buf[*idx].clone(); *idx += 1; Ok(Some(n)) } else { Ok(None) }
            }
            AxisState::PrecedingSibling { buf, idx, initialized } => {
                if !*initialized {
                    *initialized = true;
                    if let Some(parent) = self.node.parent() {
                        for sib in parent.children() {
                            if sib == self.node { break; }
                            if !Self::is_attr_or_namespace(&sib) { buf.push(sib); }
                        }
                    }
                }
                if *idx < buf.len() { let n = buf[*idx].clone(); *idx += 1; Ok(Some(n)) } else { Ok(None) }
            }
            AxisState::Following { anchor, next, initialized } => {
                if !*initialized {
                    *initialized = true;
                    let start = Self::last_descendant_in_doc(self.node.clone());
                    *anchor = Some(start.clone());
                    *next = Self::doc_successor(&start);
                }
                if let Some(n) = next.take() {
                    // compute following successor for subsequent call
                    if !Self::is_attr_or_namespace(&n) {
                        *anchor = Some(n.clone());
                        *next = Self::doc_successor(&n);
                        return Ok(Some(n));
                    } else {
                        // skip attr/ns; continue to next successor
                        *anchor = Some(n.clone());
                        *next = Self::doc_successor(&n);
                        return self.next_candidate();
                    }
                }
                Ok(None)
            }
            AxisState::Preceding { path, depth, sibs, sib_idx, subtree_stack } => {
                // emit subtree nodes if any
                if let Some(n) = subtree_stack.pop() {
                    // push children in reverse order to get pre-order traversal
                    let mut tmp: SmallVec<[N; 16]> = SmallVec::new();
                    for c in n.children() { if !Self::is_attr_or_namespace(&c) { tmp.push(c); } }
                    for c in tmp.into_iter().rev() { subtree_stack.push(c); }
                    return Ok(Some(n));
                }
                // move to next sibling subtree if available
                if *sib_idx < sibs.len() {
                    let root = sibs[*sib_idx].clone();
                    *sib_idx += 1;
                    subtree_stack.push(root);
                    return self.next_candidate();
                }
                // advance to next ancestor level
                if *depth + 1 < path.len() {
                    let parent = &path[*depth];
                    let child = &path[*depth + 1];
                    sibs.clear();
                    *sib_idx = 0;
                    for s in parent.children() {
                        if s == *child { break; }
                        if !Self::is_attr_or_namespace(&s) { sibs.push(s); }
                    }
                    *depth += 1;
                    return self.next_candidate();
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
            AxisState::Init => unreachable!(),
        }
    }

    fn matches_test(&self, node: &N) -> Result<bool, Error> {
        // Fast paths for common patterns to skip VM roundtrip
        match (&self.axis, &self.test) {
            (AxisIR::Child, NodeTestIR::WildcardAny) => {
                return Ok(matches!(node.kind(), NodeKind::Element));
            }
            (
                AxisIR::DescendantOrSelf,
                NodeTestIR::KindElement { name: None, ty: None, nillable: false },
            ) | (
                AxisIR::DescendantOrSelf,
                NodeTestIR::WildcardAny,
            ) => {
                // For descendant-or-self element()/"*" we only pass elements
                return Ok(matches!(node.kind(), NodeKind::Element));
            }
            _ => {}
        }
        self.vm.with_vm(|vm| Ok(vm.node_test(node, &self.test)))
    }
}

impl<N: 'static + Send + Sync + XdmNode + Clone> SequenceCursor<N> for NodeAxisCursor<N> {
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

    fn size_hint(&self) -> (usize, Option<usize>) { (0, None) }

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

impl<N: XdmNode + Clone> NodeAxisCursor<N> {
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
            if seen && !Self::is_attr_or_namespace(&s) { return Some(s); }
            if s == *node { seen = true; }
        }
        None
    }
    fn last_descendant_in_doc(mut node: N) -> N {
        loop {
            let mut last: Option<N> = None;
            for c in node.children() { if !Self::is_attr_or_namespace(&c) { last = Some(c); } }
            if let Some(n) = last { node = n; } else { return node; }
        }
    }
    fn doc_successor(node: &N) -> Option<N> {
        if let Some(c) = Self::first_child_in_doc(node) { return Some(c); }
        let mut cur = node.clone();
        while let Some(p) = cur.parent() {
            if let Some(sib) = Self::next_sibling_in_doc(&cur) { return Some(sib); }
            cur = p;
        }
        None
    }
}

impl<N: 'static + Send + Sync + XdmNode + Clone> SequenceCursor<N> for AxisStepCursor<N> {
    fn next_item(&mut self) -> Option<XdmItemResult<N>> {
        loop {
            if let Some(ref mut current) = self.current_output {
                if let Some(item) = current.next_item() { return Some(item); }
                self.current_output = None;
            }
            // Pull next context item
            let candidate = match self.input_cursor.next_item()? { Ok(item) => item, Err(err) => return Some(Err(err)) };
            let node = if let XdmItem::Node(n) = candidate { n } else { continue };
            // Build streaming axis cursor for this node
            let cursor = NodeAxisCursor::new(self.vm.clone(), node, self.axis.clone(), self.test.clone());
            self.current_output = Some(cursor.boxed_clone());
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) { (0, None) }

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

struct PredicateCursor<N> {
    vm: VmHandle<N>,
    predicate: InstrSeq,
    input: Box<dyn SequenceCursor<N>>,
    seed: Option<Box<dyn SequenceCursor<N>>>,
    position: usize,
    last_cache: Option<usize>,
}

impl<N: 'static + Send + Sync + XdmNode + Clone> PredicateCursor<N> {
    fn new(vm: VmHandle<N>, predicate: InstrSeq, input: Box<dyn SequenceCursor<N>>) -> Self {
        let seed = Some(input.boxed_clone());
        Self { vm, predicate, input, seed, position: 0, last_cache: None }
    }

    fn ensure_last(&mut self) -> Result<usize, Error> {
        if let Some(last) = self.last_cache {
            return Ok(last);
        }
        let mut cursor =
            if let Some(seed) = self.seed.take() { seed } else { self.input.boxed_clone() };
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

    fn evaluate_predicate(
        &self,
        item: &XdmItem<N>,
        pos: usize,
        last: usize,
    ) -> Result<bool, Error> {
        self.vm.with_vm(|vm| {
            let stream = vm.eval_subprogram_stream(
                &self.predicate,
                Some(item.clone()),
                Some(Frame { last, pos }),
                None,
            )?;
            vm.predicate_truth_value_stream(stream, pos, last)
        })
    }
}

impl<N: 'static + Send + Sync + XdmNode + Clone> SequenceCursor<N> for PredicateCursor<N> {
    fn next_item(&mut self) -> Option<XdmItemResult<N>> {
        let last = match self.ensure_last() {
            Ok(v) => v,
            Err(err) => return Some(Err(err)),
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
        })
    }
}

struct PathStepCursor<N> {
    vm: VmHandle<N>,
    code: InstrSeq,
    input: Box<dyn SequenceCursor<N>>,
    seed: Option<Box<dyn SequenceCursor<N>>>,
    input_len: Option<usize>,
    position: usize,
    current_output: Option<Box<dyn SequenceCursor<N>>>,
}

impl<N: 'static + Send + Sync + XdmNode + Clone> PathStepCursor<N> {
    fn new(vm: VmHandle<N>, input_stream: XdmSequenceStream<N>, code: InstrSeq) -> Self {
        let input = input_stream.cursor();
        let seed = Some(input.boxed_clone());
        Self { vm, code, input, seed, input_len: None, position: 0, current_output: None }
    }

    fn ensure_input_len(&mut self) -> Result<usize, Error> {
        if let Some(len) = self.input_len {
            return Ok(len);
        }
        let mut cursor =
            if let Some(seed) = self.seed.take() { seed } else { self.input.boxed_clone() };
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

impl<N: 'static + Send + Sync + XdmNode + Clone> SequenceCursor<N> for PathStepCursor<N> {
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

            let last = match self.ensure_input_len() {
                Ok(v) => v,
                Err(err) => return Some(Err(err)),
            };
            let pos = self.position + 1;
            self.position = pos;

            let stream = match self.vm.with_vm(|vm| {
                vm.eval_subprogram_stream(
                    &self.code,
                    Some(candidate.clone()),
                    Some(Frame { last, pos }),
                    None,
                )
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
        })
    }
}

struct DocOrderDistinctCursor<N> {
    vm: VmHandle<N>,
    input: Option<Box<dyn SequenceCursor<N>>>,
    buffer: VecDeque<XdmItem<N>>,
    initialized: bool,
    keyed: SmallVec<[(u64, N); 16]>,
    fallback: SmallVec<[N; 16]>,
    others: Vec<XdmItem<N>>,
}

impl<N: 'static + Send + Sync + XdmNode + Clone> DocOrderDistinctCursor<N> {
    fn new(vm: VmHandle<N>, input: Box<dyn SequenceCursor<N>>) -> Self {
        Self {
            vm,
            input: Some(input),
            buffer: VecDeque::new(),
            initialized: false,
            keyed: SmallVec::new(),
            fallback: SmallVec::new(),
            others: Vec::with_capacity(32),
        }
    }

    fn ensure_buffer(&mut self) -> Result<(), Error> {
        if self.initialized {
            return Ok(());
        }
        let mut cursor = match self.input.take() {
            Some(c) => c,
            None => return Ok(()),
        };

        let mut keyed = mem::take(&mut self.keyed);
        let mut fallback = mem::take(&mut self.fallback);
        let mut others = mem::take(&mut self.others);
        keyed.clear();
        fallback.clear();
        others.clear();
        self.buffer.clear();

        while let Some(item) = cursor.next_item() {
            match item {
                Ok(XdmItem::Node(n)) => {
                    if let Some(key) = n.doc_order_key() {
                        keyed.push((key, n));
                    } else {
                        fallback.push(n);
                    }
                }
                Ok(other) => others.push(other),
                Err(err) => return Err(err),
            }
        }

        if keyed.is_empty() && fallback.is_empty() {
            self.buffer.extend(others.drain(..));
            self.initialized = true;
            self.keyed = keyed;
            self.fallback = fallback;
            self.others = others;
            return Ok(());
        }

        if fallback.is_empty() {
            keyed.sort_by_key(|(k, _)| *k);
            keyed.dedup_by(|a, b| a.0 == b.0);
            self.buffer.extend(others.drain(..));
            self.buffer.extend(keyed.drain(..).map(|(_, n)| XdmItem::Node(n)));
            self.initialized = true;
            self.keyed = keyed;
            self.fallback = fallback;
            self.others = others;
            return Ok(());
        }

        if keyed.is_empty() {
            fallback.sort_by(|a, b| self.compare_nodes(a, b));
            fallback.dedup();
            self.buffer.extend(others.drain(..));
            self.buffer.extend(fallback.drain(..).map(XdmItem::Node));
            self.initialized = true;
            self.keyed = keyed;
            self.fallback = fallback;
            self.others = others;
            return Ok(());
        }

        keyed.sort_by_key(|(k, _)| *k);
        keyed.dedup_by(|a, b| a.0 == b.0);
        fallback.extend(keyed.drain(..).map(|(_, n)| n));
        fallback.sort_by(|a, b| self.compare_nodes(a, b));
        fallback.dedup();
        self.buffer.extend(others.drain(..));
        self.buffer.extend(fallback.drain(..).map(XdmItem::Node));
        self.initialized = true;
        self.keyed = keyed;
        self.fallback = fallback;
        self.others = others;
        Ok(())
    }

    fn compare_nodes(&self, a: &N, b: &N) -> Ordering {
        self.vm.with_vm(|vm| vm.node_compare(a, b)).unwrap_or(Ordering::Equal)
    }
}

impl<N: 'static + Send + Sync + XdmNode + Clone> SequenceCursor<N> for DocOrderDistinctCursor<N> {
    fn next_item(&mut self) -> Option<XdmItemResult<N>> {
        if let Err(err) = self.ensure_buffer() {
            return Some(Err(err));
        }
        self.buffer.pop_front().map(Ok)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.initialized {
            let len = self.buffer.len();
            (len, Some(len))
        } else {
            (0, None)
        }
    }

    fn boxed_clone(&self) -> Box<dyn SequenceCursor<N>> {
        Box::new(Self {
            vm: self.vm.clone(),
            input: self.input.as_ref().map(|c| c.boxed_clone()),
            buffer: self.buffer.clone(),
            initialized: self.initialized,
            keyed: self.keyed.clone(),
            fallback: self.fallback.clone(),
            others: self.others.clone(),
        })
    }
}

#[derive(Clone, Copy, Debug)]
enum SetOpKind {
    Union,
    Intersect,
    Except,
}

struct SetOperationCursor<N> {
    vm: VmHandle<N>,
    lhs: Option<Box<dyn SequenceCursor<N>>>,
    rhs: Option<Box<dyn SequenceCursor<N>>>,
    kind: SetOpKind,
    buffer: VecDeque<XdmItem<N>>,
    initialized: bool,
}

impl<N: 'static + Send + Sync + XdmNode + Clone> SetOperationCursor<N> {
    fn new(
        vm: VmHandle<N>,
        kind: SetOpKind,
        lhs: Box<dyn SequenceCursor<N>>,
        rhs: Box<dyn SequenceCursor<N>>,
    ) -> Self {
        Self {
            vm,
            lhs: Some(lhs),
            rhs: Some(rhs),
            kind,
            buffer: VecDeque::new(),
            initialized: false,
        }
    }

    fn collect_sequence(mut cursor: Box<dyn SequenceCursor<N>>) -> Result<XdmSequence<N>, Error> {
        let (lower, upper) = cursor.size_hint();
        let mut seq = match upper {
            Some(exact) => Vec::with_capacity(exact),
            None => Vec::with_capacity(lower.max(4)),
        };

        while let Some(item) = cursor.next_item() {
            if seq.len() == seq.capacity() {
                let grow = seq.len().max(16);
                seq.reserve(grow);
            }
            seq.push(item?);
        }
        Ok(seq)
    }

    fn ensure_buffer(&mut self) -> Result<(), Error> {
        if self.initialized {
            return Ok(());
        }
        let lhs_cursor = self.lhs.take().unwrap();
        let rhs_cursor = self.rhs.take().unwrap();
        let lhs_seq = Self::collect_sequence(lhs_cursor)?;
        let rhs_seq = Self::collect_sequence(rhs_cursor)?;
        let is_nodes_only =
            |seq: &XdmSequence<N>| seq.iter().all(|it| matches!(it, XdmItem::Node(_)));
        if !is_nodes_only(&lhs_seq) || !is_nodes_only(&rhs_seq) {
            return Err(Error::from_code(
                ErrorCode::XPTY0004,
                "set operators require node sequences",
            ));
        }
        let result = self.vm.with_vm(|vm| match self.kind {
            SetOpKind::Union => vm.set_union(lhs_seq, rhs_seq),
            SetOpKind::Intersect => vm.set_intersect(lhs_seq, rhs_seq),
            SetOpKind::Except => vm.set_except(lhs_seq, rhs_seq),
        })?;
        self.buffer = VecDeque::from(result);
        self.initialized = true;
        Ok(())
    }
}

impl<N: 'static + Send + Sync + XdmNode + Clone> SequenceCursor<N> for SetOperationCursor<N> {
    fn next_item(&mut self) -> Option<XdmItemResult<N>> {
        if let Err(err) = self.ensure_buffer() {
            return Some(Err(err));
        }
        self.buffer.pop_front().map(Ok)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.initialized {
            let len = self.buffer.len();
            (len, Some(len))
        } else {
            (0, None)
        }
    }

    fn boxed_clone(&self) -> Box<dyn SequenceCursor<N>> {
        Box::new(Self {
            vm: self.vm.clone(),
            lhs: self.lhs.as_ref().map(|c| c.boxed_clone()),
            rhs: self.rhs.as_ref().map(|c| c.boxed_clone()),
            kind: self.kind,
            buffer: self.buffer.clone(),
            initialized: self.initialized,
        })
    }
}

struct ForLoopCursor<N> {
    vm: VmHandle<N>,
    var: ExpandedName,
    body: InstrSeq,
    input: Box<dyn SequenceCursor<N>>,
    seed: Option<Box<dyn SequenceCursor<N>>>,
    input_len: Option<usize>,
    position: usize,
    current_output: Option<Box<dyn SequenceCursor<N>>>,
}

impl<N: 'static + Send + Sync + XdmNode + Clone> ForLoopCursor<N> {
    fn new(
        vm: VmHandle<N>,
        input_stream: XdmSequenceStream<N>,
        var: ExpandedName,
        body: InstrSeq,
    ) -> Self {
        let input = input_stream.cursor();
        let seed = Some(input.boxed_clone());
        Self { vm, var, body, input, seed, input_len: None, position: 0, current_output: None }
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
                let mut cursor = self.seed.take().unwrap();
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

impl<N: 'static + Send + Sync + XdmNode + Clone> SequenceCursor<N> for ForLoopCursor<N> {
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

            let last = match self.ensure_input_len() {
                Ok(v) => v,
                Err(err) => return Some(Err(err)),
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
        })
    }
}

struct QuantLoopCursor<N> {
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
}

impl<N: 'static + Send + Sync + XdmNode + Clone> QuantLoopCursor<N> {
    fn new(
        vm: VmHandle<N>,
        input_stream: XdmSequenceStream<N>,
        kind: QuantifierKind,
        var: ExpandedName,
        body: InstrSeq,
    ) -> Self {
        let input = input_stream.cursor();
        let seed = Some(input.boxed_clone());
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
                let mut cursor = self.seed.take().unwrap();
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
        let total = self.ensure_input_len()?;
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

impl<N: 'static + Send + Sync + XdmNode + Clone> SequenceCursor<N> for QuantLoopCursor<N> {
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
        })
    }
}
#[derive(Clone, Debug)]
struct Frame {
    last: usize,
    pos: usize,
}

impl<N: 'static + Send + Sync + XdmNode + Clone> Vm<N> {
    fn new(compiled: Arc<CompiledXPath>, dyn_ctx: Arc<DynamicContext<N>>) -> Self {
        // Resolve default collation once per VM (dynamic takes precedence over static)
        let default_collation = {
            let reg = &dyn_ctx.collations;
            if let Some(uri) = &dyn_ctx.default_collation {
                reg.get(uri)
            } else if let Some(uri) = &compiled.static_ctx.default_collation {
                reg.get(uri)
            } else {
                None
            }
        };
        let functions = dyn_ctx.provide_functions();
        let current_context_item = dyn_ctx.context_item.clone();
        let cancel_flag = dyn_ctx.cancel_flag.clone();
        Self {
            compiled,
            dyn_ctx,
            stack: SmallVec::new(),
            local_vars: SmallVec::new(),
            frames: SmallVec::new(),
            default_collation,
            functions,
            current_context_item,
            axis_buffer: SmallVec::new(),
            cancel_flag,
            set_fallback: SmallVec::new(),
        }
    }

    fn run(&mut self, code: &InstrSeq) -> Result<XdmSequenceStream<N>, Error> {
        let stack_base = self.stack.len();
        self.execute(code)?;
        let result = self.stack.pop().unwrap_or_default();
        debug_assert_eq!(self.stack.len(), stack_base);
        Ok(result)
    }

    fn snapshot(&self) -> VmSnapshot<N> {
        VmSnapshot {
            compiled: Arc::clone(&self.compiled),
            dyn_ctx: Arc::clone(&self.dyn_ctx),
            local_vars: self.local_vars.clone(),
            frames: self.frames.clone(),
            default_collation: self.default_collation.as_ref().map(Arc::clone),
            functions: Arc::clone(&self.functions),
            current_context_item: self.current_context_item.clone(),
        }
    }

    fn handle(&self) -> VmHandle<N> {
        VmHandle::new(self.snapshot(), self.cancel_flag.clone())
    }

    fn from_snapshot(snapshot: &VmSnapshot<N>) -> Self {
        Self {
            compiled: Arc::clone(&snapshot.compiled),
            dyn_ctx: Arc::clone(&snapshot.dyn_ctx),
            stack: SmallVec::new(),
            local_vars: snapshot.local_vars.clone(),
            frames: snapshot.frames.clone(),
            default_collation: snapshot.default_collation.as_ref().map(Arc::clone),
            functions: Arc::clone(&snapshot.functions),
            current_context_item: snapshot.current_context_item.clone(),
            axis_buffer: SmallVec::new(),
            cancel_flag: snapshot.dyn_ctx.cancel_flag.clone(),
            set_fallback: SmallVec::new(),
        }
    }

    fn reset_to_snapshot(&mut self, snapshot: &VmSnapshot<N>) {
        self.compiled = Arc::clone(&snapshot.compiled);
        self.dyn_ctx = Arc::clone(&snapshot.dyn_ctx);
        self.local_vars = snapshot.local_vars.clone();
        self.frames = snapshot.frames.clone();
        self.default_collation = snapshot.default_collation.as_ref().map(Arc::clone);
        self.functions = Arc::clone(&snapshot.functions);
        self.current_context_item = snapshot.current_context_item.clone();
        self.stack.clear();
        self.axis_buffer.clear();
        self.cancel_flag = snapshot.dyn_ctx.cancel_flag.clone();
        self.set_fallback.clear();
    }

    fn push_seq(&mut self, seq: XdmSequence<N>) {
        self.stack.push(XdmSequenceStream::from_vec(seq));
    }

    fn push_stream(&mut self, seq: XdmSequenceStream<N>) {
        self.stack.push(seq);
    }

    fn pop_stream(&mut self) -> XdmSequenceStream<N> {
        self.stack.pop().unwrap_or_default()
    }

    fn pop_seq(&mut self) -> Result<XdmSequence<N>, Error> {
        self.pop_stream().materialize()
    }

    fn check_cancel(&self) -> Result<(), Error> {
        if self.cancel_flag.as_ref().is_some_and(|flag| flag.load(AtomicOrdering::Relaxed)) {
            return Err(Error::from_code(ErrorCode::FOER0000, "evaluation cancelled"));
        }
        Ok(())
    }

    fn singleton_atomic_from_stream(
        &self,
        stream: XdmSequenceStream<N>,
    ) -> Result<XdmAtomicValue, Error> {
        use crate::xdm::XdmItem;
        let mut c = stream.cursor();
        let first = c.next_item().ok_or_else(|| {
            Error::from_code(ErrorCode::FORG0006, "expected singleton atomic; sequence is empty")
        })??;
        let atom = match first {
            XdmItem::Atomic(a) => a,
            XdmItem::Node(n) => {
                let tv = n.typed_value();
                if tv.len() != 1 {
                    return Err(Error::from_code(
                        ErrorCode::FORG0006,
                        "expected singleton atomic; node atomization not singleton",
                    ));
                }
                tv.into_iter().next().unwrap()
            }
        };
        if c.next_item().is_some() {
            return Err(Error::from_code(
                ErrorCode::FORG0006,
                "expected singleton atomic; sequence has more than one item",
            ));
        }
        Ok(atom)
    }

    fn first_node_from_stream(&self, stream: XdmSequenceStream<N>) -> Option<N> {
        use crate::xdm::XdmItem;
        let mut c = stream.cursor();
        match c.next_item()? {
            Ok(XdmItem::Node(n)) => Some(n),
            _ => None,
        }
    }

    fn to_number_stream(&self, stream: XdmSequenceStream<N>) -> Result<f64, Error> {
        use crate::xdm::XdmItem;
        let mut c = stream.cursor();
        match c.next_item() {
            None => Ok(f64::NAN),
            Some(Ok(XdmItem::Atomic(a))) => Self::atomic_to_number(&a),
            Some(Ok(XdmItem::Node(_))) => Ok(f64::NAN),
            Some(Err(e)) => Err(e),
        }
    }

    fn apply_predicates_stream(
        &self,
        stream: XdmSequenceStream<N>,
        predicates: &[InstrSeq],
    ) -> XdmSequenceStream<N> {
        if predicates.is_empty() {
            return stream;
        }
        let handle = self.handle();
        predicates.iter().fold(stream, |current, pred| {
            let cursor = PredicateCursor::new(handle.clone(), pred.clone(), current.cursor());
            XdmSequenceStream::new(cursor)
        })
    }

    fn doc_order_distinct_stream(&self, stream: XdmSequenceStream<N>) -> XdmSequenceStream<N> {
        let cursor = DocOrderDistinctCursor::new(self.handle(), stream.cursor());
        XdmSequenceStream::new(cursor)
    }

    fn set_operation_stream(
        &self,
        lhs: XdmSequenceStream<N>,
        rhs: XdmSequenceStream<N>,
        kind: SetOpKind,
    ) -> XdmSequenceStream<N> {
        let cursor = SetOperationCursor::new(self.handle(), kind, lhs.cursor(), rhs.cursor());
        XdmSequenceStream::new(cursor)
    }

    fn execute(&mut self, code: &InstrSeq) -> Result<(), Error> {
        let mut ip: usize = 0;
        let ops = &code.0;
        while ip < ops.len() {
            self.check_cancel()?;
            match &ops[ip] {
                // Data and variables
                OpCode::PushAtomic(a) => {
                    self.push_seq(vec![XdmItem::Atomic(a.clone())]);
                    ip += 1;
                }
                OpCode::LoadVarByName(name) => {
                    if let Some((_, v)) = self.local_vars.iter().rev().find(|(n, _)| n == name) {
                        self.push_stream(v.clone());
                    } else {
                        let v = self.dyn_ctx.variable(name).unwrap_or_default();
                        self.push_seq(v);
                    }
                    ip += 1;
                }
                OpCode::LoadContextItem => {
                    match &self.current_context_item {
                        Some(it) => self.push_seq(vec![it.clone()]),
                        None => self.push_seq(Vec::new()),
                    }
                    ip += 1;
                }
                OpCode::Position => {
                    let v = self.frames.last().map(|f| f.pos).unwrap_or(0) as i64;
                    self.push_seq(vec![XdmItem::Atomic(XdmAtomicValue::Integer(v))]);
                    ip += 1;
                }
                OpCode::Last => {
                    let v = self.frames.last().map(|f| f.last).unwrap_or(0) as i64;
                    self.push_seq(vec![XdmItem::Atomic(XdmAtomicValue::Integer(v))]);
                    ip += 1;
                }
                OpCode::ToRoot => {
                    // Navigate from current context item to root via parent() chain
                    let root = match &self.current_context_item {
                        Some(XdmItem::Node(n)) => {
                            let mut cur = n.clone();
                            let mut parent_opt = cur.parent();
                            while let Some(p) = parent_opt {
                                cur = p.clone();
                                parent_opt = cur.parent();
                            }
                            vec![XdmItem::Node(cur)]
                        }
                        _ => Vec::new(),
                    };
                    self.push_seq(root);
                    ip += 1;
                }

                // Stack helpers
                OpCode::Dup => {
                    let top = self.stack.last().cloned().unwrap_or_default();
                    self.push_stream(top);
                    ip += 1;
                }
                OpCode::Swap => {
                    let len = self.stack.len();
                    if len >= 2 {
                        self.stack.swap(len - 1, len - 2);
                    }
                    ip += 1;
                }

                // Steps / filters
                OpCode::AxisStep(axis, test, pred_ir) => {
                    let input_stream = self.pop_stream();
                    let axis_cursor = AxisStepCursor::new(
                        self.handle(),
                        input_stream,
                        axis.clone(),
                        test.clone(),
                    );
                    let axis_stream = XdmSequenceStream::new(axis_cursor);
                    let filtered_stream = self.apply_predicates_stream(axis_stream, pred_ir);
                    self.push_stream(filtered_stream);
                    ip += 1;
                }
                OpCode::PathExprStep(step_ir) => {
                    let input_stream = self.pop_stream();
                    let cursor = PathStepCursor::new(self.handle(), input_stream, step_ir.clone());
                    self.push_stream(XdmSequenceStream::new(cursor));
                    ip += 1;
                }
                OpCode::ApplyPredicates(preds) => {
                    let input_stream = self.pop_stream();
                    let filtered = self.apply_predicates_stream(input_stream, preds);
                    self.push_stream(filtered);
                    ip += 1;
                }
                OpCode::DocOrderDistinct => {
                    let stream = self.pop_stream();
                    let ordered = self.doc_order_distinct_stream(stream);
                    self.push_stream(ordered);
                    ip += 1;
                }
                OpCode::DocOrderDistinctOptimistic => {
                    // Fast path: assume input is already in document order and duplicate-free → no-op
                    // Still pop and push to keep stack discipline identical to the non-optimistic path.
                    let stream = self.pop_stream();
                    self.push_stream(stream);
                    ip += 1;
                }

                // Arithmetic / logic
                OpCode::Add
                | OpCode::Sub
                | OpCode::Mul
                | OpCode::Div
                | OpCode::IDiv
                | OpCode::Mod => {
                    use XdmAtomicValue as V;
                    // Streamed singleton-atomics (with atomization of nodes)
                    let rhs_stream = self.pop_stream();
                    let rhs_atom = self.singleton_atomic_from_stream(rhs_stream)?;
                    let lhs_stream = self.pop_stream();
                    let lhs_atom = self.singleton_atomic_from_stream(lhs_stream)?;
                    let (mut a, mut b) = (lhs_atom, rhs_atom);

                    // Handle temporal arithmetic and duration ops before numeric normalization
                    // Supported:
                    // - dateTime ± dayTimeDuration
                    // - date ± yearMonthDuration (with day saturation)
                    // - duration ± duration (same family)
                    // - duration * number | number * duration
                    // - duration div number
                    // - yearMonthDuration div yearMonthDuration -> double
                    // - dayTimeDuration div dayTimeDuration -> double
                    let op = &ops[ip];
                    // Helper: add months to NaiveDate saturating day to end of month
                    fn add_months_saturating(
                        date: chrono::NaiveDate,
                        delta_months: i32,
                    ) -> chrono::NaiveDate {
                        use chrono::{Datelike, NaiveDate};
                        let y = date.year();
                        let m = date.month() as i32; // 1-12
                        let total = y
                            .checked_mul(12)
                            .unwrap_or(0)
                            .checked_add(m - 1)
                            .unwrap_or(0)
                            .checked_add(delta_months)
                            .unwrap_or(0);
                        let ny = total.div_euclid(12);
                        let nm0 = total.rem_euclid(12);
                        let nm = (nm0 + 1) as u32; // 1..=12
                        // compute last day of target month
                        let last_day = match nm {
                            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                            4 | 6 | 9 | 11 => 30,
                            2 => {
                                let leap = (ny % 4 == 0 && ny % 100 != 0) || (ny % 400 == 0);
                                if leap { 29 } else { 28 }
                            }
                            _ => 30,
                        } as u32;
                        let day = date.day().min(last_day);
                        NaiveDate::from_ymd_opt(ny, nm, day).unwrap()
                    }

                    // Numeric value for a if numeric, else None
                    let classify_numeric = |v: &V| -> Option<f64> {
                        match v {
                            V::Integer(i) => Some(*i as f64),
                            V::Decimal(d) => Some(*d),
                            V::Double(d) => Some(*d),
                            V::Float(f) => Some(*f as f64),
                            _ => None,
                        }
                    };

                    // duration * number and friends
                    let handled_temporal = match op {
                        OpCode::Add => {
                            match (&a, &b) {
                                (V::DateTime(dt), V::DayTimeDuration(secs)) => {
                                    let ndt = *dt + ChronoDuration::seconds(*secs);
                                    self.push_seq(vec![XdmItem::Atomic(V::DateTime(ndt))]);
                                    ip += 1;
                                    true
                                }
                                (V::DayTimeDuration(secs), V::DateTime(dt)) => {
                                    let ndt = *dt + ChronoDuration::seconds(*secs);
                                    self.push_seq(vec![XdmItem::Atomic(V::DateTime(ndt))]);
                                    ip += 1;
                                    true
                                }
                                (V::Date { date, tz }, V::YearMonthDuration(months)) => {
                                    let nd = add_months_saturating(*date, *months);
                                    self.push_seq(vec![XdmItem::Atomic(V::Date {
                                        date: nd,
                                        tz: *tz,
                                    })]);
                                    ip += 1;
                                    true
                                }
                                (V::YearMonthDuration(months), V::Date { date, tz }) => {
                                    let nd = add_months_saturating(*date, *months);
                                    self.push_seq(vec![XdmItem::Atomic(V::Date {
                                        date: nd,
                                        tz: *tz,
                                    })]);
                                    ip += 1;
                                    true
                                }
                                (V::DateTime(dt), V::YearMonthDuration(months)) => {
                                    // Apply months with day saturation preserving time and timezone offset
                                    let date_part = dt.naive_utc().date();
                                    let nd = add_months_saturating(date_part, *months);
                                    let naive_with_time = nd.and_time(dt.time());
                                    let ndt = dt
                                        .offset()
                                        .from_local_datetime(&naive_with_time)
                                        .single()
                                        .unwrap_or_else(|| {
                                            chrono::DateTime::from_naive_utc_and_offset(
                                                naive_with_time,
                                                *dt.offset(),
                                            )
                                        });
                                    self.push_seq(vec![XdmItem::Atomic(V::DateTime(ndt))]);
                                    ip += 1;
                                    true
                                }
                                (V::YearMonthDuration(months), V::DateTime(dt)) => {
                                    let date_part = dt.naive_utc().date();
                                    let nd = add_months_saturating(date_part, *months);
                                    let naive_with_time = nd.and_time(dt.time());
                                    let ndt = dt
                                        .offset()
                                        .from_local_datetime(&naive_with_time)
                                        .single()
                                        .unwrap_or_else(|| {
                                            chrono::DateTime::from_naive_utc_and_offset(
                                                naive_with_time,
                                                *dt.offset(),
                                            )
                                        });
                                    self.push_seq(vec![XdmItem::Atomic(V::DateTime(ndt))]);
                                    ip += 1;
                                    true
                                }
                                (V::YearMonthDuration(a_m), V::YearMonthDuration(b_m)) => {
                                    self.push_seq(vec![XdmItem::Atomic(V::YearMonthDuration(
                                        *a_m + *b_m,
                                    ))]);
                                    ip += 1;
                                    true
                                }
                                (V::DayTimeDuration(a_s), V::DayTimeDuration(b_s)) => {
                                    self.push_seq(vec![XdmItem::Atomic(V::DayTimeDuration(
                                        *a_s + *b_s,
                                    ))]);
                                    ip += 1;
                                    true
                                }
                                _ => false,
                            }
                        }
                        OpCode::Sub => match (&a, &b) {
                            (V::DateTime(dt), V::DayTimeDuration(secs)) => {
                                let ndt = *dt - ChronoDuration::seconds(*secs);
                                self.push_seq(vec![XdmItem::Atomic(V::DateTime(ndt))]);
                                ip += 1;
                                true
                            }
                            (V::Date { date, tz }, V::YearMonthDuration(months)) => {
                                let nd = add_months_saturating(*date, -*months);
                                self.push_seq(vec![XdmItem::Atomic(V::Date { date: nd, tz: *tz })]);
                                ip += 1;
                                true
                            }
                            (V::DateTime(dt), V::YearMonthDuration(months)) => {
                                let date_part = dt.naive_utc().date();
                                let nd = add_months_saturating(date_part, -*months);
                                let naive_with_time = nd.and_time(dt.time());
                                let ndt = dt
                                    .offset()
                                    .from_local_datetime(&naive_with_time)
                                    .single()
                                    .unwrap_or_else(|| {
                                        chrono::DateTime::from_naive_utc_and_offset(
                                            naive_with_time,
                                            *dt.offset(),
                                        )
                                    });
                                self.push_seq(vec![XdmItem::Atomic(V::DateTime(ndt))]);
                                ip += 1;
                                true
                            }
                            (V::DateTime(da), V::DateTime(db)) => {
                                let diff = (*da - *db).num_seconds();
                                self.push_seq(vec![XdmItem::Atomic(V::DayTimeDuration(diff))]);
                                ip += 1;
                                true
                            }
                            (V::YearMonthDuration(a_m), V::YearMonthDuration(b_m)) => {
                                self.push_seq(vec![XdmItem::Atomic(V::YearMonthDuration(
                                    *a_m - *b_m,
                                ))]);
                                ip += 1;
                                true
                            }
                            (V::DayTimeDuration(a_s), V::DayTimeDuration(b_s)) => {
                                self.push_seq(vec![XdmItem::Atomic(V::DayTimeDuration(
                                    *a_s - *b_s,
                                ))]);
                                ip += 1;
                                true
                            }
                            _ => false,
                        },
                        OpCode::Mul => {
                            // duration * number or number * duration
                            match (&a, &b) {
                                (V::DayTimeDuration(secs), _) => {
                                    if let Some(n) = classify_numeric(&b) {
                                        let v = (*secs as f64 * n).trunc() as i64;
                                        self.push_seq(vec![XdmItem::Atomic(V::DayTimeDuration(v))]);
                                        ip += 1;
                                        true
                                    } else {
                                        false
                                    }
                                }
                                (V::YearMonthDuration(months), _) => {
                                    if let Some(n) = classify_numeric(&b) {
                                        let v = (*months as f64 * n).trunc() as i32;
                                        self.push_seq(vec![XdmItem::Atomic(V::YearMonthDuration(
                                            v,
                                        ))]);
                                        ip += 1;
                                        true
                                    } else {
                                        false
                                    }
                                }
                                (_, V::DayTimeDuration(secs)) => {
                                    if let Some(n) = classify_numeric(&a) {
                                        let v = (*secs as f64 * n).trunc() as i64;
                                        self.push_seq(vec![XdmItem::Atomic(V::DayTimeDuration(v))]);
                                        ip += 1;
                                        true
                                    } else {
                                        false
                                    }
                                }
                                (_, V::YearMonthDuration(months)) => {
                                    if let Some(n) = classify_numeric(&a) {
                                        let v = (*months as f64 * n).trunc() as i32;
                                        self.push_seq(vec![XdmItem::Atomic(V::YearMonthDuration(
                                            v,
                                        ))]);
                                        ip += 1;
                                        true
                                    } else {
                                        false
                                    }
                                }
                                _ => false,
                            }
                        }
                        OpCode::Div => match (&a, &b) {
                            (V::YearMonthDuration(a_m), V::YearMonthDuration(b_m)) => {
                                if *b_m == 0 {
                                    return Err(Error::from_code(
                                        ErrorCode::FOAR0001,
                                        "divide by zero",
                                    ));
                                }
                                let v = *a_m as f64 / *b_m as f64;
                                self.push_seq(vec![XdmItem::Atomic(V::Double(v))]);
                                ip += 1;
                                true
                            }
                            (V::DayTimeDuration(a_s), V::DayTimeDuration(b_s)) => {
                                if *b_s == 0 {
                                    return Err(Error::from_code(
                                        ErrorCode::FOAR0001,
                                        "divide by zero",
                                    ));
                                }
                                let v = *a_s as f64 / *b_s as f64;
                                self.push_seq(vec![XdmItem::Atomic(V::Double(v))]);
                                ip += 1;
                                true
                            }
                            (V::YearMonthDuration(months), _) => {
                                if let Some(n) = classify_numeric(&b) {
                                    if n == 0.0 {
                                        return Err(Error::from_code(
                                            ErrorCode::FOAR0001,
                                            "divide by zero",
                                        ));
                                    }
                                    let v = (*months as f64 / n).trunc() as i32;
                                    self.push_seq(vec![XdmItem::Atomic(V::YearMonthDuration(v))]);
                                    ip += 1;
                                    true
                                } else {
                                    false
                                }
                            }
                            (V::DayTimeDuration(secs), _) => {
                                if let Some(n) = classify_numeric(&b) {
                                    if n == 0.0 {
                                        return Err(Error::from_code(
                                            ErrorCode::FOAR0001,
                                            "divide by zero",
                                        ));
                                    }
                                    let v = (*secs as f64 / n).trunc() as i64;
                                    self.push_seq(vec![XdmItem::Atomic(V::DayTimeDuration(v))]);
                                    ip += 1;
                                    true
                                } else {
                                    false
                                }
                            }
                            _ => false,
                        },
                        OpCode::IDiv | OpCode::Mod => false, // not supported for durations
                        _ => false,
                    };
                    if handled_temporal {
                        continue;
                    }
                    // Normalize untypedAtomic: must be numeric for arithmetic. Invalid lexical -> FORG0001.
                    let norm_untyped = |v: &V| -> Result<V, Error> {
                        Ok(match v {
                            V::UntypedAtomic(s) => match s.parse::<f64>() {
                                Ok(num) => V::Double(num),
                                Err(_) => {
                                    return Err(Error::from_code(
                                        ErrorCode::FORG0001,
                                        "invalid numeric literal for arithmetic",
                                    ));
                                }
                            },
                            _ => v.clone(),
                        })
                    };
                    a = norm_untyped(&a)?;
                    b = norm_untyped(&b)?;

                    // Classification + minimal numeric promotion (duplicated small helper from compare_atomic)
                    #[derive(Clone, Copy)]
                    enum NumKind {
                        Int(i64),
                        Dec(f64),
                        Float(f32),
                        Double(f64),
                    }
                    impl NumKind {
                        fn to_f64(self) -> f64 {
                            match self {
                                NumKind::Int(i) => i as f64,
                                NumKind::Dec(d) => d,
                                NumKind::Float(f) => f as f64,
                                NumKind::Double(d) => d,
                            }
                        }
                    }
                    fn classify(v: &V) -> Option<NumKind> {
                        match v {
                            V::Integer(i) => Some(NumKind::Int(*i)),
                            V::Decimal(d) => Some(NumKind::Dec(*d)),
                            V::Float(f) => Some(NumKind::Float(*f)),
                            V::Double(d) => Some(NumKind::Double(*d)),
                            _ => None,
                        }
                    }
                    fn unify_numeric(a: NumKind, b: NumKind) -> (NumKind, NumKind) {
                        use NumKind::*;
                        match (a, b) {
                            (Double(x), y) => (Double(x), Double(y.to_f64())),
                            (y, Double(x)) => (Double(y.to_f64()), Double(x)),
                            (Float(x), Float(y)) => (Float(x), Float(y)),
                            (Float(x), Int(y)) => (Float(x), Float(y as f32)),
                            (Int(x), Float(y)) => (Float(x as f32), Float(y)),
                            (Float(x), Dec(y)) => (Float(x), Float(y as f32)),
                            (Dec(x), Float(y)) => (Float(x as f32), Float(y)),
                            (Dec(x), Dec(y)) => (Dec(x), Dec(y)),
                            (Dec(x), Int(y)) => (Dec(x), Dec(y as f64)),
                            (Int(x), Dec(y)) => (Dec(x as f64), Dec(y)),
                            (Int(x), Int(y)) => (Int(x), Int(y)),
                        }
                    }

                    let (ka, kb) = match (classify(&a), classify(&b)) {
                        (Some(x), Some(y)) => (x, y),
                        _ => {
                            return Err(Error::from_code(
                                ErrorCode::XPTY0004,
                                "non-numeric operand",
                            ));
                        }
                    };
                    let (ua, ub) = unify_numeric(ka, kb);

                    // Determine promoted result "kind" (excluding operation-specific adjustments)
                    use NumKind::*;
                    let promoted_kind = match (ua, ub) {
                        (Double(_), _) | (_, Double(_)) => Double(0.0),
                        (Float(_), _) | (_, Float(_)) => Float(0.0),
                        (Dec(_), _) | (_, Dec(_)) => Dec(0.0),
                        (Int(_), Int(_)) => Int(0),
                    };

                    // Integer-specialized path: when both operands are Int, prefer exact i128 arithmetic
                    // with lazy promotion to decimal on overflow. Only emit FOAR0002 where no representable
                    // promotion exists (e.g., idiv result beyond i64 range which must be xs:integer).
                    let mut pushed = false;
                    if matches!((ua, ub), (Int(_), Int(_))) {
                        let (ai, bi) = match (ua, ub) {
                            (Int(x), Int(y)) => (x as i128, y as i128),
                            _ => unreachable!(),
                        };
                        match &ops[ip] {
                            OpCode::Add => {
                                if let Some(sum) = ai.checked_add(bi) {
                                    if sum >= i64::MIN as i128 && sum <= i64::MAX as i128 {
                                        self.push_seq(vec![XdmItem::Atomic(V::Integer(
                                            sum as i64,
                                        ))]);
                                    } else {
                                        self.push_seq(vec![XdmItem::Atomic(V::Decimal(
                                            sum as f64,
                                        ))]);
                                    }
                                    ip += 1;
                                    pushed = true;
                                } else {
                                    // i128 overflow (extremely rare) → promote to decimal
                                    self.push_seq(vec![XdmItem::Atomic(V::Decimal(
                                        (ai as f64) + (bi as f64),
                                    ))]);
                                    ip += 1;
                                    pushed = true;
                                }
                            }
                            OpCode::Sub => {
                                if let Some(diff) = ai.checked_sub(bi) {
                                    if diff >= i64::MIN as i128 && diff <= i64::MAX as i128 {
                                        self.push_seq(vec![XdmItem::Atomic(V::Integer(
                                            diff as i64,
                                        ))]);
                                    } else {
                                        self.push_seq(vec![XdmItem::Atomic(V::Decimal(
                                            diff as f64,
                                        ))]);
                                    }
                                    ip += 1;
                                    pushed = true;
                                } else {
                                    self.push_seq(vec![XdmItem::Atomic(V::Decimal(
                                        (ai as f64) - (bi as f64),
                                    ))]);
                                    ip += 1;
                                    pushed = true;
                                }
                            }
                            OpCode::Mul => {
                                if let Some(prod) = ai.checked_mul(bi) {
                                    if prod >= i64::MIN as i128 && prod <= i64::MAX as i128 {
                                        self.push_seq(vec![XdmItem::Atomic(V::Integer(
                                            prod as i64,
                                        ))]);
                                    } else {
                                        self.push_seq(vec![XdmItem::Atomic(V::Decimal(
                                            prod as f64,
                                        ))]);
                                    }
                                    ip += 1;
                                    pushed = true;
                                } else {
                                    self.push_seq(vec![XdmItem::Atomic(V::Decimal(
                                        (ai as f64) * (bi as f64),
                                    ))]);
                                    ip += 1;
                                    pushed = true;
                                }
                            }
                            OpCode::IDiv => {
                                if bi == 0 {
                                    return Err(Error::from_code(
                                        ErrorCode::FOAR0001,
                                        "idiv by zero",
                                    ));
                                }
                                // floor division semantics
                                let q_trunc = ai / bi; // trunc toward 0
                                let r = ai % bi;
                                let needs_adjust = (r != 0) && ((ai ^ bi) < 0);
                                let q_floor = if needs_adjust { q_trunc - 1 } else { q_trunc };
                                if q_floor >= i64::MIN as i128 && q_floor <= i64::MAX as i128 {
                                    self.push_seq(vec![XdmItem::Atomic(V::Integer(
                                        q_floor as i64,
                                    ))]);
                                } else {
                                    // xs:integer result cannot be represented by our i64 storage → FOAR0002
                                    return Err(Error::from_code(
                                        ErrorCode::FOAR0002,
                                        "idiv result overflows xs:integer range",
                                    ));
                                }
                                ip += 1;
                                pushed = true;
                            }
                            OpCode::Mod => {
                                if bi == 0 {
                                    return Err(Error::from_code(
                                        ErrorCode::FOAR0001,
                                        "mod by zero",
                                    ));
                                }
                                // XPath mod defined as a - b*floor(a/b); for integers we can mirror via arithmetic
                                let q_trunc = ai / bi;
                                let r_trunc = ai % bi;
                                let needs_adjust = (r_trunc != 0) && ((ai ^ bi) < 0);
                                let q_floor = if needs_adjust { q_trunc - 1 } else { q_trunc };
                                let rem = ai - bi * q_floor;
                                // rem magnitude is < |bi|, thus guaranteed to fit into i64
                                self.push_seq(vec![XdmItem::Atomic(V::Integer(rem as i64))]);
                                ip += 1;
                                pushed = true;
                            }
                            OpCode::Div => {}
                            _ => {}
                        }
                    }
                    if pushed {
                        continue;
                    }

                    // Extract numeric primitives for calculation (generic floating/decimal path)
                    let (av_f64, bv_f64) = (ua.to_f64(), ub.to_f64());
                    // Operation semantics
                    let op = &ops[ip];
                    let result_value = match op {
                        OpCode::Add => av_f64 + bv_f64,
                        OpCode::Sub => av_f64 - bv_f64,
                        OpCode::Mul => av_f64 * bv_f64,
                        OpCode::Div => {
                            if bv_f64 == 0.0 {
                                match promoted_kind {
                                    // IEEE 754 semantics for float/double: produce ±INF or NaN
                                    NumKind::Double(_) | NumKind::Float(_) => av_f64 / bv_f64,
                                    // Decimal / Integer division by zero is an error per XPath 2.0
                                    _ => {
                                        return Err(Error::from_code(
                                            ErrorCode::FOAR0001,
                                            "divide by zero",
                                        ));
                                    }
                                }
                            } else {
                                av_f64 / bv_f64
                            }
                        }
                        OpCode::IDiv => {
                            if bv_f64 == 0.0 {
                                return Err(Error::from_code(ErrorCode::FOAR0001, "idiv by zero"));
                            }
                            // floor division per spec (handles negatives correctly)
                            (av_f64 / bv_f64).floor()
                        }
                        OpCode::Mod => {
                            if bv_f64 == 0.0 {
                                return Err(Error::from_code(ErrorCode::FOAR0001, "mod by zero"));
                            }
                            av_f64 % bv_f64
                        }
                        _ => unreachable!(),
                    };

                    // Determine result type (XPath 2.0 rules simplified):
                    // - idiv -> integer
                    // - div: if promoted integer -> decimal; if decimal -> decimal; float->float; double->double
                    // - add/sub/mul/mod -> promoted kind
                    let result_atomic = match op {
                        OpCode::IDiv => {
                            // Guard overflow: xs:integer result must fit our i64 storage
                            if !result_value.is_finite()
                                || result_value < i64::MIN as f64
                                || result_value > i64::MAX as f64
                            {
                                return Err(Error::from_code(
                                    ErrorCode::FOAR0002,
                                    "idiv result overflows xs:integer range",
                                ));
                            }
                            V::Integer(result_value as i64)
                        }
                        OpCode::Div => match promoted_kind {
                            Double(_) => V::Double(result_value),
                            Float(_) => V::Float(result_value as f32),
                            Dec(_) | Int(_) => V::Decimal(result_value), // integer division yields decimal
                        },
                        OpCode::Add | OpCode::Sub | OpCode::Mul => match promoted_kind {
                            Double(_) => V::Double(result_value),
                            Float(_) => V::Float(result_value as f32),
                            Dec(_) => V::Decimal(result_value),
                            Int(_) => {
                                // If exact integer keep integer else decimal (rare due to overflow/frac)
                                if (result_value.fract()).abs() < f64::EPSILON {
                                    V::Integer(result_value as i64)
                                } else {
                                    V::Decimal(result_value)
                                }
                            }
                        },
                        OpCode::Mod => match promoted_kind {
                            Double(_) => V::Double(result_value),
                            Float(_) => V::Float(result_value as f32),
                            Dec(_) => V::Decimal(result_value),
                            Int(_) => V::Integer(result_value as i64),
                        },
                        _ => unreachable!(),
                    };
                    self.push_seq(vec![XdmItem::Atomic(result_atomic)]);
                    ip += 1;
                }
                OpCode::And => {
                    let rhs_stream = self.pop_stream();
                    let lhs_stream = self.pop_stream();
                    let lhs_b = self.ebv_stream(lhs_stream.cursor())?;
                    if !lhs_b {
                        self.push_seq(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(false))]);
                        ip += 1;
                    } else {
                        let rhs_b = self.ebv_stream(rhs_stream.cursor())?;
                        self.push_seq(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(rhs_b))]);
                        ip += 1;
                    }
                }
                OpCode::Or => {
                    let rhs_stream = self.pop_stream();
                    let lhs_stream = self.pop_stream();
                    let lhs_b = self.ebv_stream(lhs_stream.cursor())?;
                    if lhs_b {
                        self.push_seq(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(true))]);
                        ip += 1;
                    } else {
                        let rhs_b = self.ebv_stream(rhs_stream.cursor())?;
                        self.push_seq(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(rhs_b))]);
                        ip += 1;
                    }
                }
                OpCode::Not => {
                    let v = self.pop_stream();
                    let b = !self.ebv_stream(v.cursor())?;
                    self.push_seq(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                    ip += 1;
                }
                OpCode::ToEBV => {
                    let v = self.pop_stream();
                    let b = self.ebv_stream(v.cursor())?;
                    self.push_seq(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                    ip += 1;
                }
                OpCode::Atomize => {
                    let v = self.pop_stream();
                    let cursor = AtomizeCursor::new(v);
                    self.push_stream(XdmSequenceStream::new(cursor));
                    ip += 1;
                }
                OpCode::Pop => {
                    let _ = self.stack.pop();
                    ip += 1;
                }
                OpCode::JumpIfTrue(delta) => {
                    let v = self.pop_stream();
                    let b = self.ebv_stream(v.cursor())?;
                    if b {
                        ip += 1 + *delta;
                    } else {
                        ip += 1;
                    }
                }
                OpCode::JumpIfFalse(delta) => {
                    let v = self.pop_stream();
                    let b = self.ebv_stream(v.cursor())?;
                    if !b {
                        ip += 1 + *delta;
                    } else {
                        ip += 1;
                    }
                }
                OpCode::Jump(delta) => {
                    ip += 1 + *delta;
                }

                // Comparisons
                OpCode::CompareValue(op) => {
                    // Value comparison ( =, !=, lt, etc. with 'value' grammar) expects each side to be a singleton.
                    // Streamed singleton atomics (with node atomization) and direct atomic comparison.
                    let rhs_stream = self.pop_stream();
                    let rhs_atom = self.singleton_atomic_from_stream(rhs_stream)?;
                    let lhs_stream = self.pop_stream();
                    let lhs_atom = self.singleton_atomic_from_stream(lhs_stream)?;
                    let b = self.compare_atomic(&lhs_atom, &rhs_atom, *op)?;
                    self.push_seq(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                    ip += 1;
                }
                OpCode::CompareGeneral(op) => {
                    // Streaming left, cached right atomics; early-exit on first match.
                    let rhs_stream = self.pop_stream();
                    let lhs_stream = self.pop_stream();

                    // Build RHS atom cache (atomize nodes inline)
                    let mut rhs_atoms: Vec<XdmAtomicValue> = Vec::new();
                    let mut rc = rhs_stream.cursor();
                    while let Some(item) = rc.next_item() {
                        match item? {
                            XdmItem::Atomic(a) => rhs_atoms.push(a),
                            XdmItem::Node(n) => rhs_atoms.extend(n.typed_value()),
                        }
                    }

                    let mut any_true = false;
                    let mut lc = lhs_stream.cursor();
                    'outer: while let Some(item) = lc.next_item() {
                        let item = item?;
                        match item {
                            XdmItem::Atomic(la) => {
                                for rb in &rhs_atoms {
                                    match self.compare_atomic(&la, rb, *op) {
                                        Ok(res) if res => { any_true = true; break 'outer; }
                                        Ok(_) => {}
                                        Err(e) => match e.code_enum() {
                                            ErrorCode::FORG0006 | ErrorCode::XPTY0004 => {}
                                            _ => return Err(e),
                                        },
                                    }
                                }
                            }
                            XdmItem::Node(n) => {
                                for la in n.typed_value() {
                                    for rb in &rhs_atoms {
                                        match self.compare_atomic(&la, rb, *op) {
                                            Ok(res) if res => { any_true = true; break 'outer; }
                                            Ok(_) => {}
                                            Err(e) => match e.code_enum() {
                                                ErrorCode::FORG0006 | ErrorCode::XPTY0004 => {}
                                                _ => return Err(e),
                                            },
                                        }
                                    }
                                }
                            }
                        }
                    }
                    self.push_seq(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(any_true))]);
                    ip += 1;
                }
                OpCode::NodeIs => {
                    let rhs_stream = self.pop_stream();
                    let rhs = self.first_node_from_stream(rhs_stream);
                    let lhs_stream = self.pop_stream();
                    let lhs = self.first_node_from_stream(lhs_stream);
                    let b = match (lhs, rhs) {
                        (Some(a), Some(b)) => a == b,
                        _ => false,
                    };
                    self.push_seq(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                    ip += 1;
                }
                OpCode::NodeBefore | OpCode::NodeAfter => {
                    let after = matches!(&ops[ip], OpCode::NodeAfter);
                    let rhs_stream = self.pop_stream();
                    let rhs = self.first_node_from_stream(rhs_stream);
                    let lhs_stream = self.pop_stream();
                    let lhs = self.first_node_from_stream(lhs_stream);
                    let b = match (lhs, rhs) {
                        (Some(a), Some(b)) => match a.compare_document_order(&b) {
                            Ok(ord) => if after { ord.is_gt() } else { ord.is_lt() },
                            Err(e) => return Err(e),
                        },
                        _ => false,
                    };
                    self.push_seq(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                    ip += 1;
                }

                // Sequences and sets
                OpCode::MakeSeq(n) => {
                    let n = *n;
                    let mut parts: Vec<XdmSequenceStream<N>> = Vec::with_capacity(n);
                    for _ in 0..n {
                        parts.push(self.pop_stream());
                    }
                    parts.reverse();
                    let mut iter = parts.into_iter();
                    if let Some(first) = iter.next() {
                        let chained = iter.fold(first, |acc, stream| acc.chain(stream));
                        self.push_stream(chained);
                    } else {
                        self.push_stream(XdmSequenceStream::empty());
                    }
                    ip += 1;
                }
                OpCode::ConcatSeq => {
                    let rhs = self.pop_stream();
                    let lhs = self.pop_stream();
                    self.push_stream(lhs.chain(rhs));
                    ip += 1;
                }
                OpCode::Union | OpCode::Intersect | OpCode::Except => {
                    let rhs_stream = self.pop_stream();
                    let lhs_stream = self.pop_stream();
                    let kind = match &ops[ip] {
                        OpCode::Union => SetOpKind::Union,
                        OpCode::Intersect => SetOpKind::Intersect,
                        OpCode::Except => SetOpKind::Except,
                        _ => unreachable!(),
                    };
                    let result = self.set_operation_stream(lhs_stream, rhs_stream, kind);
                    self.push_stream(result);
                    ip += 1;
                }
                OpCode::RangeTo => {
                    let end_stream = self.pop_stream();
                    let end = self.to_number_stream(end_stream)?;
                    let start_stream = self.pop_stream();
                    let start = self.to_number_stream(start_stream)?;
                    let a = start as i64;
                    let b = end as i64;
                    if a <= b {
                        self.push_stream(XdmSequenceStream::from_range_inclusive(a, b));
                    } else {
                        self.push_stream(XdmSequenceStream::empty());
                    }
                    ip += 1;
                }

                // Control flow / bindings (not fully supported)
                OpCode::BeginScope(_) | OpCode::EndScope => {
                    ip += 1;
                }
                OpCode::LetStartByName(var_name) => {
                    let value_stream = self.pop_stream();
                    self.local_vars.push((var_name.clone(), value_stream));
                    ip += 1;
                }
                OpCode::LetEnd => {
                    if self.local_vars.pop().is_none() {
                        return Err(Error::from_code(
                            ErrorCode::FOER0000,
                            "imbalanced let scope during evaluation",
                        ));
                    }
                    ip += 1;
                }
                OpCode::ForLoop { var, body } => {
                    let input_stream = self.pop_stream();
                    let cursor =
                        ForLoopCursor::new(self.handle(), input_stream, var.clone(), body.clone());
                    self.push_stream(XdmSequenceStream::new(cursor));
                    ip += 1;
                }
                OpCode::QuantLoop { kind, var, body } => {
                    let input_stream = self.pop_stream();
                    let cursor = QuantLoopCursor::new(
                        self.handle(),
                        input_stream,
                        *kind,
                        var.clone(),
                        body.clone(),
                    );
                    self.push_stream(XdmSequenceStream::new(cursor));
                    ip += 1;
                }

                // Types
                OpCode::Cast(t) => {
                    let stream = self.pop_stream();
                    // Enforce singleton/empty semantics without full materialization
                    let mut c = stream.cursor();
                    let first = match c.next_item() { Some(it) => it?, None => {
                        if t.optional { self.push_stream(XdmSequenceStream::empty()); ip += 1; continue; }
                        else { return Err(Error::from_code(ErrorCode::XPST0003, "empty not allowed")); }
                    }};
                    if c.next_item().is_some() { return Err(Error::from_code(ErrorCode::XPTY0004, "cast of multi-item")); }
                    let val = match first { XdmItem::Atomic(a) => a, XdmItem::Node(n) => XdmAtomicValue::UntypedAtomic(n.string_value()) };
                    let casted = self.cast_atomic(val, &t.atomic)?;
                    self.push_stream(XdmSequenceStream::from_item(XdmItem::Atomic(casted)));
                    ip += 1;
                }
                OpCode::Castable(t) => {
                    let stream = self.pop_stream();
                    let mut c = stream.cursor();
                    let ok = match c.next_item() {
                        None => t.optional,
                        Some(Err(e)) => return Err(e),
                        Some(Ok(first)) => {
                            // more than one item → false
                            if c.next_item().is_some() { false } else {
                                let val = match first { XdmItem::Atomic(a) => a, XdmItem::Node(n) => XdmAtomicValue::UntypedAtomic(n.string_value()) };
                                // QName castable requires prefix resolution in static context
                                if t.atomic.local == "QName" {
                                    if let XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) = &val {
                                        if let Some(idx) = s.find(':') {
                                            let p = &s[..idx];
                                            if p.is_empty() { false } else if p == "xml" { true } else {
                                                self.compiled.static_ctx.namespaces.by_prefix.contains_key(p)
                                            }
                                        } else { !s.is_empty() }
                                    } else { false }
                                } else {
                                    self.cast_atomic(val, &t.atomic).is_ok()
                                }
                            }
                        }
                    };
                    self.push_seq(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(ok))]);
                    ip += 1;
                }
                OpCode::Treat(t) => {
                    let input = self.pop_stream();
                    let cursor = TreatCursor::new(self.handle(), input, t.clone());
                    self.push_stream(XdmSequenceStream::new(cursor));
                    ip += 1;
                }
                OpCode::InstanceOf(t) => {
                    let stream = self.pop_stream();
                    let b = self.instance_of_stream(stream, t)?;
                    self.push_seq(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                    ip += 1;
                }

                // Functions
                OpCode::CallByName(name, argc) => {
                    let argc = *argc;
                    let mut args: Vec<XdmSequence<N>> = Vec::with_capacity(argc);
                    for _ in 0..argc {
                        args.push(self.pop_seq()?);
                    }
                    args.reverse();
                    let en = name; // lookup will resolve default namespace when needed
                    let def_ns = self.compiled.static_ctx.default_function_namespace.as_deref();
                    if let Some(specs) = self
                        .compiled
                        .static_ctx
                        .function_signatures
                        .param_types_for_call(en, argc, def_ns)
                    {
                        self.apply_function_conversions(&mut args, specs)?;
                    }
                    let f = match self.functions.resolve(en, argc, def_ns) {
                        Ok(f) => f,
                        Err(crate::engine::runtime::ResolveError::Unknown(resolved)) => {
                            return Err(Error::from_code(
                                ErrorCode::XPST0017,
                                format!("unknown function: {{{:?}}}#{argc}", resolved),
                            ));
                        }
                        Err(crate::engine::runtime::ResolveError::WrongArity {
                            name: resolved,
                            ..
                        }) => {
                            // Humanize the provided argument count for a clearer diagnostic
                            let arg_phrase = match argc {
                                0 => "no arguments".to_string(),
                                1 => "one argument".to_string(),
                                2 => "two arguments".to_string(),
                                3 => "three arguments".to_string(),
                                n => format!("{n} arguments"),
                            };
                            return Err(Error::from_code(
                                ErrorCode::XPST0017,
                                format!(
                                    "function {}() cannot be called with {}",
                                    resolved.local, arg_phrase
                                ),
                            ));
                        }
                    };
                    // Use cached default collation for this VM
                    let default_collation = self.default_collation.clone();
                    let call_ctx = CallCtx {
                        dyn_ctx: self.dyn_ctx.as_ref(),
                        static_ctx: &self.compiled.static_ctx,
                        default_collation,
                        regex: self.dyn_ctx.regex.clone(),
                        current_context_item: self.current_context_item.clone(),
                    };
                    let result = (f)(&call_ctx, &args)?;
                    self.push_seq(result);
                    ip += 1;
                }

                // Errors
                OpCode::Raise(code) => {
                    // Interpret legacy raise codes; prefer enum when possible.
                    return Err(Error::new_qname(Error::parse_code(code), "raised by program"));
                }
            }
        }
        Ok(())
    }

    fn apply_function_conversions(
        &self,
        args: &mut [XdmSequence<N>],
        specs: &[ParamTypeSpec],
    ) -> Result<(), Error> {
        for (idx, spec) in specs.iter().enumerate() {
            if let Some(arg) = args.get_mut(idx) {
                if spec.requires_atomization()
                    && arg.iter().any(|item| matches!(item, XdmItem::Node(_)))
                {
                    let taken = mem::take(arg);
                    *arg = Self::atomize(taken);
                }
                let converted =
                    spec.apply_to_sequence(mem::take(arg), &self.compiled.static_ctx)?;
                *arg = converted;
            }
        }
        Ok(())
    }

    fn eval_subprogram_stream(
        &mut self,
        code: &InstrSeq,
        context_item: Option<XdmItem<N>>,
        frame: Option<Frame>,
        extra_var: Option<(ExpandedName, XdmSequenceStream<N>)>,
    ) -> Result<XdmSequenceStream<N>, Error> {
        let saved_context = self.current_context_item.clone();
        let stack_base = self.stack.len();
        let locals_base = self.local_vars.len();
        let frames_base = self.frames.len();

        if let Some(frame) = frame {
            self.frames.push(frame);
        }
        if let Some((name, value)) = extra_var {
            self.local_vars.push((name, value));
        }
        self.current_context_item = context_item;

        let result = (|| {
            self.execute(code)?;
            let output = self.pop_stream();
            Ok(output)
        })();

        self.stack.truncate(stack_base);
        self.local_vars.truncate(locals_base);
        self.frames.truncate(frames_base);
        self.current_context_item = saved_context;

        result
    }

    fn ebv(seq: &XdmSequence<N>) -> Result<bool, Error> {
        match seq.len() {
            0 => Ok(false),
            1 => match &seq[0] {
                XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => Ok(*b),
                XdmItem::Atomic(XdmAtomicValue::String(s)) => Ok(!s.is_empty()),
                XdmItem::Atomic(XdmAtomicValue::Integer(i)) => Ok(*i != 0),
                XdmItem::Atomic(XdmAtomicValue::Decimal(d)) => Ok(*d != 0.0),
                XdmItem::Atomic(XdmAtomicValue::Double(d)) => Ok(*d != 0.0 && !d.is_nan()),
                XdmItem::Atomic(XdmAtomicValue::Float(f)) => Ok(*f != 0.0 && !f.is_nan()),
                XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => Ok(!s.is_empty()),
                XdmItem::Node(_) => Ok(true),
                _ => Err(Error::from_code(
                    ErrorCode::FORG0006,
                    "EBV for this atomic type not supported",
                )),
            },
            _ => {
                if seq.iter().all(|item| matches!(item, XdmItem::Node(_))) {
                    Ok(true)
                } else {
                    Err(Error::from_code(
                        ErrorCode::FORG0006,
                        "effective boolean value of sequence of length > 1",
                    ))
                }
            }
        }
    }

    fn ebv_stream(&self, mut cursor: Box<dyn SequenceCursor<N>>) -> Result<bool, Error> {
        use crate::xdm::XdmItem;
        use crate::xdm::XdmAtomicValue as V;
        let mut seen = 0usize;
        let mut first_atomic: Option<bool> = None;
        let mut saw_node_only = true;

        while let Some(item) = cursor.next_item() {
            let item = item?;
            seen = seen.saturating_add(1);
            match item {
                XdmItem::Node(_) => {
                    // keep scanning to ensure sequence does not contain atomics
                }
                XdmItem::Atomic(a) => {
                    saw_node_only = false;
                    if seen == 1 {
                        let ebv = match a {
                            V::Boolean(b) => b,
                            V::String(ref s) | V::UntypedAtomic(ref s) => !s.is_empty(),
                            V::Integer(i) => i != 0,
                            V::Decimal(d) => d != 0.0,
                            V::Double(d) => d != 0.0 && !d.is_nan(),
                            V::Float(f) => f != 0.0 && !f.is_nan(),
                            _ => {
                                return Err(Error::from_code(
                                    ErrorCode::FORG0006,
                                    "EBV for this atomic type not supported",
                                ));
                            }
                        };
                        first_atomic = Some(ebv);
                    } else {
                        return Err(Error::from_code(
                            ErrorCode::FORG0006,
                            "effective boolean value of sequence of length > 1",
                        ));
                    }
                }
            }
        }

        if seen == 0 {
            return Ok(false);
        }
        if saw_node_only {
            return Ok(true);
        }
        Ok(first_atomic.unwrap_or(false))
    }

    // XPath 2.0 predicate semantics:
    // - If result is a number: keep node iff number == position()
    // - Else: use EBV of the result

    fn predicate_truth_value_stream(
        &self,
        stream: XdmSequenceStream<N>,
        position: usize,
        _last: usize,
    ) -> Result<bool, Error> {
        use crate::xdm::XdmAtomicValue as A;
        let mut c = stream.cursor();
        let first = match c.next_item() { Some(it) => it?, None => return Ok(false) };
        match first {
            XdmItem::Node(_) => {
                // If we see a second node, EBV=true immediately; if we see an atomic afterward → error.
                match c.next_item() {
                    Some(Ok(XdmItem::Node(_))) => Ok(true),
                    Some(Ok(XdmItem::Atomic(_))) => Err(Error::from_code(
                        ErrorCode::FORG0006,
                        "effective boolean value of sequence of length > 1",
                    )),
                    Some(Err(e)) => Err(e),
                    None => Ok(true),
                }
            }
            XdmItem::Atomic(a) => {
                // Peek if there is a second item; if yes → EBV error (length>1 with atomics)
                if c.next_item().is_some() {
                    return Err(Error::from_code(
                        ErrorCode::FORG0006,
                        "effective boolean value of sequence of length > 1",
                    ));
                }
                // Singleton atomic: try numeric predicate special case first
                let num_opt = match &a {
                    A::Integer(i) => Some(*i as f64),
                    A::Decimal(d) => Some(*d),
                    A::Double(d) => Some(*d),
                    A::Float(f) => Some(*f as f64),
                    A::UntypedAtomic(s) => s.parse::<f64>().ok(),
                    _ => None,
                };
                if let Some(num) = num_opt {
                    if num.is_nan() { return Ok(false); }
                    return Ok((num - (position as f64)).abs() < f64::EPSILON);
                }
                // Otherwise EBV of singleton atomic
                Self::ebv(&vec![XdmItem::Atomic(a)])
            }
        }
    }

    fn atomize(seq: XdmSequence<N>) -> XdmSequence<N> {
        let mut out = Vec::with_capacity(seq.len());
        for it in seq {
            match it {
                XdmItem::Atomic(a) => out.push(XdmItem::Atomic(a)),
                XdmItem::Node(n) => {
                    for atom in n.typed_value() {
                        out.push(XdmItem::Atomic(atom));
                    }
                }
            }
        }
        out
    }

    #[allow(dead_code)]
    fn to_number(seq: &XdmSequence<N>) -> Result<f64, Error> {
        let aseq = Self::atomize(seq.clone());
        if aseq.is_empty() {
            return Ok(f64::NAN);
        }
        match &aseq[0] {
            XdmItem::Atomic(a) => Self::atomic_to_number(a),
            XdmItem::Node(_) => Ok(f64::NAN),
        }
    }

    fn atomic_to_number(a: &XdmAtomicValue) -> Result<f64, Error> {
        Ok(match a {
            XdmAtomicValue::Integer(i) => *i as f64,
            XdmAtomicValue::Decimal(d) => *d,
            XdmAtomicValue::Double(d) => *d,
            XdmAtomicValue::Float(f) => *f as f64,
            XdmAtomicValue::Boolean(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            XdmAtomicValue::UntypedAtomic(s) | XdmAtomicValue::String(s) => {
                s.parse::<f64>().unwrap_or(f64::NAN)
            }
            _ => f64::NAN,
        })
    }

    #[allow(dead_code)]
    fn compare_value(
        &self,
        lhs: &XdmSequence<N>,
        rhs: &XdmSequence<N>,
        op: ComparisonOp,
    ) -> Result<bool, Error> {
        let la = Self::atomize(lhs.clone());
        let ra = Self::atomize(rhs.clone());
        if la.len() != 1 || ra.len() != 1 {
            return Err(Error::from_code(
                ErrorCode::FORG0006,
                "value comparison requires singletons",
            ));
        }
        match (&la[0], &ra[0]) {
            (XdmItem::Atomic(a), XdmItem::Atomic(b)) => self.compare_atomic(a, b, op),
            _ => Ok(false),
        }
    }

    fn compare_atomic(
        &self,
        a: &XdmAtomicValue,
        b: &XdmAtomicValue,
        op: ComparisonOp,
    ) -> Result<bool, Error> {
        use ComparisonOp::*;
        // XPath 2.0 value comparison promotions (refined numeric path):
        // 1. untypedAtomic normalization (string or attempt numeric if other numeric)
        // 2. Numeric tower minimal promotion: integer + integer -> integer; integer + decimal -> decimal; decimal + float -> float;
        //    any + double -> double; float + float -> float; decimal + decimal -> decimal; integer + float -> float; etc.
        // 3. Boolean: only Eq/Ne allowed vs boolean; relational ops on booleans error
        // 4. String vs numeric relational is error (still simplified to FORG0006 here)
        use XdmAtomicValue as V;

        // Helper: determine unified numeric representation with minimal promotion.
        #[derive(Clone, Copy)]
        enum NumKind {
            Int(i64),
            Dec(f64),
            Float(f32),
            Double(f64),
        }
        impl NumKind {
            fn to_f64(self) -> f64 {
                match self {
                    NumKind::Int(i) => i as f64,
                    NumKind::Dec(d) => d,
                    NumKind::Float(f) => f as f64,
                    NumKind::Double(d) => d,
                }
            }
        }
        fn classify(v: &V) -> Option<NumKind> {
            match v {
                V::Integer(i) => Some(NumKind::Int(*i)),
                V::Decimal(d) => Some(NumKind::Dec(*d)),
                V::Float(f) => Some(NumKind::Float(*f)),
                V::Double(d) => Some(NumKind::Double(*d)),
                _ => None,
            }
        }
        fn unify_numeric(a: NumKind, b: NumKind) -> (NumKind, NumKind) {
            use NumKind::*;
            match (a, b) {
                (Double(x), y) => (Double(x), Double(y.to_f64())),
                (y, Double(x)) => (Double(y.to_f64()), Double(x)),
                (Float(x), Float(y)) => (Float(x), Float(y)),
                (Float(x), Int(y)) => (Float(x), Float(y as f32)),
                (Int(x), Float(y)) => (Float(x as f32), Float(y)),
                (Float(x), Dec(y)) => (Float(x), Float(y as f32)),
                (Dec(x), Float(y)) => (Float(x as f32), Float(y)),
                (Dec(x), Dec(y)) => (Dec(x), Dec(y)),
                (Dec(x), Int(y)) => (Dec(x), Dec(y as f64)),
                (Int(x), Dec(y)) => (Dec(x as f64), Dec(y)),
                (Int(x), Int(y)) => (Int(x), Int(y)),
            }
        }

        // Normalize untypedAtomic per context: if the counterpart is numeric attempt numeric cast (error on failure),
        // else treat both sides' untyped as string. untyped vs untyped -> both strings.
        let (a_norm, b_norm) = match (a, b) {
            (V::UntypedAtomic(sa), V::UntypedAtomic(sb)) => {
                (V::String(sa.clone()), V::String(sb.clone()))
            }
            (V::UntypedAtomic(s), other)
                if matches!(other, V::Integer(_) | V::Decimal(_) | V::Double(_) | V::Float(_)) =>
            {
                let num = s.parse::<f64>().map_err(|_| {
                    Error::from_code(ErrorCode::FORG0001, "invalid numeric literal")
                })?;
                (V::Double(num), other.clone())
            }
            (other, V::UntypedAtomic(s))
                if matches!(other, V::Integer(_) | V::Decimal(_) | V::Double(_) | V::Float(_)) =>
            {
                let num = s.parse::<f64>().map_err(|_| {
                    Error::from_code(ErrorCode::FORG0001, "invalid numeric literal")
                })?;
                (other.clone(), V::Double(num))
            }
            (V::UntypedAtomic(s), other) => (V::String(s.clone()), other.clone()),
            (other, V::UntypedAtomic(s)) => (other.clone(), V::String(s.clone())),
            _ => (a.clone(), b.clone()),
        };

        // Boolean handling
        if let (V::Boolean(x), V::Boolean(y)) = (&a_norm, &b_norm) {
            return Ok(match op {
                Eq => x == y,
                Ne => x != y,
                Lt | Le | Gt | Ge => {
                    return Err(Error::from_code(ErrorCode::XPTY0004, "relational op on boolean"));
                }
            });
        }

        // If both (after normalization) are strings and not numeric context
        if matches!((&a_norm, &b_norm), (V::String(_), V::String(_)))
            && matches!(op, Lt | Le | Gt | Ge | Eq | Ne)
        {
            let ls = if let V::String(s) = &a_norm { s } else { unreachable!() };
            let rs = if let V::String(s) = &b_norm { s } else { unreachable!() };
            // Collation-aware: use default collation (fallback to codepoint)
            let coll_arc;
            let coll: &dyn crate::engine::collation::Collation =
                if let Some(c) = &self.default_collation {
                    c.as_ref()
                } else {
                    coll_arc = self
                        .dyn_ctx
                        .collations
                        .get(crate::engine::collation::CODEPOINT_URI)
                        .expect("codepoint collation registered");
                    coll_arc.as_ref()
                };
            return Ok(match op {
                Eq => coll.key(ls) == coll.key(rs),
                Ne => coll.key(ls) != coll.key(rs),
                Lt => coll.compare(ls, rs).is_lt(),
                Le => {
                    let ord = coll.compare(ls, rs);
                    ord.is_lt() || ord.is_eq()
                }
                Gt => coll.compare(ls, rs).is_gt(),
                Ge => {
                    let ord = coll.compare(ls, rs);
                    ord.is_gt() || ord.is_eq()
                }
            });
        }

        // QName equality (only Eq/Ne permitted); compare namespace URI + local name; ignore prefix
        if let (
            XdmAtomicValue::QName { ns_uri: nsa, local: la, .. },
            XdmAtomicValue::QName { ns_uri: nsb, local: lb, .. },
        ) = (a, b)
        {
            return Ok(match op {
                Eq => nsa == nsb && la == lb,
                Ne => nsa != nsb || la != lb,
                Lt | Le | Gt | Ge => {
                    return Err(Error::from_code(ErrorCode::XPTY0004, "relational op on QName"));
                }
            });
        }

        // NOTATION equality (only Eq/Ne permitted); current engine treats NOTATION as lexical string
        if let (XdmAtomicValue::Notation(na), XdmAtomicValue::Notation(nb)) = (a, b) {
            return Ok(match op {
                Eq => na == nb,
                Ne => na != nb,
                Lt | Le | Gt | Ge => {
                    return Err(Error::from_code(ErrorCode::XPTY0004, "relational op on NOTATION"));
                }
            });
        }

        // Numeric path with minimal promotion
        if let (Some(ca), Some(cb)) = (classify(&a_norm), classify(&b_norm)) {
            let (ua, ub) = unify_numeric(ca, cb);
            let (ln, rn) = (ua.to_f64(), ub.to_f64());
            if ln.is_nan() || rn.is_nan() {
                return Ok(matches!(op, ComparisonOp::Ne));
            }
            return Ok(match op {
                Eq => ln == rn,
                Ne => ln != rn,
                Lt => ln < rn,
                Le => ln <= rn,
                Gt => ln > rn,
                Ge => ln >= rn,
            });
        }

        // dateTime relational comparisons by absolute instant
        if let (XdmAtomicValue::DateTime(da), XdmAtomicValue::DateTime(db)) = (a, b) {
            let (a_ts, b_ts) = (da.timestamp(), db.timestamp());
            let (a_ns, b_ns) = (da.timestamp_subsec_nanos(), db.timestamp_subsec_nanos());
            let ord = (a_ts, a_ns).cmp(&(b_ts, b_ns));
            return Ok(match op {
                Eq => ord == core::cmp::Ordering::Equal,
                Ne => ord != core::cmp::Ordering::Equal,
                Lt => ord == core::cmp::Ordering::Less,
                Le => ord != core::cmp::Ordering::Greater,
                Gt => ord == core::cmp::Ordering::Greater,
                Ge => ord != core::cmp::Ordering::Less,
            });
        }

        // duration comparisons (same family only)
        if let (XdmAtomicValue::YearMonthDuration(ma), XdmAtomicValue::YearMonthDuration(mb)) =
            (a, b)
        {
            let ord = ma.cmp(mb);
            return Ok(match op {
                Eq => ord == core::cmp::Ordering::Equal,
                Ne => ord != core::cmp::Ordering::Equal,
                Lt => ord == core::cmp::Ordering::Less,
                Le => ord != core::cmp::Ordering::Greater,
                Gt => ord == core::cmp::Ordering::Greater,
                Ge => ord != core::cmp::Ordering::Less,
            });
        }
        if let (XdmAtomicValue::DayTimeDuration(sa), XdmAtomicValue::DayTimeDuration(sb)) = (a, b) {
            let ord = sa.cmp(sb);
            return Ok(match op {
                Eq => ord == core::cmp::Ordering::Equal,
                Ne => ord != core::cmp::Ordering::Equal,
                Lt => ord == core::cmp::Ordering::Less,
                Le => ord != core::cmp::Ordering::Greater,
                Gt => ord == core::cmp::Ordering::Greater,
                Ge => ord != core::cmp::Ordering::Less,
            });
        }

        // date comparisons: normalize to midnight in effective timezone
        if let (
            XdmAtomicValue::Date { date: da, tz: ta },
            XdmAtomicValue::Date { date: db, tz: tb },
        ) = (a, b)
        {
            let eff_tz_a = (*ta).unwrap_or_else(|| self.implicit_timezone());
            let eff_tz_b = (*tb).unwrap_or_else(|| self.implicit_timezone());
            let na = da.and_time(ChronoNaiveTime::from_hms_opt(0, 0, 0).unwrap());
            let nb = db.and_time(ChronoNaiveTime::from_hms_opt(0, 0, 0).unwrap());
            let dta = eff_tz_a.from_local_datetime(&na).single().unwrap();
            let dtb = eff_tz_b.from_local_datetime(&nb).single().unwrap();
            let ord = (dta.timestamp(), dta.timestamp_subsec_nanos())
                .cmp(&(dtb.timestamp(), dtb.timestamp_subsec_nanos()));
            return Ok(match op {
                Eq => ord == core::cmp::Ordering::Equal,
                Ne => ord != core::cmp::Ordering::Equal,
                Lt => ord == core::cmp::Ordering::Less,
                Le => ord != core::cmp::Ordering::Greater,
                Gt => ord == core::cmp::Ordering::Greater,
                Ge => ord != core::cmp::Ordering::Less,
            });
        }

        // time comparisons: anchor to a fixed date and compare instants in effective timezone
        if let (
            XdmAtomicValue::Time { time: ta, tz: tza },
            XdmAtomicValue::Time { time: tb, tz: tzb },
        ) = (a, b)
        {
            let eff_tz_a = (*tza).unwrap_or_else(|| self.implicit_timezone());
            let eff_tz_b = (*tzb).unwrap_or_else(|| self.implicit_timezone());
            let base = chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();
            let na = base.and_time(*ta);
            let nb = base.and_time(*tb);
            let dta = eff_tz_a.from_local_datetime(&na).single().unwrap();
            let dtb = eff_tz_b.from_local_datetime(&nb).single().unwrap();
            let ord = (dta.timestamp(), dta.timestamp_subsec_nanos())
                .cmp(&(dtb.timestamp(), dtb.timestamp_subsec_nanos()));
            return Ok(match op {
                Eq => ord == core::cmp::Ordering::Equal,
                Ne => ord != core::cmp::Ordering::Equal,
                Lt => ord == core::cmp::Ordering::Less,
                Le => ord != core::cmp::Ordering::Greater,
                Gt => ord == core::cmp::Ordering::Greater,
                Ge => ord != core::cmp::Ordering::Less,
            });
        }

        // Unsupported / incomparable type combination → type error (XPTY0004)
        Err(Error::from_code(ErrorCode::XPTY0004, "incomparable atomic types"))
    }

    fn implicit_timezone(&self) -> ChronoFixedOffset {
        if let Some(tz) = self.dyn_ctx.timezone_override {
            return tz;
        }
        if let Some(n) = self.dyn_ctx.now {
            return *n.offset();
        }
        ChronoFixedOffset::east_opt(0).unwrap()
    }

    fn doc_order_distinct(&self, seq: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
        // Fast path: operate directly on the provided Vec without creating a stream.
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

        // If either keyed or fallback is empty, handle each case efficiently
        if fallback.is_empty() {
            if keyed.is_empty() {
                return Ok(others); // no nodes at all
            }
            keyed.sort_by_key(|(k, _)| *k);
            keyed.dedup_by(|a, b| a.0 == b.0);
            let mut out = others;
            out.extend(keyed.into_iter().map(|(_, n)| XdmItem::Node(n)));
            return Ok(out);
        }
        if keyed.is_empty() {
            // Sort by document order using adapter's comparator
            fallback.sort_by(|a, b| self.node_compare(a, b).unwrap_or(Ordering::Equal));
            fallback.dedup();
            let mut out = others;
            out.extend(fallback.into_iter().map(XdmItem::Node));
            return Ok(out);
        }

        // Merge keyed into fallback, then sort/dedup fallback by document order
        keyed.sort_by_key(|(k, _)| *k);
        keyed.dedup_by(|a, b| a.0 == b.0);
        fallback.extend(keyed.into_iter().map(|(_, n)| n));
        fallback.sort_by(|a, b| self.node_compare(a, b).unwrap_or(Ordering::Equal));
        fallback.dedup();
        let mut out = others;
        out.extend(fallback.into_iter().map(XdmItem::Node));
        Ok(out)
    }

    // (function name resolution for error messages is handled in FunctionRegistry::resolve)

    // (default_collation cached in Vm::new)

    // ===== Axis & NodeTest helpers =====
    fn axis_iter(&mut self, node: N, axis: &AxisIR) {
        self.axis_buffer.clear();
        match axis {
            AxisIR::SelfAxis => self.axis_buffer.push(node),
            AxisIR::Child => {
                // Direct iteration without intermediate Vec allocation
                for child in node.children() {
                    if !Self::is_attr_or_namespace(&child) {
                        self.axis_buffer.push(child);
                    }
                }
            }
            AxisIR::Attribute => {
                // Direct iteration for attributes too
                for attr in node.attributes() {
                    self.axis_buffer.push(attr);
                }
            }
            AxisIR::Parent => {
                if let Some(parent) = node.parent() {
                    self.axis_buffer.push(parent);
                }
            }
            AxisIR::Ancestor => self.push_ancestors(node, false),
            AxisIR::AncestorOrSelf => self.push_ancestors(node, true),
            AxisIR::Descendant => self.push_descendants(node, false),
            AxisIR::DescendantOrSelf => self.push_descendants(node, true),
            AxisIR::FollowingSibling => self.push_siblings(node, false),
            AxisIR::PrecedingSibling => self.push_siblings(node, true),
            AxisIR::Following => self.push_following(node),
            AxisIR::Namespace => self.push_namespaces(node),
            AxisIR::Preceding => self.push_preceding(node),
        }
    }
    fn push_ancestors(&mut self, node: N, include_self: bool) {
        if include_self {
            self.axis_buffer.push(node.clone());
        }
        let mut current = node.parent();
        while let Some(parent) = current {
            self.axis_buffer.push(parent.clone());
            current = parent.parent();
        }
    }
    fn push_descendants(&mut self, node: N, include_self: bool) {
        if include_self {
            self.axis_buffer.push(node.clone());
        }
        let mut stack: SmallVec<[N; 16]> = SmallVec::new(); // Use SmallVec for better cache locality

        // Collect children efficiently without intermediate allocation
        for child in node.children() {
            if !Self::is_attr_or_namespace(&child) {
                stack.push(child);
            }
        }

        while let Some(cur) = stack.pop() {
            self.axis_buffer.push(cur.clone());
            // Process children directly without allocation
            for child in cur.children() {
                if !Self::is_attr_or_namespace(&child) {
                    stack.push(child);
                }
            }
        }
    }
    fn push_siblings(&mut self, node: N, preceding: bool) {
        if let Some(parent) = node.parent() {
            if preceding {
                for sib in parent.children() {
                    if sib == node {
                        break;
                    }
                    if !Self::is_attr_or_namespace(&sib) {
                        self.axis_buffer.push(sib);
                    }
                }
            } else {
                let mut seen = false;
                for sib in parent.children() {
                    if seen {
                        if !Self::is_attr_or_namespace(&sib) {
                            self.axis_buffer.push(sib);
                        }
                    } else if sib == node {
                        seen = true;
                    }
                }
            }
        }
    }
    fn push_following(&mut self, node: N) {
        let mut anchor = Self::last_descendant_in_doc(node);
        let mut cursor = self.doc_successor(&anchor);
        while let Some(next) = cursor {
            if !Self::is_attr_or_namespace(&next) {
                self.axis_buffer.push(next.clone());
            }
            anchor = next;
            cursor = self.doc_successor(&anchor);
        }
    }
    fn push_namespaces(&mut self, node: N) {
        use crate::model::NodeKind;
        if !matches!(node.kind(), NodeKind::Element) {
            return;
        }
        let mut seen = SmallVec::<[DefaultAtom; 8]>::new();
        let mut cur: Option<N> = Some(node);
        while let Some(n) = cur {
            if matches!(n.kind(), NodeKind::Element) {
                for ns in n.namespaces() {
                    if let Some(qn) = ns.name() {
                        let atom = DefaultAtom::from(qn.prefix.unwrap_or_default().as_str());
                        if !seen.iter().any(|existing| existing == &atom) {
                            seen.push(atom);
                            self.axis_buffer.push(ns.clone());
                        }
                    }
                }
            }
            cur = n.parent();
        }
    }
    fn push_preceding(&mut self, node: N) {
        let mut cursor = self.doc_predecessor(&node);
        while let Some(prev) = cursor {
            let prev_cursor = self.doc_predecessor(&prev);
            if !Self::is_attr_or_namespace(&prev) && !self.is_ancestor_of(&prev, &node) {
                self.axis_buffer.push(prev.clone());
            }
            cursor = prev_cursor;
        }
        self.axis_buffer.reverse();
    }
    fn doc_successor(&self, node: &N) -> Option<N> {
        if let Some(child) = Self::first_child_in_doc(node) {
            return Some(child);
        }
        let mut current = node.clone();
        while let Some(parent) = current.parent() {
            if let Some(sib) = Self::next_sibling_in_doc(&current) {
                return Some(sib);
            }
            current = parent;
        }
        None
    }
    fn doc_predecessor(&self, node: &N) -> Option<N> {
        if let Some(prev) = Self::prev_sibling_in_doc(node) {
            return Some(Self::last_descendant_in_doc(prev));
        }
        let mut current = node.clone();
        while let Some(parent) = current.parent() {
            if !Self::is_attr_or_namespace(&parent) {
                return Some(parent);
            }
            if let Some(prev) = Self::prev_sibling_in_doc(&parent) {
                return Some(Self::last_descendant_in_doc(prev));
            }
            current = parent;
        }
        None
    }
    fn first_child_in_doc(node: &N) -> Option<N> {
        node.children().find(|child| !Self::is_attr_or_namespace(child))
    }
    fn next_sibling_in_doc(node: &N) -> Option<N> {
        let parent = node.parent()?;
        let mut found = false;
        for sib in parent.children() {
            if found && !Self::is_attr_or_namespace(&sib) {
                return Some(sib);
            }
            if sib == *node {
                found = true;
            }
        }
        None
    }
    fn prev_sibling_in_doc(node: &N) -> Option<N> {
        let parent = node.parent()?;
        let mut prev: Option<N> = None;
        for sib in parent.children() {
            if sib == *node {
                break;
            }
            if !Self::is_attr_or_namespace(&sib) {
                prev = Some(sib);
            }
        }
        prev
    }
    fn last_descendant_in_doc(node: N) -> N {
        let mut current = node;
        loop {
            let mut last_child: Option<N> = None;
            for child in current.children() {
                if !Self::is_attr_or_namespace(&child) {
                    last_child = Some(child);
                }
            }
            if let Some(child) = last_child {
                current = child;
            } else {
                return current;
            }
        }
    }
    fn is_attr_or_namespace(node: &N) -> bool {
        matches!(node.kind(), NodeKind::Attribute | NodeKind::Namespace)
    }
    fn is_descendant_of(&self, node: &N, ancestor: &N) -> bool {
        let mut current = node.parent();
        while let Some(parent) = current {
            if parent == *ancestor {
                return true;
            }
            current = parent.parent();
        }
        false
    }
    fn is_ancestor_of(&self, node: &N, descendant: &N) -> bool {
        self.is_descendant_of(descendant, node)
    }

    #[inline]
    fn matches_interned_name(
        &self,
        node: &N,
        expected: &crate::compiler::ir::InternedQName,
    ) -> bool {
        let node_name = match node.name() {
            Some(n) => n,
            None => return false,
        };

        if expected.local.as_ref() != node_name.local.as_str() {
            return false;
        }

        let node_ns = node_name.ns_uri.as_ref().map(|ns| DefaultAtom::from(ns.as_str()));

        let effective_ns = match node_ns {
            Some(atom) => Some(atom),
            None => match node_name.prefix.as_deref() {
                Some(prefix) => self.resolve_prefix_namespace(node, prefix),
                None if matches!(node.kind(), crate::model::NodeKind::Attribute) => None,
                None => self.resolve_prefix_namespace(node, ""),
            },
        };

        match (&effective_ns, &expected.ns_uri) {
            (None, None) => true,
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }

    fn node_test(&self, node: &N, test: &NodeTestIR) -> bool {
        use NodeTestIR::*;
        match test {
            AnyKind => true,
            Name(q) => {
                // For namespace nodes, the NameTest matches by prefix (local) only.
                if matches!(node.kind(), crate::model::NodeKind::Namespace) {
                    return node.name().map(|n| n.local == q.original.local).unwrap_or(false);
                }
                // Use fast path for name comparison
                self.matches_interned_name(node, q)
            }
            WildcardAny => true,
            NsWildcard(ns) => node
                .name()
                .map(|n| {
                    let eff = if let Some(uri) = n.ns_uri.as_ref() {
                        Some(DefaultAtom::from(uri.as_str()))
                    } else if let Some(pref) = &n.prefix {
                        self.resolve_prefix_namespace(node, pref)
                    } else if matches!(
                        node.kind(),
                        crate::model::NodeKind::Element | crate::model::NodeKind::Namespace
                    ) {
                        self.resolve_prefix_namespace(node, "")
                    } else {
                        None
                    };
                    eff.is_some_and(|atom| atom == *ns)
                })
                .unwrap_or(false),
            LocalWildcard(local) => {
                // Use interned comparison for local names
                node.name().map(|n| n.local.as_str() == local.as_ref()).unwrap_or(false)
            }
            KindText => matches!(node.kind(), crate::model::NodeKind::Text),
            KindComment => matches!(node.kind(), crate::model::NodeKind::Comment),
            KindProcessingInstruction(target_opt) => {
                if !matches!(node.kind(), crate::model::NodeKind::ProcessingInstruction) {
                    return false;
                }
                if let Some(target) = target_opt {
                    if let Some(nm) = node.name() { nm.local == *target } else { false }
                } else {
                    true
                }
            }
            KindDocument(inner_opt) => {
                if !matches!(node.kind(), crate::model::NodeKind::Document) {
                    return false;
                }
                if let Some(inner) = inner_opt {
                    for c in node.children() {
                        if self.node_test(&c, inner) {
                            return true;
                        }
                    }
                    false
                } else {
                    true
                }
            }
            KindElement { name, .. } => {
                if !matches!(node.kind(), crate::model::NodeKind::Element) {
                    return false;
                }
                match name {
                    None => true,
                    Some(NameOrWildcard::Any) => true,
                    Some(NameOrWildcard::Name(exp)) => self.matches_interned_name(node, exp),
                }
            }
            KindAttribute { name, .. } => {
                if !matches!(node.kind(), crate::model::NodeKind::Attribute) {
                    return false;
                }
                match name {
                    None => true,
                    Some(NameOrWildcard::Any) => true,
                    Some(NameOrWildcard::Name(exp)) => self.matches_interned_name(node, exp),
                }
            }
            KindSchemaElement(_) | KindSchemaAttribute(_) => true, // simplified
        }
    }

    /// Resolve a namespace prefix to its in-scope namespace URI for the given node by walking
    /// up the ancestor chain and inspecting declared namespace nodes. Honors the implicit `xml`
    /// binding. Returns `None` when no binding is found.
    fn resolve_prefix_namespace(&self, node: &N, prefix: &str) -> Option<DefaultAtom> {
        if prefix == "xml" {
            return Some(DefaultAtom::from(crate::consts::XML_URI));
        }
        use crate::model::NodeKind;
        let mut cur = Some(node.clone());
        while let Some(n) = cur {
            if matches!(n.kind(), NodeKind::Element) {
                for ns in n.namespaces() {
                    if let Some(q) = ns.name() {
                        let p = q.prefix.unwrap_or_default();
                        if p == prefix {
                            return Some(DefaultAtom::from(ns.string_value().as_str()));
                        }
                    }
                }
            }
            cur = n.parent();
        }
        None
    }

    // ===== Set operations (nodes-only; results in document order with duplicates removed) =====
    fn set_union(&mut self, a: XdmSequence<N>, b: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
        let mut combined: XdmSequence<N> = Vec::with_capacity(a.len() + b.len());
        combined.extend(a);
        combined.extend(b);
        self.doc_order_distinct(combined)
    }
    fn set_intersect(
        &mut self,
        a: XdmSequence<N>,
        b: XdmSequence<N>,
    ) -> Result<XdmSequence<N>, Error> {
        let lhs = self.sorted_distinct_nodes(a)?;
        let rhs = self.sorted_distinct_nodes(b)?;
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
        // Pre-allocate with minimum estimate
        let mut out: Vec<N> =
            Vec::with_capacity(lhs.len().min(rhs_keys.len() + rhs_fallback.len()));
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
    fn set_except(
        &mut self,
        a: XdmSequence<N>,
        b: XdmSequence<N>,
    ) -> Result<XdmSequence<N>, Error> {
        let lhs = self.sorted_distinct_nodes(a)?;
        let rhs = self.sorted_distinct_nodes(b)?;
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
        // Pre-allocate with optimistic estimate (worst case is size of lhs)
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
    fn sorted_distinct_nodes(&self, seq: XdmSequence<N>) -> Result<Vec<N>, Error> {
        let ordered = self.doc_order_distinct(seq)?;
        ordered
            .into_iter()
            .map(|item| match item {
                XdmItem::Node(n) => Ok(n),
                _ => Err(Error::not_implemented("non-node item encountered in set operation")),
            })
            .collect()
    }
    fn node_compare(&self, a: &N, b: &N) -> Result<Ordering, Error> {
        match (a.doc_order_key(), b.doc_order_key()) {
            (Some(ak), Some(bk)) => Ok(ak.cmp(&bk)),
            _ => a.compare_document_order(b),
        }
    }

    // ===== Type operations (very small subset) =====
    #[allow(dead_code)]
    #[allow(dead_code)]
    fn cast(&self, seq: XdmSequence<N>, t: &SingleTypeIR) -> Result<XdmSequence<N>, Error> {
        if seq.len() > 1 {
            return Err(Error::from_code(ErrorCode::XPTY0004, "cast of multi-item"));
        }
        if seq.is_empty() {
            if t.optional {
                return Ok(Vec::new());
            } else {
                return Err(Error::from_code(ErrorCode::XPST0003, "empty not allowed"));
            }
        }
        let item = seq[0].clone();
        let val = match item {
            XdmItem::Atomic(a) => a,
            XdmItem::Node(n) => XdmAtomicValue::UntypedAtomic(n.string_value()),
        };
        let casted = self.cast_atomic(val, &t.atomic)?;
        Ok(vec![XdmItem::Atomic(casted)])
    }
    fn parse_integer_string(&self, text: &str, target: &str) -> Result<i128, Error> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(Error::from_code(
                ErrorCode::FORG0001,
                format!("cannot cast to {target}: empty string"),
            ));
        }
        trimmed.parse::<i128>().map_err(|_| {
            Error::from_code(ErrorCode::FORG0001, format!("invalid lexical for {target}"))
        })
    }

    fn float_to_integer(&self, value: f64, target: &str) -> Result<i128, Error> {
        if !value.is_finite() {
            return Err(Error::from_code(ErrorCode::FOCA0001, format!("{target} overflow")));
        }
        if value.fract() != 0.0 {
            return Err(Error::from_code(
                ErrorCode::FOCA0001,
                format!("non-integer value for {target}"),
            ));
        }
        if value < i128::MIN as f64 || value > i128::MAX as f64 {
            return Err(Error::from_code(ErrorCode::FOCA0001, format!("{target} overflow")));
        }
        Ok(value as i128)
    }

    fn integer_from_atomic(&self, atom: &XdmAtomicValue, target: &str) -> Result<i128, Error> {
        use XdmAtomicValue::*;
        match atom {
            Integer(v) => Ok(*v as i128),
            Long(v) => Ok(*v as i128),
            Int(v) => Ok(*v as i128),
            Short(v) => Ok(*v as i128),
            Byte(v) => Ok(*v as i128),
            NonPositiveInteger(v) => Ok(*v as i128),
            NegativeInteger(v) => Ok(*v as i128),
            UnsignedLong(v) => Ok(*v as i128),
            UnsignedInt(v) => Ok(*v as i128),
            UnsignedShort(v) => Ok(*v as i128),
            UnsignedByte(v) => Ok(*v as i128),
            NonNegativeInteger(v) => Ok(*v as i128),
            PositiveInteger(v) => Ok(*v as i128),
            Decimal(d) => self.float_to_integer(*d, target),
            Double(d) => self.float_to_integer(*d, target),
            Float(f) => self.float_to_integer(*f as f64, target),
            other => {
                if let Some(text) = string_like_value(other) {
                    self.parse_integer_string(&text, target)
                } else {
                    Err(Error::from_code(ErrorCode::FORG0001, format!("cannot cast to {target}")))
                }
            }
        }
    }

    fn unsigned_from_atomic(&self, atom: &XdmAtomicValue, target: &str) -> Result<u128, Error> {
        match atom {
            XdmAtomicValue::UnsignedLong(v) => Ok(*v as u128),
            XdmAtomicValue::UnsignedInt(v) => Ok(*v as u128),
            XdmAtomicValue::UnsignedShort(v) => Ok(*v as u128),
            XdmAtomicValue::UnsignedByte(v) => Ok(*v as u128),
            XdmAtomicValue::NonNegativeInteger(v) => Ok(*v as u128),
            XdmAtomicValue::PositiveInteger(v) => Ok(*v as u128),
            other => {
                let signed = self.integer_from_atomic(other, target)?;
                if signed < 0 {
                    Err(Error::from_code(
                        ErrorCode::FORG0001,
                        format!("negative value not allowed for {target}"),
                    ))
                } else {
                    Ok(signed as u128)
                }
            }
        }
    }

    fn ensure_range_i128(
        &self,
        value: i128,
        min: i128,
        max: i128,
        target: &str,
    ) -> Result<i128, Error> {
        if value < min || value > max {
            Err(Error::from_code(ErrorCode::FORG0001, format!("value out of range for {target}")))
        } else {
            Ok(value)
        }
    }

    fn ensure_range_u128(
        &self,
        value: u128,
        min: u128,
        max: u128,
        target: &str,
    ) -> Result<u128, Error> {
        if value < min || value > max {
            Err(Error::from_code(ErrorCode::FORG0001, format!("value out of range for {target}")))
        } else {
            Ok(value)
        }
    }

    fn require_string_like(&self, atom: &XdmAtomicValue, target: &str) -> Result<String, Error> {
        string_like_value(atom).ok_or_else(|| {
            Error::from_code(ErrorCode::FORG0001, format!("cannot cast to {target}"))
        })
    }
    fn cast_atomic(
        &self,
        a: XdmAtomicValue,
        target: &ExpandedName,
    ) -> Result<XdmAtomicValue, Error> {
        let local = target.local.as_str();
        match local {
            "anyAtomicType" => Ok(a),
            "string" => {
                let text = string_like_value(&a).unwrap_or_else(|| self.atomic_to_string(&a));
                Ok(XdmAtomicValue::String(text))
            }
            "untypedAtomic" => {
                let text = string_like_value(&a).unwrap_or_else(|| self.atomic_to_string(&a));
                Ok(XdmAtomicValue::UntypedAtomic(text))
            }
            "boolean" => match a {
                XdmAtomicValue::Boolean(b) => Ok(XdmAtomicValue::Boolean(b)),
                XdmAtomicValue::Integer(i) => Ok(XdmAtomicValue::Boolean(i != 0)),
                XdmAtomicValue::Decimal(d) => Ok(XdmAtomicValue::Boolean(d != 0.0)),
                XdmAtomicValue::Double(d) => Ok(XdmAtomicValue::Boolean(d != 0.0 && !d.is_nan())),
                XdmAtomicValue::Float(f) => Ok(XdmAtomicValue::Boolean(f != 0.0 && !f.is_nan())),
                other => {
                    let text = self.require_string_like(&other, "xs:boolean")?;
                    let b = match text.as_str() {
                        "true" | "1" => true,
                        "false" | "0" => false,
                        _ => {
                            return Err(Error::from_code(
                                ErrorCode::FORG0001,
                                "invalid boolean lexical form",
                            ));
                        }
                    };
                    Ok(XdmAtomicValue::Boolean(b))
                }
            },
            "integer" => match a {
                XdmAtomicValue::Integer(v) => Ok(XdmAtomicValue::Integer(v)),
                other => {
                    let value = self.integer_from_atomic(&other, "xs:integer")?;
                    let bounded = self.ensure_range_i128(
                        value,
                        i64::MIN as i128,
                        i64::MAX as i128,
                        "xs:integer",
                    )?;
                    Ok(XdmAtomicValue::Integer(bounded as i64))
                }
            },
            "decimal" => match a {
                XdmAtomicValue::Decimal(d) => Ok(XdmAtomicValue::Decimal(d)),
                XdmAtomicValue::Integer(i) => Ok(XdmAtomicValue::Decimal(i as f64)),
                XdmAtomicValue::Long(i) => Ok(XdmAtomicValue::Decimal(i as f64)),
                XdmAtomicValue::Int(i) => Ok(XdmAtomicValue::Decimal(i as f64)),
                XdmAtomicValue::Short(i) => Ok(XdmAtomicValue::Decimal(i as f64)),
                XdmAtomicValue::Byte(i) => Ok(XdmAtomicValue::Decimal(i as f64)),
                XdmAtomicValue::NonPositiveInteger(i) => Ok(XdmAtomicValue::Decimal(i as f64)),
                XdmAtomicValue::NegativeInteger(i) => Ok(XdmAtomicValue::Decimal(i as f64)),
                XdmAtomicValue::NonNegativeInteger(i) => Ok(XdmAtomicValue::Decimal(i as f64)),
                XdmAtomicValue::PositiveInteger(i) => Ok(XdmAtomicValue::Decimal(i as f64)),
                XdmAtomicValue::UnsignedLong(i) => Ok(XdmAtomicValue::Decimal(i as f64)),
                XdmAtomicValue::UnsignedInt(i) => Ok(XdmAtomicValue::Decimal(i as f64)),
                XdmAtomicValue::UnsignedShort(i) => Ok(XdmAtomicValue::Decimal(i as f64)),
                XdmAtomicValue::UnsignedByte(i) => Ok(XdmAtomicValue::Decimal(i as f64)),
                XdmAtomicValue::Double(d) => {
                    if d.is_finite() {
                        Ok(XdmAtomicValue::Decimal(d))
                    } else {
                        Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:decimal"))
                    }
                }
                XdmAtomicValue::Float(f) => {
                    let v = f as f64;
                    if v.is_finite() {
                        Ok(XdmAtomicValue::Decimal(v))
                    } else {
                        Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:decimal"))
                    }
                }
                other => {
                    let text = self.require_string_like(&other, "xs:decimal")?;
                    let trimmed = text.trim();
                    if trimmed.eq_ignore_ascii_case("nan")
                        || trimmed.eq_ignore_ascii_case("inf")
                        || trimmed.eq_ignore_ascii_case("-inf")
                    {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:decimal"));
                    }
                    let value: f64 = trimmed
                        .parse()
                        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:decimal"))?;
                    Ok(XdmAtomicValue::Decimal(value))
                }
            },
            "double" => match a {
                XdmAtomicValue::Double(d) => Ok(XdmAtomicValue::Double(d)),
                XdmAtomicValue::Float(f) => Ok(XdmAtomicValue::Double(f as f64)),
                XdmAtomicValue::Decimal(d) => Ok(XdmAtomicValue::Double(d)),
                XdmAtomicValue::Integer(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::Long(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::Int(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::Short(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::Byte(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::NonPositiveInteger(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::NegativeInteger(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::NonNegativeInteger(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::PositiveInteger(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::UnsignedLong(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::UnsignedInt(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::UnsignedShort(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::UnsignedByte(i) => Ok(XdmAtomicValue::Double(i as f64)),
                other => {
                    let text = self.require_string_like(&other, "xs:double")?;
                    let trimmed = text.trim();
                    let value = match trimmed {
                        "NaN" | "nan" => f64::NAN,
                        "INF" | "inf" => f64::INFINITY,
                        "-INF" | "-inf" => f64::NEG_INFINITY,
                        _ => trimmed.parse().map_err(|_| {
                            Error::from_code(ErrorCode::FORG0001, "invalid xs:double")
                        })?,
                    };
                    Ok(XdmAtomicValue::Double(value))
                }
            },
            "float" => match a {
                XdmAtomicValue::Float(f) => Ok(XdmAtomicValue::Float(f)),
                XdmAtomicValue::Double(d) => Ok(XdmAtomicValue::Float(d as f32)),
                XdmAtomicValue::Decimal(d) => Ok(XdmAtomicValue::Float(d as f32)),
                XdmAtomicValue::Integer(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::Long(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::Int(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::Short(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::Byte(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::NonPositiveInteger(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::NegativeInteger(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::NonNegativeInteger(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::PositiveInteger(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::UnsignedLong(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::UnsignedInt(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::UnsignedShort(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::UnsignedByte(i) => Ok(XdmAtomicValue::Float(i as f32)),
                other => {
                    let text = self.require_string_like(&other, "xs:float")?;
                    let trimmed = text.trim();
                    let value = match trimmed {
                        "NaN" | "nan" => f32::NAN,
                        "INF" | "inf" => f32::INFINITY,
                        "-INF" | "-inf" => f32::NEG_INFINITY,
                        _ => trimmed.parse().map_err(|_| {
                            Error::from_code(ErrorCode::FORG0001, "invalid xs:float")
                        })?,
                    };
                    Ok(XdmAtomicValue::Float(value))
                }
            },
            "long" => match a {
                XdmAtomicValue::Long(v) => Ok(XdmAtomicValue::Long(v)),
                other => {
                    let value = self.integer_from_atomic(&other, "xs:long")?;
                    let bounded = self.ensure_range_i128(
                        value,
                        i64::MIN as i128,
                        i64::MAX as i128,
                        "xs:long",
                    )?;
                    Ok(XdmAtomicValue::Long(bounded as i64))
                }
            },
            "int" => match a {
                XdmAtomicValue::Int(v) => Ok(XdmAtomicValue::Int(v)),
                other => {
                    let value = self.integer_from_atomic(&other, "xs:int")?;
                    let bounded = self.ensure_range_i128(
                        value,
                        i32::MIN as i128,
                        i32::MAX as i128,
                        "xs:int",
                    )?;
                    Ok(XdmAtomicValue::Int(bounded as i32))
                }
            },
            "short" => match a {
                XdmAtomicValue::Short(v) => Ok(XdmAtomicValue::Short(v)),
                other => {
                    let value = self.integer_from_atomic(&other, "xs:short")?;
                    let bounded = self.ensure_range_i128(
                        value,
                        i16::MIN as i128,
                        i16::MAX as i128,
                        "xs:short",
                    )?;
                    Ok(XdmAtomicValue::Short(bounded as i16))
                }
            },
            "byte" => match a {
                XdmAtomicValue::Byte(v) => Ok(XdmAtomicValue::Byte(v)),
                other => {
                    let value = self.integer_from_atomic(&other, "xs:byte")?;
                    let bounded =
                        self.ensure_range_i128(value, i8::MIN as i128, i8::MAX as i128, "xs:byte")?;
                    Ok(XdmAtomicValue::Byte(bounded as i8))
                }
            },
            "unsignedLong" => match a {
                XdmAtomicValue::UnsignedLong(v) => Ok(XdmAtomicValue::UnsignedLong(v)),
                other => {
                    let value = self.unsigned_from_atomic(&other, "xs:unsignedLong")?;
                    let bounded =
                        self.ensure_range_u128(value, 0, u64::MAX as u128, "xs:unsignedLong")?;
                    Ok(XdmAtomicValue::UnsignedLong(bounded as u64))
                }
            },
            "unsignedInt" => match a {
                XdmAtomicValue::UnsignedInt(v) => Ok(XdmAtomicValue::UnsignedInt(v)),
                other => {
                    let value = self.unsigned_from_atomic(&other, "xs:unsignedInt")?;
                    let bounded =
                        self.ensure_range_u128(value, 0, u32::MAX as u128, "xs:unsignedInt")?;
                    Ok(XdmAtomicValue::UnsignedInt(bounded as u32))
                }
            },
            "unsignedShort" => match a {
                XdmAtomicValue::UnsignedShort(v) => Ok(XdmAtomicValue::UnsignedShort(v)),
                other => {
                    let value = self.unsigned_from_atomic(&other, "xs:unsignedShort")?;
                    let bounded =
                        self.ensure_range_u128(value, 0, u16::MAX as u128, "xs:unsignedShort")?;
                    Ok(XdmAtomicValue::UnsignedShort(bounded as u16))
                }
            },
            "unsignedByte" => match a {
                XdmAtomicValue::UnsignedByte(v) => Ok(XdmAtomicValue::UnsignedByte(v)),
                other => {
                    let value = self.unsigned_from_atomic(&other, "xs:unsignedByte")?;
                    let bounded =
                        self.ensure_range_u128(value, 0, u8::MAX as u128, "xs:unsignedByte")?;
                    Ok(XdmAtomicValue::UnsignedByte(bounded as u8))
                }
            },
            "nonPositiveInteger" => match a {
                XdmAtomicValue::NonPositiveInteger(v) => Ok(XdmAtomicValue::NonPositiveInteger(v)),
                other => {
                    let value = self.integer_from_atomic(&other, "xs:nonPositiveInteger")?;
                    if value > 0 {
                        return Err(Error::from_code(
                            ErrorCode::FORG0001,
                            "value must be <= 0 for xs:nonPositiveInteger",
                        ));
                    }
                    let bounded = self.ensure_range_i128(
                        value,
                        i64::MIN as i128,
                        0,
                        "xs:nonPositiveInteger",
                    )?;
                    Ok(XdmAtomicValue::NonPositiveInteger(bounded as i64))
                }
            },
            "negativeInteger" => match a {
                XdmAtomicValue::NegativeInteger(v) => Ok(XdmAtomicValue::NegativeInteger(v)),
                other => {
                    let value = self.integer_from_atomic(&other, "xs:negativeInteger")?;
                    if value >= 0 {
                        return Err(Error::from_code(
                            ErrorCode::FORG0001,
                            "value must be < 0 for xs:negativeInteger",
                        ));
                    }
                    let bounded =
                        self.ensure_range_i128(value, i64::MIN as i128, -1, "xs:negativeInteger")?;
                    Ok(XdmAtomicValue::NegativeInteger(bounded as i64))
                }
            },
            "nonNegativeInteger" => match a {
                XdmAtomicValue::NonNegativeInteger(v) => Ok(XdmAtomicValue::NonNegativeInteger(v)),
                other => {
                    let value = self.unsigned_from_atomic(&other, "xs:nonNegativeInteger")?;
                    let bounded = self.ensure_range_u128(
                        value,
                        0,
                        u64::MAX as u128,
                        "xs:nonNegativeInteger",
                    )?;
                    Ok(XdmAtomicValue::NonNegativeInteger(bounded as u64))
                }
            },
            "positiveInteger" => match a {
                XdmAtomicValue::PositiveInteger(v) => Ok(XdmAtomicValue::PositiveInteger(v)),
                other => {
                    let value = self.unsigned_from_atomic(&other, "xs:positiveInteger")?;
                    if value == 0 {
                        return Err(Error::from_code(
                            ErrorCode::FORG0001,
                            "value must be > 0 for xs:positiveInteger",
                        ));
                    }
                    let bounded =
                        self.ensure_range_u128(value, 1, u64::MAX as u128, "xs:positiveInteger")?;
                    Ok(XdmAtomicValue::PositiveInteger(bounded as u64))
                }
            },
            "anyURI" => match a {
                XdmAtomicValue::AnyUri(uri) => Ok(XdmAtomicValue::AnyUri(uri)),
                other => {
                    let text = self.require_string_like(&other, "xs:anyURI")?;
                    Ok(XdmAtomicValue::AnyUri(text.trim().to_string()))
                }
            },
            "QName" => match a {
                XdmAtomicValue::QName { ns_uri, prefix, local } => {
                    Ok(XdmAtomicValue::QName { ns_uri, prefix, local })
                }
                other => {
                    let text = self.require_string_like(&other, "xs:QName")?;
                    let (prefix, local) = parse_qname_lexical(&text).map_err(|_| {
                        Error::from_code(ErrorCode::FORG0001, "invalid QName lexical")
                    })?;
                    Ok(XdmAtomicValue::QName { ns_uri: None, prefix, local })
                }
            },
            "NOTATION" => match a {
                XdmAtomicValue::Notation(s) => Ok(XdmAtomicValue::Notation(s)),
                other => {
                    let text = self.require_string_like(&other, "xs:NOTATION")?;
                    if parse_qname_lexical(&text).is_err() {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:NOTATION"));
                    }
                    Ok(XdmAtomicValue::Notation(text))
                }
            },
            "base64Binary" => match a {
                XdmAtomicValue::Base64Binary(v) => Ok(XdmAtomicValue::Base64Binary(v)),
                XdmAtomicValue::HexBinary(hex) => {
                    let bytes = decode_hex(&hex).ok_or_else(|| {
                        Error::from_code(ErrorCode::FORG0001, "invalid xs:hexBinary")
                    })?;
                    let encoded = BASE64_STANDARD.encode(bytes);
                    Ok(XdmAtomicValue::Base64Binary(encoded))
                }
                other => {
                    let text = self.require_string_like(&other, "xs:base64Binary")?;
                    let normalized: String = text.chars().filter(|c| !c.is_whitespace()).collect();
                    if BASE64_STANDARD.decode(normalized.as_bytes()).is_err() {
                        return Err(Error::from_code(
                            ErrorCode::FORG0001,
                            "invalid xs:base64Binary",
                        ));
                    }
                    Ok(XdmAtomicValue::Base64Binary(normalized))
                }
            },
            "hexBinary" => match a {
                XdmAtomicValue::HexBinary(v) => Ok(XdmAtomicValue::HexBinary(v)),
                XdmAtomicValue::Base64Binary(b64) => {
                    let bytes = BASE64_STANDARD.decode(b64.as_bytes()).map_err(|_| {
                        Error::from_code(ErrorCode::FORG0001, "invalid xs:base64Binary")
                    })?;
                    let encoded = encode_hex_upper(&bytes);
                    Ok(XdmAtomicValue::HexBinary(encoded))
                }
                other => {
                    let text = self.require_string_like(&other, "xs:hexBinary")?;
                    let normalized: String = text.chars().filter(|c| !c.is_whitespace()).collect();
                    if decode_hex(&normalized).is_none() {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:hexBinary"));
                    }
                    Ok(XdmAtomicValue::HexBinary(normalized.to_uppercase()))
                }
            },
            "normalizedString" => match a {
                XdmAtomicValue::NormalizedString(s) => Ok(XdmAtomicValue::NormalizedString(s)),
                other => {
                    let text = self.require_string_like(&other, "xs:normalizedString")?;
                    let normalized = replace_xml_whitespace(&text);
                    Ok(XdmAtomicValue::NormalizedString(normalized))
                }
            },
            "token" => match a {
                XdmAtomicValue::Token(s) => Ok(XdmAtomicValue::Token(s)),
                other => {
                    let text = self.require_string_like(&other, "xs:token")?;
                    let collapsed = collapse_xml_whitespace(&text);
                    Ok(XdmAtomicValue::Token(collapsed))
                }
            },
            "language" => match a {
                XdmAtomicValue::Language(s) => Ok(XdmAtomicValue::Language(s)),
                other => {
                    let text = self.require_string_like(&other, "xs:language")?;
                    let collapsed = collapse_xml_whitespace(&text);
                    if !is_valid_language(&collapsed) {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:language"));
                    }
                    Ok(XdmAtomicValue::Language(collapsed))
                }
            },
            "Name" => match a {
                XdmAtomicValue::Name(s) => Ok(XdmAtomicValue::Name(s)),
                other => {
                    let text = self.require_string_like(&other, "xs:Name")?;
                    let collapsed = collapse_xml_whitespace(&text);
                    if !is_valid_name(&collapsed, true, true) {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:Name"));
                    }
                    Ok(XdmAtomicValue::Name(collapsed))
                }
            },
            "NCName" => match a {
                XdmAtomicValue::NCName(s) => Ok(XdmAtomicValue::NCName(s)),
                other => {
                    let text = self.require_string_like(&other, "xs:NCName")?;
                    let collapsed = collapse_xml_whitespace(&text);
                    if !is_valid_name(&collapsed, true, false) {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:NCName"));
                    }
                    Ok(XdmAtomicValue::NCName(collapsed))
                }
            },
            "NMTOKEN" => match a {
                XdmAtomicValue::NMTOKEN(s) => Ok(XdmAtomicValue::NMTOKEN(s)),
                other => {
                    let text = self.require_string_like(&other, "xs:NMTOKEN")?;
                    let collapsed = collapse_xml_whitespace(&text);
                    if !is_valid_nmtoken(&collapsed) {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:NMTOKEN"));
                    }
                    Ok(XdmAtomicValue::NMTOKEN(collapsed))
                }
            },
            "ID" => match a {
                XdmAtomicValue::Id(s) => Ok(XdmAtomicValue::Id(s)),
                other => {
                    let text = self.require_string_like(&other, "xs:ID")?;
                    let collapsed = collapse_xml_whitespace(&text);
                    if !is_valid_name(&collapsed, true, false) {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:ID"));
                    }
                    Ok(XdmAtomicValue::Id(collapsed))
                }
            },
            "IDREF" => match a {
                XdmAtomicValue::IdRef(s) => Ok(XdmAtomicValue::IdRef(s)),
                other => {
                    let text = self.require_string_like(&other, "xs:IDREF")?;
                    let collapsed = collapse_xml_whitespace(&text);
                    if !is_valid_name(&collapsed, true, false) {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:IDREF"));
                    }
                    Ok(XdmAtomicValue::IdRef(collapsed))
                }
            },
            "ENTITY" => match a {
                XdmAtomicValue::Entity(s) => Ok(XdmAtomicValue::Entity(s)),
                other => {
                    let text = self.require_string_like(&other, "xs:ENTITY")?;
                    let collapsed = collapse_xml_whitespace(&text);
                    if !is_valid_name(&collapsed, true, false) {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:ENTITY"));
                    }
                    Ok(XdmAtomicValue::Entity(collapsed))
                }
            },
            "date" => match a {
                XdmAtomicValue::Date { date, tz } => Ok(XdmAtomicValue::Date { date, tz }),
                other => {
                    let text = self.require_string_like(&other, "xs:date")?;
                    match self.parse_date(&text) {
                        Ok(v) => Ok(v),
                        Err(_) => Err(Error::from_code(ErrorCode::FORG0001, "invalid date")),
                    }
                }
            },
            "dateTime" => match a {
                XdmAtomicValue::DateTime(dt) => Ok(XdmAtomicValue::DateTime(dt)),
                other => {
                    let text = self.require_string_like(&other, "xs:dateTime")?;
                    match self.parse_date_time(&text) {
                        Ok(v) => Ok(v),
                        Err(_) => Err(Error::from_code(ErrorCode::FORG0001, "invalid dateTime")),
                    }
                }
            },
            "time" => match a {
                XdmAtomicValue::Time { time, tz } => Ok(XdmAtomicValue::Time { time, tz }),
                other => {
                    let text = self.require_string_like(&other, "xs:time")?;
                    match self.parse_time(&text) {
                        Ok(v) => Ok(v),
                        Err(_) => Err(Error::from_code(ErrorCode::FORG0001, "invalid time")),
                    }
                }
            },
            "yearMonthDuration" => match a {
                XdmAtomicValue::YearMonthDuration(m) => Ok(XdmAtomicValue::YearMonthDuration(m)),
                other => {
                    let text = self.require_string_like(&other, "xs:yearMonthDuration")?;
                    self.parse_year_month_duration(&text).map_err(|_| {
                        Error::from_code(ErrorCode::FORG0001, "invalid yearMonthDuration")
                    })
                }
            },
            "dayTimeDuration" => match a {
                XdmAtomicValue::DayTimeDuration(m) => Ok(XdmAtomicValue::DayTimeDuration(m)),
                other => {
                    let text = self.require_string_like(&other, "xs:dayTimeDuration")?;
                    self.parse_day_time_duration(&text).map_err(|_| {
                        Error::from_code(ErrorCode::FORG0001, "invalid dayTimeDuration")
                    })
                }
            },
            "gYear" => match a {
                XdmAtomicValue::GYear { year, tz } => Ok(XdmAtomicValue::GYear { year, tz }),
                other => {
                    let text = self.require_string_like(&other, "xs:gYear")?;
                    let (year, tz) = parse_g_year(&text)
                        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gYear"))?;
                    Ok(XdmAtomicValue::GYear { year, tz })
                }
            },
            "gYearMonth" => match a {
                XdmAtomicValue::GYearMonth { year, month, tz } => {
                    Ok(XdmAtomicValue::GYearMonth { year, month, tz })
                }
                other => {
                    let text = self.require_string_like(&other, "xs:gYearMonth")?;
                    let (year, month, tz) = parse_g_year_month(&text).map_err(|_| {
                        Error::from_code(ErrorCode::FORG0001, "invalid xs:gYearMonth")
                    })?;
                    Ok(XdmAtomicValue::GYearMonth { year, month, tz })
                }
            },
            "gMonth" => match a {
                XdmAtomicValue::GMonth { month, tz } => Ok(XdmAtomicValue::GMonth { month, tz }),
                other => {
                    let text = self.require_string_like(&other, "xs:gMonth")?;
                    let (month, tz) = parse_g_month(&text)
                        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gMonth"))?;
                    Ok(XdmAtomicValue::GMonth { month, tz })
                }
            },
            "gMonthDay" => match a {
                XdmAtomicValue::GMonthDay { month, day, tz } => {
                    Ok(XdmAtomicValue::GMonthDay { month, day, tz })
                }
                other => {
                    let text = self.require_string_like(&other, "xs:gMonthDay")?;
                    let (month, day, tz) = parse_g_month_day(&text).map_err(|_| {
                        Error::from_code(ErrorCode::FORG0001, "invalid xs:gMonthDay")
                    })?;
                    Ok(XdmAtomicValue::GMonthDay { month, day, tz })
                }
            },
            "gDay" => match a {
                XdmAtomicValue::GDay { day, tz } => Ok(XdmAtomicValue::GDay { day, tz }),
                other => {
                    let text = self.require_string_like(&other, "xs:gDay")?;
                    let (day, tz) = parse_g_day(&text)
                        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gDay"))?;
                    Ok(XdmAtomicValue::GDay { day, tz })
                }
            },
            _ => Err(Error::not_implemented("cast target type")),
        }
    }
    #[allow(dead_code)]
    #[allow(dead_code)]
    fn is_castable(&self, seq: &XdmSequence<N>, t: &SingleTypeIR) -> bool {
        // Cardinality: empty sequence is castable only if optional
        if seq.is_empty() {
            return t.optional;
        }
        if seq.len() > 1 {
            return false;
        }
        // Obtain atomic (atomization semantics simplified; node => untypedAtomic)
        let item = &seq[0];
        let atomic = match item {
            XdmItem::Atomic(a) => a.clone(),
            XdmItem::Node(n) => XdmAtomicValue::UntypedAtomic(n.string_value()),
        };
        // Fast-path for QName to ensure prefix resolution requirement similar to constructor semantics.
        if t.atomic.local == "QName"
            && let XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) = &atomic
        {
            if let Some(idx) = s.find(':') {
                let p = &s[..idx];
                if p.is_empty() {
                    return false;
                }
                if p == "xml" {
                } else {
                    // look up prefix in static context
                    if !self.compiled.static_ctx.namespaces.by_prefix.contains_key(p) {
                        return false;
                    }
                }
                // local part must exist
                if idx == s.len() - 1 {
                    return false;
                }
            } else if s.is_empty() {
                return false;
            }
        }
        self.cast_atomic(atomic, &t.atomic).is_ok()
    }
    // Helper: best-effort canonical string form for debugging / fallback casts
    fn atomic_to_string(&self, a: &XdmAtomicValue) -> String {
        format!("{:?}", a)
    }
    fn parse_date(&self, s: &str) -> Result<XdmAtomicValue, crate::util::temporal::TemporalErr> {
        let (d, tz) = crate::util::temporal::parse_date_lex(s)?;
        Ok(XdmAtomicValue::Date { date: d, tz })
    }
    fn parse_time(&self, s: &str) -> Result<XdmAtomicValue, crate::util::temporal::TemporalErr> {
        let (t, tz) = crate::util::temporal::parse_time_lex(s)?;
        Ok(XdmAtomicValue::Time { time: t, tz })
    }
    fn parse_date_time(
        &self,
        s: &str,
    ) -> Result<XdmAtomicValue, crate::util::temporal::TemporalErr> {
        let (d, t, tz) = crate::util::temporal::parse_date_time_lex(s)?;
        let dt = crate::util::temporal::build_naive_datetime(d, t, tz);
        Ok(XdmAtomicValue::DateTime(dt))
    }
    fn parse_year_month_duration(&self, s: &str) -> Result<XdmAtomicValue, ()> {
        // PnYnM pattern subset
        if !s.starts_with('P') {
            return Err(());
        }
        let body = &s[1..];
        let mut years = 0;
        let mut months = 0;
        let mut cur = String::new();
        for ch in body.chars() {
            if ch.is_ascii_digit() {
                cur.push(ch);
                continue;
            }
            match ch {
                'Y' => {
                    years = cur.parse::<i32>().map_err(|_| ())?;
                    cur.clear();
                }
                'M' => {
                    months = cur.parse::<i32>().map_err(|_| ())?;
                    cur.clear();
                }
                _ => return Err(()),
            }
        }
        if !cur.is_empty() {
            return Err(());
        }
        Ok(XdmAtomicValue::YearMonthDuration(years * 12 + months))
    }
    fn parse_day_time_duration(&self, s: &str) -> Result<XdmAtomicValue, ()> {
        // PnDTnHnMnS subset (strict: at least one component)
        if !s.starts_with('P') {
            return Err(());
        }
        let body = &s[1..];
        let mut days = 0i64;
        let mut hours = 0i64;
        let mut mins = 0i64;
        let mut secs = 0i64;
        let mut cur = String::new();
        let mut time_part = false;
        let mut saw_component = false;
        for ch in body.chars() {
            if ch == 'T' {
                time_part = true;
                continue;
            }
            if ch.is_ascii_digit() {
                cur.push(ch);
                continue;
            }
            match ch {
                'D' => {
                    days = cur.parse::<i64>().map_err(|_| ())?;
                    cur.clear();
                    saw_component = true;
                }
                'H' => {
                    hours = cur.parse::<i64>().map_err(|_| ())?;
                    cur.clear();
                    saw_component = true;
                }
                'M' => {
                    if time_part {
                        mins = cur.parse::<i64>().map_err(|_| ())?;
                        cur.clear();
                        saw_component = true;
                    } else {
                        return Err(());
                    }
                }
                'S' => {
                    secs = cur.parse::<i64>().map_err(|_| ())?;
                    cur.clear();
                    saw_component = true;
                }
                _ => return Err(()),
            }
        }
        if !cur.is_empty() {
            return Err(());
        }
        if !saw_component {
            return Err(());
        } // reject bare "PT" (no component)
        let total = days * 86400 + hours * 3600 + mins * 60 + secs;
        Ok(XdmAtomicValue::DayTimeDuration(total))
    }
    #[allow(dead_code)]
    #[allow(dead_code)]
    fn assert_treat(&self, seq: &XdmSequence<N>, t: &SeqTypeIR) -> Result<(), Error> {
        // Spec oriented: produce differentiated diagnostics while keeping XPTY0004 as error code.
        use crate::compiler::ir::{OccurrenceIR, SeqTypeIR};
        let (need_min, need_max, item_type) = match t {
            SeqTypeIR::EmptySequence => {
                if !seq.is_empty() {
                    return Err(Error::from_code(
                        ErrorCode::XPTY0004,
                        "treat as empty-sequence() failed: cardinality mismatch (expected 0 got >0)",
                    ));
                }
                return Ok(());
            }
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
        let actual = seq.len();
        if actual < need_min {
            return Err(Error::from_code(
                ErrorCode::XPTY0004,
                format!(
                    "treat as failed: cardinality mismatch (expected min {} got {})",
                    need_min, actual
                ),
            ));
        }
        if let Some(max) = need_max
            && actual > max
        {
            return Err(Error::from_code(
                ErrorCode::XPTY0004,
                format!(
                    "treat as failed: cardinality mismatch (expected max {} got {})",
                    max, actual
                ),
            ));
        }
        for it in seq {
            if !self.item_matches_type(it, item_type)? {
                return Err(Error::from_code(
                    ErrorCode::XPTY0004,
                    "treat as failed: type mismatch",
                ));
            }
        }
        Ok(())
    }
    #[allow(dead_code)]
    fn instance_of(&self, _seq: &SeqTypeIR) -> Result<bool, Error> { unreachable!() }
    fn instance_of_stream(&self, stream: XdmSequenceStream<N>, t: &SeqTypeIR) -> Result<bool, Error> {
        use crate::compiler::ir::{OccurrenceIR, SeqTypeIR};
        let mut c = stream.cursor();
        match t {
            SeqTypeIR::EmptySequence => Ok(c.next_item().is_none()),
            SeqTypeIR::Typed { item, occ } => {
                let mut count = 0usize;
                while let Some(it) = c.next_item() {
                    let it = it?;
                    count = count.saturating_add(1);
                    if !self.item_matches_type(&it, item)? { return Ok(false); }
                    match occ {
                        OccurrenceIR::One | OccurrenceIR::ZeroOrOne => {
                            if count > 1 { return Ok(false); }
                        }
                        _ => {}
                    }
                }
                match occ {
                    OccurrenceIR::One => Ok(count == 1),
                    OccurrenceIR::ZeroOrOne => Ok(count <= 1),
                    OccurrenceIR::ZeroOrMore => Ok(true),
                    OccurrenceIR::OneOrMore => Ok(count >= 1),
                }
            }
        }
    }
    fn item_matches_type(
        &self,
        item: &XdmItem<N>,
        t: &crate::compiler::ir::ItemTypeIR,
    ) -> Result<bool, Error> {
        use crate::compiler::ir::ItemTypeIR;
        use XdmItem::*;
        match (item, t) {
            (_, ItemTypeIR::AnyItem) => Ok(true),
            (Node(_), ItemTypeIR::AnyNode) => Ok(true),
            (Atomic(_), ItemTypeIR::AnyNode) => Ok(false),
            (Node(n), ItemTypeIR::Kind(k)) => Ok(self.node_test(n, &k.clone())), // reuse existing node_test via IR NodeTestIR
            (Atomic(a), ItemTypeIR::Atomic(exp)) => Ok(self.atomic_matches_name(a, exp)),
            (Atomic(_), ItemTypeIR::Kind(_)) => Ok(false),
            (Node(_), ItemTypeIR::Atomic(_)) => Ok(false),
        }
    }
    fn atomic_matches_name(&self, a: &XdmAtomicValue, exp: &crate::xdm::ExpandedName) -> bool {
        use XdmAtomicValue::*;
        // Only recognize XML Schema built-ins (xs:*). Unknown namespaces do not match.
        let xs_ns = crate::consts::XS;
        if let Some(ns) = &exp.ns_uri
            && ns.as_str() != xs_ns
        {
            return false;
        }
        match exp.local.as_str() {
            // Supertype
            "anyAtomicType" => true,

            // Primitives
            "string" => matches!(
                a,
                String(_)
                    | NormalizedString(_)
                    | Token(_)
                    | Language(_)
                    | Name(_)
                    | NCName(_)
                    | NMTOKEN(_)
                    | Id(_)
                    | IdRef(_)
                    | Entity(_)
            ),
            "boolean" => matches!(a, Boolean(_)),
            "decimal" => matches!(
                a,
                Decimal(_)
                    | Integer(_)
                    | Long(_)
                    | Int(_)
                    | Short(_)
                    | Byte(_)
                    | UnsignedLong(_)
                    | UnsignedInt(_)
                    | UnsignedShort(_)
                    | UnsignedByte(_)
                    | NonPositiveInteger(_)
                    | NegativeInteger(_)
                    | NonNegativeInteger(_)
                    | PositiveInteger(_)
            ),
            "integer" => matches!(
                a,
                Integer(_)
                    | Long(_)
                    | Int(_)
                    | Short(_)
                    | Byte(_)
                    | UnsignedLong(_)
                    | UnsignedInt(_)
                    | UnsignedShort(_)
                    | UnsignedByte(_)
                    | NonPositiveInteger(_)
                    | NegativeInteger(_)
                    | NonNegativeInteger(_)
                    | PositiveInteger(_)
            ),
            "double" => matches!(a, Double(_)),
            "float" => matches!(a, Float(_)),
            "anyURI" => matches!(a, AnyUri(_)),
            "QName" => matches!(a, QName { .. }),
            "NOTATION" => matches!(a, Notation(_)),

            // Untyped
            "untypedAtomic" => matches!(a, UntypedAtomic(_)),

            // Binary
            "base64Binary" => matches!(a, Base64Binary(_)),
            "hexBinary" => matches!(a, HexBinary(_)),

            // Temporal and durations
            "dateTime" => matches!(a, DateTime(_)),
            "date" => matches!(a, Date { .. }),
            "time" => matches!(a, Time { .. }),
            "yearMonthDuration" => matches!(a, YearMonthDuration(_)),
            "dayTimeDuration" => matches!(a, DayTimeDuration(_)),

            // String-derived specifics (covered by string above but allow exact tests)
            "normalizedString" => matches!(a, NormalizedString(_)),
            "token" => matches!(a, Token(_)),
            "language" => matches!(a, Language(_)),
            "Name" => matches!(a, Name(_)),
            "NCName" => matches!(a, NCName(_)),
            "NMTOKEN" => matches!(a, NMTOKEN(_)),
            "ID" => matches!(a, Id(_)),
            "IDREF" => matches!(a, IdRef(_)),
            "ENTITY" => matches!(a, Entity(_)),

            // Unknown atomic type name -> no match
            _ => false,
        }
    }
}

// (removed IterState; for-expr now handled entirely within ForLoop opcode execution)

fn string_like_value(atom: &XdmAtomicValue) -> Option<String> {
    match atom {
        XdmAtomicValue::String(s)
        | XdmAtomicValue::UntypedAtomic(s)
        | XdmAtomicValue::NormalizedString(s)
        | XdmAtomicValue::Token(s)
        | XdmAtomicValue::Language(s)
        | XdmAtomicValue::Name(s)
        | XdmAtomicValue::NCName(s)
        | XdmAtomicValue::NMTOKEN(s)
        | XdmAtomicValue::Id(s)
        | XdmAtomicValue::IdRef(s)
        | XdmAtomicValue::Entity(s)
        | XdmAtomicValue::Notation(s)
        | XdmAtomicValue::AnyUri(s) => Some(s.clone()),
        _ => None,
    }
}

fn replace_xml_whitespace(input: &str) -> String {
    input
        .chars()
        .map(|ch| match ch {
            '\t' | '\n' | '\r' => ' ',
            other => other,
        })
        .collect()
}

fn collapse_xml_whitespace(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_space = false;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !in_space {
                out.push(' ');
                in_space = true;
            }
        } else {
            out.push(ch);
            in_space = false;
        }
    }
    while out.starts_with(' ') {
        out.remove(0);
    }
    while out.ends_with(' ') {
        out.pop();
    }
    out
}

fn is_valid_language(s: &str) -> bool {
    let mut parts = s.split('-');
    if let Some(first) = parts.next() {
        if !(1..=8).contains(&first.len()) || !first.chars().all(|c| c.is_ascii_alphabetic()) {
            return false;
        }
    } else {
        return false;
    }
    for part in parts {
        if part.is_empty() || part.len() > 8 || !part.chars().all(|c| c.is_ascii_alphanumeric()) {
            return false;
        }
    }
    true
}

fn is_name_start_char(ch: char, allow_colon: bool) -> bool {
    (allow_colon && ch == ':') || ch == '_' || ch.is_ascii_alphabetic()
}

fn is_name_char(ch: char, allow_colon: bool) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.' || (allow_colon && ch == ':')
}

fn is_valid_name(s: &str, require_start: bool, allow_colon: bool) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return !require_start;
    };
    if !is_name_start_char(first, allow_colon) {
        return false;
    }
    for ch in chars {
        if !is_name_char(ch, allow_colon) {
            return false;
        }
    }
    true
}

fn is_valid_nmtoken(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    s.chars().all(|ch| is_name_char(ch, true))
}

fn decode_hex(input: &str) -> Option<Vec<u8>> {
    if !input.len().is_multiple_of(2) {
        return None;
    }
    let mut bytes = Vec::with_capacity(input.len() / 2);
    let mut chars = input.chars();
    while let (Some(high_ch), Some(low_ch)) = (chars.next(), chars.next()) {
        let high = high_ch.to_digit(16)?;
        let low = low_ch.to_digit(16)?;
        bytes.push(((high << 4) | low) as u8);
    }
    Some(bytes)
}

fn encode_hex_upper(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{:02X}", byte));
    }
    out
}
// Cursor that atomizes items lazily
struct AtomizeCursor<N> {
    input: Box<dyn SequenceCursor<N>>,
    pending: VecDeque<XdmAtomicValue>,
}

impl<N: 'static + Send + Sync + XdmNode + Clone> AtomizeCursor<N> {
    fn new(stream: XdmSequenceStream<N>) -> Self {
        Self { input: stream.cursor(), pending: VecDeque::new() }
    }
}

impl<N: 'static + Send + Sync + XdmNode + Clone> SequenceCursor<N> for AtomizeCursor<N> {
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
                    for a in n.typed_value() { self.pending.push_back(a); }
                    if let Some(atom) = self.pending.pop_front() {
                        return Some(Ok(Atomic(atom)));
                    }
                    continue;
                }
                Err(e) => return Some(Err(e)),
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) { (0, None) }

    fn boxed_clone(&self) -> Box<dyn SequenceCursor<N>> {
        Box::new(Self { input: self.input.boxed_clone(), pending: self.pending.clone() })
    }
}

// Cursor that enforces treat as semantics while passing items through
struct TreatCursor<N> {
    vm: VmHandle<N>,
    input: Box<dyn SequenceCursor<N>>,
    item_type: crate::compiler::ir::ItemTypeIR,
    min: usize,
    max: Option<usize>,
    seen: usize,
    pending_error: Option<Error>,
}

impl<N: 'static + Send + Sync + XdmNode + Clone> TreatCursor<N> {
    fn new(vm: VmHandle<N>, stream: XdmSequenceStream<N>, t: crate::compiler::ir::SeqTypeIR) -> Self {
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

impl<N: 'static + Send + Sync + XdmNode + Clone> SequenceCursor<N> for TreatCursor<N> {
    fn next_item(&mut self) -> Option<XdmItemResult<N>> {
        if let Some(err) = self.pending_error.take() { return Some(Err(err)); }
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
                if let Some(max) = self.max && self.seen > max {
                    return Some(Err(Error::from_code(
                        ErrorCode::XPTY0004,
                        format!("treat as failed: cardinality mismatch (expected max {} got {})", max, self.seen),
                    )));
                }
                let ok = self.vm.with_vm(|vm| vm.item_matches_type(&it, &self.item_type)).unwrap_or(false);
                if !ok { return Some(Err(Error::from_code(ErrorCode::XPTY0004, "treat as failed: type mismatch"))); }
                Some(Ok(it))
            }
            Some(Err(e)) => Some(Err(e)),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) { (0, None) }
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
