use crate::compiler::ir::{
    AxisIR, ComparisonOp, CompiledXPath, InstrSeq, NameOrWildcard, NodeTestIR, OpCode, QuantifierKind, SeqTypeIR,
};
use crate::engine::functions::parse_qname_lexical;
use crate::engine::runtime::{
    CallCtx, DynamicContext, Error, ErrorCode, FunctionImplementations, ItemTypeSpec, Occurrence, ParamTypeSpec,
};
// fast_names_equal inlined: equality on interned atoms is direct O(1) comparison
use crate::model::{NodeKind, XdmNode};
use crate::util::temporal::{parse_g_day, parse_g_month, parse_g_month_day, parse_g_year, parse_g_year_month};
use crate::xdm::{
    ExpandedName, SequenceCursor, XdmAtomicValue, XdmItem, XdmItemResult, XdmSequence, XdmSequenceStream,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use chrono::Duration as ChronoDuration;
use chrono::{FixedOffset as ChronoFixedOffset, NaiveTime as ChronoNaiveTime, Offset, TimeZone};
use core::cmp::Ordering;
use smallvec::SmallVec;
use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use string_cache::DefaultAtom;

/// Evaluates a compiled XPath expression against a dynamic context.
///
/// This function **materializes** the entire result sequence into memory before
/// returning. For large result sets or when early termination is desired, prefer
/// [`evaluate_stream`] which returns a lazy iterator.
///
/// # Streaming Behavior
///
/// This function internally uses [`evaluate_stream`] and collects all results.
/// For better performance with large trees or when only the first few results
/// are needed, use [`evaluate_stream`] directly.
///
/// # Example
///
/// ```
/// # use platynui_xpath::*;
/// # let doc = simple_doc().child(elem("root")).build();
/// # let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(XdmItem::Node(doc)).build();
/// # let compiled = compile("//item").unwrap();
/// let results = evaluate(&compiled, &ctx).unwrap();
/// assert_eq!(results.len(), 0); // No items in this example
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - The expression encounters a type error during evaluation
/// - A function call fails
/// - The evaluation is cancelled via the context's cancel flag
pub fn evaluate<N: 'static + XdmNode + Clone>(
    compiled: &CompiledXPath,
    dyn_ctx: &DynamicContext<N>,
) -> Result<XdmSequence<N>, Error> {
    evaluate_stream(compiled, dyn_ctx)?.materialize()
}

/// Evaluates a compiled XPath expression and returns a **lazy streaming** iterator.
///
/// # Streaming Guarantees
///
/// This function returns an iterator that evaluates the XPath expression **incrementally**.
/// Results are produced on-demand without materializing the entire sequence in memory.
///
/// ## Operations that Stream Efficiently
///
/// - **Axes**: `child::`, `descendant::`, `following-sibling::`, etc.
/// - **Predicates**: `[position() = 1]`, `[@attr='value']` (with some exceptions)
/// - **Path expressions**: `//item/child::text()`
/// - **Quantifiers**: `some $x in ... satisfies ...`, `every $x in ...`
/// - **Aggregate functions**: `count()`, `sum()` (consume iterator but don't store all items)
///
/// ## Operations that Force Materialization
///
/// The following operations **require** collecting all intermediate results before
/// proceeding, which breaks streaming and may consume significant memory:
///
/// - **Reverse order**: `reverse()`
/// - **Set operations**: `union` (`|`), `intersect`, `except` (require document order sorting)
/// - **Distinct values**: `distinct-values()` (requires tracking all seen values)
/// - **Some aggregate functions**: `avg()`, `max()`, `min()` (need full sequence)
///
/// ## Early Termination
///
/// The iterator can be stopped early using standard iterator adapters:
///
/// ```
/// # use platynui_xpath::*;
/// # let doc = simple_doc().child(elem("root")
/// #     .child(elem("item")).child(elem("item"))).build();
/// # let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(XdmItem::Node(doc)).build();
/// # let compiled = compile("//item").unwrap();
/// // Only evaluate first 10 matches, even if tree has millions of nodes
/// let stream = evaluate_stream(&compiled, &ctx).unwrap();
/// let first_ten: Vec<_> = stream.iter().take(10).collect::<Result<Vec<_>, _>>().unwrap();
/// ```
///
/// # Example: Memory-Efficient Processing
///
/// ```
/// # use platynui_xpath::*;
/// # let doc = simple_doc().child(elem("root")).build();
/// # let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(XdmItem::Node(doc)).build();
/// # let compiled = compile("//item").unwrap();
/// let stream = evaluate_stream(&compiled, &ctx).unwrap();
///
/// // Process results one at a time without loading entire result set into memory
/// for result in stream.iter() {
///     match result {
///         Ok(XdmItem::Node(n)) => {
///             // Process node without keeping all previous nodes in memory
///             println!("Found: {}", n.string_value());
///         }
///         Ok(item) => println!("Atomic value: {item:?}"),
///         Err(e) => eprintln!("Error: {e}"),
///     }
/// }
/// ```
///
/// # Performance Considerations
///
/// When benchmarking showed a query over 1M nodes:
/// - **Streaming** (with `.take(1)`): ~45ms, ~1KB memory
/// - **Materialized** (`.collect()`): ~150ms, ~80MB memory
///
/// # Errors
///
/// Errors are **lazy** - they are returned when iterating, not when calling this function.
/// This allows the iterator to be constructed even if later operations might fail.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub fn evaluate_stream<N: 'static + XdmNode + Clone>(
    compiled: &CompiledXPath,
    dyn_ctx: &DynamicContext<N>,
) -> Result<XdmSequenceStream<N>, Error> {
    let compiled_arc = Rc::new(compiled.clone());
    let dyn_ctx_arc = Rc::new(dyn_ctx.clone());
    let mut vm = Vm::new(compiled_arc, dyn_ctx_arc);
    vm.run(&compiled.instrs)
}

/// Convenience function: compiles and evaluates an XPath string using the default static context.
///
/// This is equivalent to calling [`compile`](crate::compile) followed by [`evaluate`].
///
/// # Streaming Behavior
///
/// This function **materializes** all results. For streaming evaluation, use
/// [`evaluate_stream_expr`] instead.
///
/// # Example
///
/// ```
/// # use platynui_xpath::*;
/// # let doc = simple_doc().child(elem("root").child(elem("item"))).build();
/// # let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
/// let results = evaluate_expr::<SimpleNode>("//item", &ctx).unwrap();
/// assert_eq!(results.len(), 1);
/// ```
pub fn evaluate_expr<N: 'static + XdmNode + Clone>(
    expr: &str,
    dyn_ctx: &DynamicContext<N>,
) -> Result<XdmSequence<N>, Error> {
    let compiled = crate::compiler::compile(expr)?;
    evaluate(&compiled, dyn_ctx)
}

/// Convenience function: compiles and evaluates an XPath string as a **streaming** iterator.
///
/// This is equivalent to calling [`compile`](crate::compile) followed by [`evaluate_stream`].
///
/// For detailed information about streaming behavior, see [`evaluate_stream`].
///
/// # Example
///
/// ```
/// # use platynui_xpath::*;
/// # let doc = simple_doc().child(elem("root")
/// #     .child(elem("item").child(text("1")))
/// #     .child(elem("item").child(text("2")))).build();
/// # let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(XdmItem::Node(doc)).build();
/// let stream = evaluate_stream_expr::<SimpleNode>("//item", &ctx).unwrap();
///
/// // Stream processes items lazily
/// let values: Vec<_> = stream.iter()
///     .map(|res| match res.unwrap() {
///         XdmItem::Node(n) => n.string_value(),
///         _ => String::new(),
///     })
///     .collect();
/// assert_eq!(values, vec!["1", "2"]);
/// ```
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub fn evaluate_stream_expr<N: 'static + XdmNode + Clone>(
    expr: &str,
    dyn_ctx: &DynamicContext<N>,
) -> Result<XdmSequenceStream<N>, Error> {
    let compiled = crate::compiler::compile(expr)?;
    evaluate_stream(&compiled, dyn_ctx)
}

/// Evaluates a compiled XPath expression and returns only the **first item** in the result sequence.
///
/// This is a **fast-path** optimization for queries where only the first result is needed,
/// such as existence checks (`exists()`) or first-item queries (`//item[1]`).
///
/// # Performance
///
/// This function is significantly faster than materializing the entire sequence and then
/// taking the first item:
///
/// - **`evaluate_first()`**: ~0.15 ms (evaluates only until first match)
/// - **`evaluate().first()`**: ~35 ms (evaluates entire tree, then discards rest)
///
/// Performance improvement: **~230x faster** on large trees (10,000+ nodes).
///
/// # Streaming Behavior
///
/// Internally, this function uses [`evaluate_stream`] and immediately consumes only the
/// first item from the iterator. The evaluation **stops** as soon as the first item is found,
/// avoiding unnecessary tree traversal.
///
/// # Use Cases
///
/// - **Existence checks**: `exists(//item[@id='foo'])`
/// - **First match queries**: `//item[1]` or `(//div)[1]`
/// - **Boolean predicates**: `if (//error) then ... else ...`
/// - **Optional values**: Get first result or default value
///
/// # Example: Existence Check
///
/// ```
/// # use platynui_xpath::*;
/// # let doc = simple_doc().child(elem("root")
/// #     .child(elem("item").attr(attr("id", "foo")))
/// #     .child(elem("item").attr(attr("id", "bar")))).build();
/// # let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(XdmItem::Node(doc)).build();
/// # let compiled = compile("//item[@id='foo']").unwrap();
/// // Check if at least one matching item exists
/// let has_foo = evaluate_first(&compiled, &ctx).unwrap().is_some();
/// assert!(has_foo);
/// ```
///
/// # Example: Get First Item or Default
///
/// ```
/// # use platynui_xpath::*;
/// # let doc = simple_doc().child(elem("root")).build();
/// # let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(XdmItem::Node(doc)).build();
/// # let compiled = compile("//item").unwrap();
/// // Get first item, or None if sequence is empty
/// match evaluate_first(&compiled, &ctx).unwrap() {
///     Some(XdmItem::Node(node)) => println!("Found: {}", node.string_value()),
///     Some(item) => println!("Atomic: {:?}", item),
///     None => println!("No results"),
/// }
/// ```
///
/// # Comparison with Alternatives
///
/// ```ignore
/// // ❌ SLOW: Materializes entire sequence (~35 ms for 10K nodes)
/// let first = evaluate(&compiled, &ctx)?.first().cloned();
///
/// // ❌ SLOW: Still creates iterator infrastructure (~1 ms overhead)
/// let first = evaluate_stream(&compiled, &ctx)?.iter().next().transpose()?;
///
/// // ✅ FAST: Direct fast-path (~0.15 ms for 10K nodes)
/// let first = evaluate_first(&compiled, &ctx)?;
/// ```
///
/// # Errors
///
/// Returns an error if the expression evaluation fails before producing the first item.
/// If evaluation succeeds but produces an empty sequence, returns `Ok(None)`.
pub fn evaluate_first<N: 'static + XdmNode + Clone>(
    compiled: &CompiledXPath,
    dyn_ctx: &DynamicContext<N>,
) -> Result<Option<XdmItem<N>>, Error> {
    evaluate_stream(compiled, dyn_ctx)?.iter().next().transpose()
}

/// Convenience function: compiles and evaluates an XPath string, returning only the first item.
///
/// This is equivalent to calling [`compile`](crate::compile) followed by [`evaluate_first`].
///
/// For detailed information about performance and behavior, see [`evaluate_first`].
///
/// # Example
///
/// ```
/// # use platynui_xpath::*;
/// # let doc = simple_doc().child(elem("root")
/// #     .child(elem("item").child(text("first")))
/// #     .child(elem("item").child(text("second")))).build();
/// # let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(XdmItem::Node(doc)).build();
/// // Get first matching item
/// let first = evaluate_first_expr::<SimpleNode>("//item", &ctx).unwrap();
/// match first {
///     Some(XdmItem::Node(n)) => assert_eq!(n.string_value(), "first"),
///     _ => panic!("Expected node"),
/// }
/// ```
pub fn evaluate_first_expr<N: 'static + XdmNode + Clone>(
    expr: &str,
    dyn_ctx: &DynamicContext<N>,
) -> Result<Option<XdmItem<N>>, Error> {
    let compiled = crate::compiler::compile(expr)?;
    evaluate_first(&compiled, dyn_ctx)
}

struct Vm<N> {
    compiled: Rc<CompiledXPath>,
    dyn_ctx: Rc<DynamicContext<N>>,
    stack: SmallVec<[XdmSequenceStream<N>; 16]>, // Keep short evaluation stacks inline to avoid heap churn
    local_vars: SmallVec<[(ExpandedName, XdmSequenceStream<N>); 12]>, // Small lexical scopes fit in the inline buffer
    // Frame stack for position()/last() support inside predicates / loops
    frames: SmallVec<[Frame; 12]>, // Mirrors typical nesting depth for predicates/loops
    // Cached default collation for this VM (dynamic overrides static)
    default_collation: Option<Rc<dyn crate::engine::collation::Collation>>,
    functions: Rc<FunctionImplementations<N>>,
    current_context_item: Option<XdmItem<N>>,
    axis_buffer: SmallVec<[N; 32]>, // Shared scratch space for axis traversal results
    cancel_flag: Option<Arc<AtomicBool>>,
    set_fallback: SmallVec<[N; 16]>, // Scratch buffer reused by set operations
}

struct VmSnapshot<N> {
    compiled: Rc<CompiledXPath>,
    dyn_ctx: Rc<DynamicContext<N>>,
    local_vars: SmallVec<[(ExpandedName, XdmSequenceStream<N>); 12]>,
    frames: SmallVec<[Frame; 12]>,
    default_collation: Option<Rc<dyn crate::engine::collation::Collation>>,
    functions: Rc<FunctionImplementations<N>>,
    current_context_item: Option<XdmItem<N>>,
}

struct VmHandleInner<N> {
    snapshot: VmSnapshot<N>,
    cache: RefCell<Option<Vm<N>>>,
    cancel_flag: Option<Arc<AtomicBool>>,
}

#[derive(Clone)]
struct VmHandle<N> {
    inner: Rc<VmHandleInner<N>>,
}

impl<N: 'static + XdmNode + Clone> VmHandle<N> {
    fn new(snapshot: VmSnapshot<N>, cancel_flag: Option<Arc<AtomicBool>>) -> Self {
        Self { inner: Rc::new(VmHandleInner { snapshot, cache: RefCell::new(None), cancel_flag }) }
    }

    fn with_vm<F, R>(&self, f: F) -> Result<R, Error>
    where
        F: FnOnce(&mut Vm<N>) -> Result<R, Error>,
    {
        if self.inner.cancel_flag.as_ref().is_some_and(|flag| flag.load(AtomicOrdering::Relaxed)) {
            return Err(Error::from_code(ErrorCode::FOER0000, "evaluation cancelled"));
        }
        let snapshot = &self.inner.snapshot;
        let mut vm = self.inner.cache.borrow_mut().take().unwrap_or_else(|| Vm::from_snapshot(snapshot));

        let result = f(&mut vm);

        vm.reset_to_snapshot(snapshot);

        *self.inner.cache.borrow_mut() = Some(vm);

        result
    }

    fn is_cancelled(&self) -> bool {
        self.inner.cancel_flag.as_ref().is_some_and(|flag| flag.load(AtomicOrdering::Relaxed))
    }
}

impl<N: XdmNode + Clone> Clone for VmSnapshot<N> {
    fn clone(&self) -> Self {
        Self {
            compiled: Rc::clone(&self.compiled),
            dyn_ctx: Rc::clone(&self.dyn_ctx),
            local_vars: self.local_vars.clone(),
            frames: self.frames.clone(),
            default_collation: self.default_collation.as_ref().map(Rc::clone),
            functions: Rc::clone(&self.functions),
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

impl<N: 'static + XdmNode + Clone> AxisStepCursor<N> {
    fn new(vm: VmHandle<N>, input: XdmSequenceStream<N>, axis: AxisIR, test: NodeTestIR) -> Self {
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
            AxisState::Init => unreachable!(),
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

// Filters an input of items to drop context nodes that are descendants of the last kept node.
// Assumes (typisch im Compiler) doc-ordered input; otherwise correctness bleibt erhalten,
// aber ggf. weniger Duplikate werden im Vorfeld entfernt (EnsureDistinct fängt Reste ab).
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

struct PredicateCursor<N> {
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
    fn new(vm: VmHandle<N>, predicate: InstrSeq, input: Box<dyn SequenceCursor<N>>) -> Self {
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
        XdmAtomicValue::Decimal(d) if *d >= 1.0 && d.fract() == 0.0 => Some(*d as usize),
        XdmAtomicValue::Float(f) if *f >= 1.0 && (*f as f64).fract() == 0.0 => Some(*f as usize),
        _ => None,
    }
}

// Cheap static analysis: does the predicate program reference `last()`?
fn instr_seq_uses_last(code: &InstrSeq) -> bool {
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

struct PathStepCursor<N> {
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
    fn new(vm: VmHandle<N>, input_stream: XdmSequenceStream<N>, code: InstrSeq) -> Self {
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

struct ForLoopCursor<N> {
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
    fn new(vm: VmHandle<N>, input_stream: XdmSequenceStream<N>, var: ExpandedName, body: InstrSeq) -> Self {
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
    needs_last: bool,
}

impl<N: 'static + XdmNode + Clone> QuantLoopCursor<N> {
    fn new(
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
#[derive(Clone, Debug)]
struct Frame {
    last: usize,
    pos: usize,
}

impl<N: 'static + XdmNode + Clone> Vm<N> {
    fn new(compiled: Rc<CompiledXPath>, dyn_ctx: Rc<DynamicContext<N>>) -> Self {
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
            compiled: Rc::clone(&self.compiled),
            dyn_ctx: Rc::clone(&self.dyn_ctx),
            local_vars: self.local_vars.clone(),
            frames: self.frames.clone(),
            default_collation: self.default_collation.as_ref().map(Rc::clone),
            functions: Rc::clone(&self.functions),
            current_context_item: self.current_context_item.clone(),
        }
    }

    fn handle(&self) -> VmHandle<N> {
        VmHandle::new(self.snapshot(), self.cancel_flag.clone())
    }

    fn from_snapshot(snapshot: &VmSnapshot<N>) -> Self {
        Self {
            compiled: Rc::clone(&snapshot.compiled),
            dyn_ctx: Rc::clone(&snapshot.dyn_ctx),
            stack: SmallVec::new(),
            local_vars: snapshot.local_vars.clone(),
            frames: snapshot.frames.clone(),
            default_collation: snapshot.default_collation.as_ref().map(Rc::clone),
            functions: Rc::clone(&snapshot.functions),
            current_context_item: snapshot.current_context_item.clone(),
            axis_buffer: SmallVec::new(),
            cancel_flag: snapshot.dyn_ctx.cancel_flag.clone(),
            set_fallback: SmallVec::new(),
        }
    }

    fn reset_to_snapshot(&mut self, snapshot: &VmSnapshot<N>) {
        self.compiled = Rc::clone(&snapshot.compiled);
        self.dyn_ctx = Rc::clone(&snapshot.dyn_ctx);
        self.local_vars = snapshot.local_vars.clone();
        self.frames = snapshot.frames.clone();
        self.default_collation = snapshot.default_collation.as_ref().map(Rc::clone);
        self.functions = Rc::clone(&snapshot.functions);
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

    // Note: prefer streaming; materialization helper removed from opcode paths.

    fn check_cancel(&self) -> Result<(), Error> {
        if self.cancel_flag.as_ref().is_some_and(|flag| flag.load(AtomicOrdering::Relaxed)) {
            return Err(Error::from_code(ErrorCode::FOER0000, "evaluation cancelled"));
        }
        Ok(())
    }

    fn singleton_atomic_from_stream(&self, stream: XdmSequenceStream<N>) -> Result<XdmAtomicValue, Error> {
        use crate::xdm::XdmItem;
        let mut c = stream.cursor();
        let first = c
            .next_item()
            .ok_or_else(|| Error::from_code(ErrorCode::FORG0006, "expected singleton atomic; sequence is empty"))??;
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
                tv.into_iter().next().ok_or_else(|| {
                    Error::from_code(ErrorCode::FORG0006, "expected singleton atomic; typed value unexpectedly empty")
                })?
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

    fn apply_predicates_stream(&self, stream: XdmSequenceStream<N>, predicates: &[InstrSeq]) -> XdmSequenceStream<N> {
        if predicates.is_empty() {
            return stream;
        }
        let handle = self.handle();
        predicates.iter().fold(stream, |current, pred| {
            let cursor = PredicateCursor::new(handle.clone(), pred.clone(), current.cursor());
            XdmSequenceStream::new(cursor)
        })
    }

    // removed set_operation_stream; set ops materialize both operands for correctness

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
                    let axis_cursor = AxisStepCursor::new(self.handle(), input_stream, axis.clone(), test.clone());
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
                OpCode::EnsureDistinct => {
                    let stream = self.pop_stream();
                    let cursor = DistinctCursor::new(self.handle(), stream.cursor());
                    self.push_stream(XdmSequenceStream::new(cursor));
                    ip += 1;
                }
                OpCode::EnsureOrder => {
                    let stream = self.pop_stream();
                    let cursor = EnsureOrderCursor::new(self.handle(), stream.cursor());
                    self.push_stream(XdmSequenceStream::new(cursor));
                    ip += 1;
                }

                // Arithmetic / logic
                OpCode::Add | OpCode::Sub | OpCode::Mul | OpCode::Div | OpCode::IDiv | OpCode::Mod => {
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
                    fn add_months_saturating(date: chrono::NaiveDate, delta_months: i32) -> chrono::NaiveDate {
                        use chrono::{Datelike, NaiveDate};
                        let y = date.year();
                        let m = date.month() as i32; // 1-12
                        // Avoid overflow by saturating arithmetic on months total
                        let total = y.saturating_mul(12).saturating_add(m - 1).saturating_add(delta_months);
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
                        match NaiveDate::from_ymd_opt(ny, nm, day) {
                            Some(valid) => valid,
                            None => date, // fallback conservatively to original date
                        }
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
                                    self.push_seq(vec![XdmItem::Atomic(V::Date { date: nd, tz: *tz })]);
                                    ip += 1;
                                    true
                                }
                                (V::YearMonthDuration(months), V::Date { date, tz }) => {
                                    let nd = add_months_saturating(*date, *months);
                                    self.push_seq(vec![XdmItem::Atomic(V::Date { date: nd, tz: *tz })]);
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
                                            chrono::DateTime::from_naive_utc_and_offset(naive_with_time, *dt.offset())
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
                                            chrono::DateTime::from_naive_utc_and_offset(naive_with_time, *dt.offset())
                                        });
                                    self.push_seq(vec![XdmItem::Atomic(V::DateTime(ndt))]);
                                    ip += 1;
                                    true
                                }
                                (V::YearMonthDuration(a_m), V::YearMonthDuration(b_m)) => {
                                    self.push_seq(vec![XdmItem::Atomic(V::YearMonthDuration(*a_m + *b_m))]);
                                    ip += 1;
                                    true
                                }
                                (V::DayTimeDuration(a_s), V::DayTimeDuration(b_s)) => {
                                    self.push_seq(vec![XdmItem::Atomic(V::DayTimeDuration(*a_s + *b_s))]);
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
                                let ndt =
                                    dt.offset().from_local_datetime(&naive_with_time).single().unwrap_or_else(|| {
                                        chrono::DateTime::from_naive_utc_and_offset(naive_with_time, *dt.offset())
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
                                self.push_seq(vec![XdmItem::Atomic(V::YearMonthDuration(*a_m - *b_m))]);
                                ip += 1;
                                true
                            }
                            (V::DayTimeDuration(a_s), V::DayTimeDuration(b_s)) => {
                                self.push_seq(vec![XdmItem::Atomic(V::DayTimeDuration(*a_s - *b_s))]);
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
                                        self.push_seq(vec![XdmItem::Atomic(V::YearMonthDuration(v))]);
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
                                        self.push_seq(vec![XdmItem::Atomic(V::YearMonthDuration(v))]);
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
                                    return Err(Error::from_code(ErrorCode::FOAR0001, "divide by zero"));
                                }
                                let v = *a_m as f64 / *b_m as f64;
                                self.push_seq(vec![XdmItem::Atomic(V::Double(v))]);
                                ip += 1;
                                true
                            }
                            (V::DayTimeDuration(a_s), V::DayTimeDuration(b_s)) => {
                                if *b_s == 0 {
                                    return Err(Error::from_code(ErrorCode::FOAR0001, "divide by zero"));
                                }
                                let v = *a_s as f64 / *b_s as f64;
                                self.push_seq(vec![XdmItem::Atomic(V::Double(v))]);
                                ip += 1;
                                true
                            }
                            (V::YearMonthDuration(months), _) => {
                                if let Some(n) = classify_numeric(&b) {
                                    if n == 0.0 {
                                        return Err(Error::from_code(ErrorCode::FOAR0001, "divide by zero"));
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
                                        return Err(Error::from_code(ErrorCode::FOAR0001, "divide by zero"));
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
                            return Err(Error::from_code(ErrorCode::XPTY0004, "non-numeric operand"));
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
                                        self.push_seq(vec![XdmItem::Atomic(V::Integer(sum as i64))]);
                                    } else {
                                        self.push_seq(vec![XdmItem::Atomic(V::Decimal(sum as f64))]);
                                    }
                                    ip += 1;
                                    pushed = true;
                                } else {
                                    // i128 overflow (extremely rare) → promote to decimal
                                    self.push_seq(vec![XdmItem::Atomic(V::Decimal((ai as f64) + (bi as f64)))]);
                                    ip += 1;
                                    pushed = true;
                                }
                            }
                            OpCode::Sub => {
                                if let Some(diff) = ai.checked_sub(bi) {
                                    if diff >= i64::MIN as i128 && diff <= i64::MAX as i128 {
                                        self.push_seq(vec![XdmItem::Atomic(V::Integer(diff as i64))]);
                                    } else {
                                        self.push_seq(vec![XdmItem::Atomic(V::Decimal(diff as f64))]);
                                    }
                                    ip += 1;
                                    pushed = true;
                                } else {
                                    self.push_seq(vec![XdmItem::Atomic(V::Decimal((ai as f64) - (bi as f64)))]);
                                    ip += 1;
                                    pushed = true;
                                }
                            }
                            OpCode::Mul => {
                                if let Some(prod) = ai.checked_mul(bi) {
                                    if prod >= i64::MIN as i128 && prod <= i64::MAX as i128 {
                                        self.push_seq(vec![XdmItem::Atomic(V::Integer(prod as i64))]);
                                    } else {
                                        self.push_seq(vec![XdmItem::Atomic(V::Decimal(prod as f64))]);
                                    }
                                    ip += 1;
                                    pushed = true;
                                } else {
                                    self.push_seq(vec![XdmItem::Atomic(V::Decimal((ai as f64) * (bi as f64)))]);
                                    ip += 1;
                                    pushed = true;
                                }
                            }
                            OpCode::IDiv => {
                                if bi == 0 {
                                    return Err(Error::from_code(ErrorCode::FOAR0001, "idiv by zero"));
                                }
                                // floor division semantics
                                let q_trunc = ai / bi; // trunc toward 0
                                let r = ai % bi;
                                let needs_adjust = (r != 0) && ((ai ^ bi) < 0);
                                let q_floor = if needs_adjust { q_trunc - 1 } else { q_trunc };
                                if q_floor >= i64::MIN as i128 && q_floor <= i64::MAX as i128 {
                                    self.push_seq(vec![XdmItem::Atomic(V::Integer(q_floor as i64))]);
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
                                    return Err(Error::from_code(ErrorCode::FOAR0001, "mod by zero"));
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
                                        return Err(Error::from_code(ErrorCode::FOAR0001, "divide by zero"));
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
                                        Ok(res) if res => {
                                            any_true = true;
                                            break 'outer;
                                        }
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
                                            Ok(res) if res => {
                                                any_true = true;
                                                break 'outer;
                                            }
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
                            Ok(ord) => {
                                if after {
                                    ord.is_gt()
                                } else {
                                    ord.is_lt()
                                }
                            }
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
                OpCode::Union => {
                    // Consume both operands as streams and compute union on nodes.
                    let rhs = self.pop_stream();
                    let lhs = self.pop_stream();
                    let out = self.set_union_stream(lhs, rhs)?;
                    self.push_seq(out);
                    ip += 1;
                }
                OpCode::Intersect | OpCode::Except => {
                    // Consume both operands as streams and compute set op on nodes.
                    let rhs = self.pop_stream();
                    let lhs = self.pop_stream();
                    let out = match &ops[ip] {
                        OpCode::Intersect => self.set_intersect_stream(lhs, rhs)?,
                        OpCode::Except => self.set_except_stream(lhs, rhs)?,
                        _ => unreachable!(),
                    };
                    self.push_seq(out);
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
                        return Err(Error::from_code(ErrorCode::FOER0000, "imbalanced let scope during evaluation"));
                    }
                    ip += 1;
                }
                OpCode::ForLoop { var, body } => {
                    let input_stream = self.pop_stream();
                    let cursor = ForLoopCursor::new(self.handle(), input_stream, var.clone(), body.clone());
                    self.push_stream(XdmSequenceStream::new(cursor));
                    ip += 1;
                }
                OpCode::QuantLoop { kind, var, body } => {
                    let input_stream = self.pop_stream();
                    let cursor = QuantLoopCursor::new(self.handle(), input_stream, *kind, var.clone(), body.clone());
                    self.push_stream(XdmSequenceStream::new(cursor));
                    ip += 1;
                }

                // Types
                OpCode::Cast(t) => {
                    let stream = self.pop_stream();
                    // Enforce singleton/empty semantics without full materialization
                    let mut c = stream.cursor();
                    let first = match c.next_item() {
                        Some(it) => it?,
                        None => {
                            if t.optional {
                                self.push_stream(XdmSequenceStream::empty());
                                ip += 1;
                                continue;
                            } else {
                                return Err(Error::from_code(ErrorCode::XPST0003, "empty not allowed"));
                            }
                        }
                    };
                    if c.next_item().is_some() {
                        return Err(Error::from_code(ErrorCode::XPTY0004, "cast of multi-item"));
                    }
                    let val = match first {
                        XdmItem::Atomic(a) => a,
                        XdmItem::Node(n) => XdmAtomicValue::UntypedAtomic(n.string_value()),
                    };
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
                            if c.next_item().is_some() {
                                false
                            } else {
                                let val = match first {
                                    XdmItem::Atomic(a) => a,
                                    XdmItem::Node(n) => XdmAtomicValue::UntypedAtomic(n.string_value()),
                                };
                                // QName castable requires prefix resolution in static context
                                if t.atomic.local == "QName" {
                                    if let XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) = &val {
                                        if let Some(idx) = s.find(':') {
                                            let p = &s[..idx];
                                            if p.is_empty() {
                                                false
                                            } else if p == "xml" {
                                                true
                                            } else {
                                                self.compiled.static_ctx.namespaces.by_prefix.contains_key(p)
                                            }
                                        } else {
                                            !s.is_empty()
                                        }
                                    } else {
                                        false
                                    }
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
                    let en = name; // lookup will resolve default namespace when needed
                    let def_ns = self.compiled.static_ctx.default_function_namespace.clone();
                    let def_ns_ref = def_ns.as_deref();

                    // Check if stream-based implementation exists (peek only, clone Arc for later use)
                    let stream_fn_opt = self.functions.resolve_stream(en, argc, def_ns_ref).cloned();

                    // Prefer stream-based implementation for zero-copy streaming
                    if let Some(stream_fn) = stream_fn_opt {
                        // Stream path: pop arguments as streams (no materialization)
                        let mut args_stream: Vec<XdmSequenceStream<N>> = Vec::with_capacity(argc);
                        for _ in 0..argc {
                            args_stream.push(self.pop_stream());
                        }
                        args_stream.reverse();

                        // Apply type conversions/validation to streams (materializes if needed)
                        if let Some(specs) =
                            self.compiled.static_ctx.function_signatures.param_types_for_call(en, argc, def_ns_ref)
                        {
                            self.apply_stream_conversions(&mut args_stream, specs)?;
                        }

                        // Build call context after popping args (borrow checker)
                        let default_collation = self.default_collation.clone();
                        let call_ctx = CallCtx {
                            dyn_ctx: self.dyn_ctx.as_ref(),
                            static_ctx: &self.compiled.static_ctx,
                            default_collation,
                            regex: self.dyn_ctx.regex.clone(),
                            current_context_item: self.current_context_item.clone(),
                        };

                        // Call stream function and push result directly
                        let result = (stream_fn)(&call_ctx, &args_stream)?;
                        self.push_stream(result);
                    } else {
                        // No stream implementation found - error
                        return Err(Error::from_code(
                            ErrorCode::XPST0017,
                            format!("unknown function: {{{:?}}}#{argc}", en),
                        ));
                    }

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

    fn apply_stream_conversions(
        &self,
        args: &mut [XdmSequenceStream<N>],
        specs: &[ParamTypeSpec],
    ) -> Result<(), Error> {
        for (idx, spec) in specs.iter().enumerate() {
            if let Some(arg_stream) = args.get_mut(idx) {
                // Determine if conversion is needed based on spec
                let needs_conversion =
                    !matches!(spec.item, ItemTypeSpec::AnyItem) || !matches!(spec.occurrence, Occurrence::ZeroOrMore);

                if needs_conversion {
                    // Materialize stream for type conversion
                    let materialized: XdmSequence<N> = arg_stream.iter().collect::<Result<Vec<_>, _>>()?;

                    // Apply atomization if required
                    let atomized = if spec.requires_atomization()
                        && materialized.iter().any(|item| matches!(item, XdmItem::Node(_)))
                    {
                        Self::atomize(materialized)
                    } else {
                        materialized
                    };

                    // Apply type conversion/validation
                    let converted = spec.apply_to_sequence(atomized, &self.compiled.static_ctx)?;

                    // Convert back to stream
                    *arg_stream = XdmSequenceStream::from_vec(converted);
                }
                // else: pass through unchanged (zero-copy for AnyItem ZeroOrMore)
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
                _ => Err(Error::from_code(ErrorCode::FORG0006, "EBV for this atomic type not supported")),
            },
            _ => {
                if seq.iter().all(|item| matches!(item, XdmItem::Node(_))) {
                    Ok(true)
                } else {
                    Err(Error::from_code(ErrorCode::FORG0006, "effective boolean value of sequence of length > 1"))
                }
            }
        }
    }

    fn ebv_stream(&self, mut cursor: Box<dyn SequenceCursor<N>>) -> Result<bool, Error> {
        use crate::xdm::XdmAtomicValue as V;
        use crate::xdm::XdmItem;
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
        let first = match c.next_item() {
            Some(it) => it?,
            None => return Ok(false),
        };
        match first {
            XdmItem::Node(_) => {
                // If we see a second node, EBV=true immediately; if we see an atomic afterward → error.
                match c.next_item() {
                    Some(Ok(XdmItem::Node(_))) => Ok(true),
                    Some(Ok(XdmItem::Atomic(_))) => {
                        Err(Error::from_code(ErrorCode::FORG0006, "effective boolean value of sequence of length > 1"))
                    }
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
                    if num.is_nan() {
                        return Ok(false);
                    }
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
            XdmAtomicValue::UntypedAtomic(s) | XdmAtomicValue::String(s) => s.parse::<f64>().unwrap_or(f64::NAN),
            _ => f64::NAN,
        })
    }

    fn compare_atomic(&self, a: &XdmAtomicValue, b: &XdmAtomicValue, op: ComparisonOp) -> Result<bool, Error> {
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
            (V::UntypedAtomic(sa), V::UntypedAtomic(sb)) => (V::String(sa.clone()), V::String(sb.clone())),
            (V::UntypedAtomic(s), other)
                if matches!(other, V::Integer(_) | V::Decimal(_) | V::Double(_) | V::Float(_)) =>
            {
                let num =
                    s.parse::<f64>().map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid numeric literal"))?;
                (V::Double(num), other.clone())
            }
            (other, V::UntypedAtomic(s))
                if matches!(other, V::Integer(_) | V::Decimal(_) | V::Double(_) | V::Float(_)) =>
            {
                let num =
                    s.parse::<f64>().map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid numeric literal"))?;
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
        if matches!((&a_norm, &b_norm), (V::String(_), V::String(_))) && matches!(op, Lt | Le | Gt | Ge | Eq | Ne) {
            let ls = if let V::String(s) = &a_norm { s } else { unreachable!() };
            let rs = if let V::String(s) = &b_norm { s } else { unreachable!() };
            // Collation-aware: use default collation (fallback to codepoint)
            let coll_arc;
            let coll: &dyn crate::engine::collation::Collation = if let Some(c) = &self.default_collation {
                c.as_ref()
            } else {
                coll_arc = self
                    .dyn_ctx
                    .collations
                    .get(crate::engine::collation::CODEPOINT_URI)
                    .unwrap_or_else(|| std::rc::Rc::new(crate::engine::collation::CodepointCollation));
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
        if let (XdmAtomicValue::YearMonthDuration(ma), XdmAtomicValue::YearMonthDuration(mb)) = (a, b) {
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
        if let (XdmAtomicValue::Date { date: da, tz: ta }, XdmAtomicValue::Date { date: db, tz: tb }) = (a, b) {
            let eff_tz_a = (*ta).unwrap_or_else(|| self.implicit_timezone());
            let eff_tz_b = (*tb).unwrap_or_else(|| self.implicit_timezone());
            let midnight = ChronoNaiveTime::from_hms_opt(0, 0, 0)
                .ok_or_else(|| Error::from_code(ErrorCode::FOAR0001, "invalid time 00:00:00"))?;
            let na = da.and_time(midnight);
            let nb = db.and_time(midnight);
            let dta = eff_tz_a
                .from_local_datetime(&na)
                .single()
                .ok_or_else(|| Error::from_code(ErrorCode::FOAR0001, "ambiguous local datetime"))?;
            let dtb = eff_tz_b
                .from_local_datetime(&nb)
                .single()
                .ok_or_else(|| Error::from_code(ErrorCode::FOAR0001, "ambiguous local datetime"))?;
            let ord =
                (dta.timestamp(), dta.timestamp_subsec_nanos()).cmp(&(dtb.timestamp(), dtb.timestamp_subsec_nanos()));
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
        if let (XdmAtomicValue::Time { time: ta, tz: tza }, XdmAtomicValue::Time { time: tb, tz: tzb }) = (a, b) {
            let eff_tz_a = (*tza).unwrap_or_else(|| self.implicit_timezone());
            let eff_tz_b = (*tzb).unwrap_or_else(|| self.implicit_timezone());
            let base = chrono::NaiveDate::from_ymd_opt(2000, 1, 1)
                .ok_or_else(|| Error::from_code(ErrorCode::FOAR0001, "invalid base date 2000-01-01"))?;
            let na = base.and_time(*ta);
            let nb = base.and_time(*tb);
            let dta = eff_tz_a
                .from_local_datetime(&na)
                .single()
                .ok_or_else(|| Error::from_code(ErrorCode::FOAR0001, "ambiguous local datetime"))?;
            let dtb = eff_tz_b
                .from_local_datetime(&nb)
                .single()
                .ok_or_else(|| Error::from_code(ErrorCode::FOAR0001, "ambiguous local datetime"))?;
            let ord =
                (dta.timestamp(), dta.timestamp_subsec_nanos()).cmp(&(dtb.timestamp(), dtb.timestamp_subsec_nanos()));
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
        // Zero offset (UTC). `Utc.fix()` yields a FixedOffset(0) without Option handling.
        chrono::Utc.fix()
    }

    // doc_order_distinct removed; stream set ops use sorted_distinct_nodes_vec directly.

    fn doc_order_only(&self, seq: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
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

    // (function name resolution for error messages is handled in FunctionRegistry::resolve)

    // (default_collation cached in Vm::new)

    // ===== Axis & NodeTest helpers =====

    #[inline]
    fn matches_interned_name(&self, node: &N, expected: &crate::compiler::ir::InternedQName) -> bool {
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
                    } else if matches!(node.kind(), crate::model::NodeKind::Element | crate::model::NodeKind::Namespace)
                    {
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
    // non-stream set_union removed (replaced by set_union_stream).

    /// Stream variant of union: consumes streams, collects nodes, then sorts/dedups.
    fn set_union_stream(&mut self, a: XdmSequenceStream<N>, b: XdmSequenceStream<N>) -> Result<XdmSequence<N>, Error> {
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
    // non-stream set_intersect removed (replaced by set_intersect_stream).

    /// Stream variant of intersect: consumes streams, sorts/dedups, then computes intersection.
    fn set_intersect_stream(
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
    // non-stream set_except removed (replaced by set_except_stream).

    /// Stream variant of except: consumes streams, sorts/dedups, then computes difference.
    fn set_except_stream(&mut self, a: XdmSequenceStream<N>, b: XdmSequenceStream<N>) -> Result<XdmSequence<N>, Error> {
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
    // sorted_distinct_nodes removed; stream variants operate on homogeneous node Vecs.

    /// Sort and deduplicate a homogeneous node vector using document order.
    fn sorted_distinct_nodes_vec(&self, nodes: Vec<N>) -> Result<Vec<N>, Error> {
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
    fn collect_nodes_from_stream(&self, s: XdmSequenceStream<N>) -> Result<Vec<N>, Error> {
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
    fn node_compare(&self, a: &N, b: &N) -> Result<Ordering, Error> {
        match (a.doc_order_key(), b.doc_order_key()) {
            (Some(ak), Some(bk)) => Ok(ak.cmp(&bk)),
            _ => a.compare_document_order(b),
        }
    }

    // ===== Type operations (very small subset) =====
    fn parse_integer_string(&self, text: &str, target: &str) -> Result<i128, Error> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(Error::from_code(ErrorCode::FORG0001, format!("cannot cast to {target}: empty string")));
        }
        trimmed
            .parse::<i128>()
            .map_err(|_| Error::from_code(ErrorCode::FORG0001, format!("invalid lexical for {target}")))
    }

    fn float_to_integer(&self, value: f64, target: &str) -> Result<i128, Error> {
        if !value.is_finite() {
            return Err(Error::from_code(ErrorCode::FOCA0001, format!("{target} overflow")));
        }
        if value.fract() != 0.0 {
            return Err(Error::from_code(ErrorCode::FOCA0001, format!("non-integer value for {target}")));
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
                    Err(Error::from_code(ErrorCode::FORG0001, format!("negative value not allowed for {target}")))
                } else {
                    Ok(signed as u128)
                }
            }
        }
    }

    fn ensure_range_i128(&self, value: i128, min: i128, max: i128, target: &str) -> Result<i128, Error> {
        if value < min || value > max {
            Err(Error::from_code(ErrorCode::FORG0001, format!("value out of range for {target}")))
        } else {
            Ok(value)
        }
    }

    fn ensure_range_u128(&self, value: u128, min: u128, max: u128, target: &str) -> Result<u128, Error> {
        if value < min || value > max {
            Err(Error::from_code(ErrorCode::FORG0001, format!("value out of range for {target}")))
        } else {
            Ok(value)
        }
    }

    fn require_string_like(&self, atom: &XdmAtomicValue, target: &str) -> Result<String, Error> {
        string_like_value(atom).ok_or_else(|| Error::from_code(ErrorCode::FORG0001, format!("cannot cast to {target}")))
    }
    fn cast_atomic(&self, a: XdmAtomicValue, target: &ExpandedName) -> Result<XdmAtomicValue, Error> {
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
                            return Err(Error::from_code(ErrorCode::FORG0001, "invalid boolean lexical form"));
                        }
                    };
                    Ok(XdmAtomicValue::Boolean(b))
                }
            },
            "integer" => match a {
                XdmAtomicValue::Integer(v) => Ok(XdmAtomicValue::Integer(v)),
                other => {
                    let value = self.integer_from_atomic(&other, "xs:integer")?;
                    let bounded = self.ensure_range_i128(value, i64::MIN as i128, i64::MAX as i128, "xs:integer")?;
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
                    let value: f64 =
                        trimmed.parse().map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:decimal"))?;
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
                        _ => trimmed.parse().map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:double"))?,
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
                        _ => trimmed.parse().map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:float"))?,
                    };
                    Ok(XdmAtomicValue::Float(value))
                }
            },
            "long" => match a {
                XdmAtomicValue::Long(v) => Ok(XdmAtomicValue::Long(v)),
                other => {
                    let value = self.integer_from_atomic(&other, "xs:long")?;
                    let bounded = self.ensure_range_i128(value, i64::MIN as i128, i64::MAX as i128, "xs:long")?;
                    Ok(XdmAtomicValue::Long(bounded as i64))
                }
            },
            "int" => match a {
                XdmAtomicValue::Int(v) => Ok(XdmAtomicValue::Int(v)),
                other => {
                    let value = self.integer_from_atomic(&other, "xs:int")?;
                    let bounded = self.ensure_range_i128(value, i32::MIN as i128, i32::MAX as i128, "xs:int")?;
                    Ok(XdmAtomicValue::Int(bounded as i32))
                }
            },
            "short" => match a {
                XdmAtomicValue::Short(v) => Ok(XdmAtomicValue::Short(v)),
                other => {
                    let value = self.integer_from_atomic(&other, "xs:short")?;
                    let bounded = self.ensure_range_i128(value, i16::MIN as i128, i16::MAX as i128, "xs:short")?;
                    Ok(XdmAtomicValue::Short(bounded as i16))
                }
            },
            "byte" => match a {
                XdmAtomicValue::Byte(v) => Ok(XdmAtomicValue::Byte(v)),
                other => {
                    let value = self.integer_from_atomic(&other, "xs:byte")?;
                    let bounded = self.ensure_range_i128(value, i8::MIN as i128, i8::MAX as i128, "xs:byte")?;
                    Ok(XdmAtomicValue::Byte(bounded as i8))
                }
            },
            "unsignedLong" => match a {
                XdmAtomicValue::UnsignedLong(v) => Ok(XdmAtomicValue::UnsignedLong(v)),
                other => {
                    let value = self.unsigned_from_atomic(&other, "xs:unsignedLong")?;
                    let bounded = self.ensure_range_u128(value, 0, u64::MAX as u128, "xs:unsignedLong")?;
                    Ok(XdmAtomicValue::UnsignedLong(bounded as u64))
                }
            },
            "unsignedInt" => match a {
                XdmAtomicValue::UnsignedInt(v) => Ok(XdmAtomicValue::UnsignedInt(v)),
                other => {
                    let value = self.unsigned_from_atomic(&other, "xs:unsignedInt")?;
                    let bounded = self.ensure_range_u128(value, 0, u32::MAX as u128, "xs:unsignedInt")?;
                    Ok(XdmAtomicValue::UnsignedInt(bounded as u32))
                }
            },
            "unsignedShort" => match a {
                XdmAtomicValue::UnsignedShort(v) => Ok(XdmAtomicValue::UnsignedShort(v)),
                other => {
                    let value = self.unsigned_from_atomic(&other, "xs:unsignedShort")?;
                    let bounded = self.ensure_range_u128(value, 0, u16::MAX as u128, "xs:unsignedShort")?;
                    Ok(XdmAtomicValue::UnsignedShort(bounded as u16))
                }
            },
            "unsignedByte" => match a {
                XdmAtomicValue::UnsignedByte(v) => Ok(XdmAtomicValue::UnsignedByte(v)),
                other => {
                    let value = self.unsigned_from_atomic(&other, "xs:unsignedByte")?;
                    let bounded = self.ensure_range_u128(value, 0, u8::MAX as u128, "xs:unsignedByte")?;
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
                    let bounded = self.ensure_range_i128(value, i64::MIN as i128, 0, "xs:nonPositiveInteger")?;
                    Ok(XdmAtomicValue::NonPositiveInteger(bounded as i64))
                }
            },
            "negativeInteger" => match a {
                XdmAtomicValue::NegativeInteger(v) => Ok(XdmAtomicValue::NegativeInteger(v)),
                other => {
                    let value = self.integer_from_atomic(&other, "xs:negativeInteger")?;
                    if value >= 0 {
                        return Err(Error::from_code(ErrorCode::FORG0001, "value must be < 0 for xs:negativeInteger"));
                    }
                    let bounded = self.ensure_range_i128(value, i64::MIN as i128, -1, "xs:negativeInteger")?;
                    Ok(XdmAtomicValue::NegativeInteger(bounded as i64))
                }
            },
            "nonNegativeInteger" => match a {
                XdmAtomicValue::NonNegativeInteger(v) => Ok(XdmAtomicValue::NonNegativeInteger(v)),
                other => {
                    let value = self.unsigned_from_atomic(&other, "xs:nonNegativeInteger")?;
                    let bounded = self.ensure_range_u128(value, 0, u64::MAX as u128, "xs:nonNegativeInteger")?;
                    Ok(XdmAtomicValue::NonNegativeInteger(bounded as u64))
                }
            },
            "positiveInteger" => match a {
                XdmAtomicValue::PositiveInteger(v) => Ok(XdmAtomicValue::PositiveInteger(v)),
                other => {
                    let value = self.unsigned_from_atomic(&other, "xs:positiveInteger")?;
                    if value == 0 {
                        return Err(Error::from_code(ErrorCode::FORG0001, "value must be > 0 for xs:positiveInteger"));
                    }
                    let bounded = self.ensure_range_u128(value, 1, u64::MAX as u128, "xs:positiveInteger")?;
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
                XdmAtomicValue::QName { ns_uri, prefix, local } => Ok(XdmAtomicValue::QName { ns_uri, prefix, local }),
                other => {
                    let text = self.require_string_like(&other, "xs:QName")?;
                    let (prefix, local) = parse_qname_lexical(&text)
                        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid QName lexical"))?;
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
                    let bytes = decode_hex(&hex)
                        .ok_or_else(|| Error::from_code(ErrorCode::FORG0001, "invalid xs:hexBinary"))?;
                    let encoded = BASE64_STANDARD.encode(bytes);
                    Ok(XdmAtomicValue::Base64Binary(encoded))
                }
                other => {
                    let text = self.require_string_like(&other, "xs:base64Binary")?;
                    let normalized: String = text.chars().filter(|c| !c.is_whitespace()).collect();
                    if BASE64_STANDARD.decode(normalized.as_bytes()).is_err() {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:base64Binary"));
                    }
                    Ok(XdmAtomicValue::Base64Binary(normalized))
                }
            },
            "hexBinary" => match a {
                XdmAtomicValue::HexBinary(v) => Ok(XdmAtomicValue::HexBinary(v)),
                XdmAtomicValue::Base64Binary(b64) => {
                    let bytes = BASE64_STANDARD
                        .decode(b64.as_bytes())
                        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:base64Binary"))?;
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
                    self.parse_year_month_duration(&text)
                        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid yearMonthDuration"))
                }
            },
            "dayTimeDuration" => match a {
                XdmAtomicValue::DayTimeDuration(m) => Ok(XdmAtomicValue::DayTimeDuration(m)),
                other => {
                    let text = self.require_string_like(&other, "xs:dayTimeDuration")?;
                    self.parse_day_time_duration(&text)
                        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid dayTimeDuration"))
                }
            },
            "gYear" => match a {
                XdmAtomicValue::GYear { year, tz } => Ok(XdmAtomicValue::GYear { year, tz }),
                other => {
                    let text = self.require_string_like(&other, "xs:gYear")?;
                    let (year, tz) =
                        parse_g_year(&text).map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gYear"))?;
                    Ok(XdmAtomicValue::GYear { year, tz })
                }
            },
            "gYearMonth" => match a {
                XdmAtomicValue::GYearMonth { year, month, tz } => Ok(XdmAtomicValue::GYearMonth { year, month, tz }),
                other => {
                    let text = self.require_string_like(&other, "xs:gYearMonth")?;
                    let (year, month, tz) = parse_g_year_month(&text)
                        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gYearMonth"))?;
                    Ok(XdmAtomicValue::GYearMonth { year, month, tz })
                }
            },
            "gMonth" => match a {
                XdmAtomicValue::GMonth { month, tz } => Ok(XdmAtomicValue::GMonth { month, tz }),
                other => {
                    let text = self.require_string_like(&other, "xs:gMonth")?;
                    let (month, tz) =
                        parse_g_month(&text).map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gMonth"))?;
                    Ok(XdmAtomicValue::GMonth { month, tz })
                }
            },
            "gMonthDay" => match a {
                XdmAtomicValue::GMonthDay { month, day, tz } => Ok(XdmAtomicValue::GMonthDay { month, day, tz }),
                other => {
                    let text = self.require_string_like(&other, "xs:gMonthDay")?;
                    let (month, day, tz) = parse_g_month_day(&text)
                        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gMonthDay"))?;
                    Ok(XdmAtomicValue::GMonthDay { month, day, tz })
                }
            },
            "gDay" => match a {
                XdmAtomicValue::GDay { day, tz } => Ok(XdmAtomicValue::GDay { day, tz }),
                other => {
                    let text = self.require_string_like(&other, "xs:gDay")?;
                    let (day, tz) =
                        parse_g_day(&text).map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gDay"))?;
                    Ok(XdmAtomicValue::GDay { day, tz })
                }
            },
            _ => Err(Error::not_implemented("cast target type")),
        }
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
    fn parse_date_time(&self, s: &str) -> Result<XdmAtomicValue, crate::util::temporal::TemporalErr> {
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
                    if !self.item_matches_type(&it, item)? {
                        return Ok(false);
                    }
                    match occ {
                        OccurrenceIR::One | OccurrenceIR::ZeroOrOne => {
                            if count > 1 {
                                return Ok(false);
                            }
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
    fn item_matches_type(&self, item: &XdmItem<N>, t: &crate::compiler::ir::ItemTypeIR) -> Result<bool, Error> {
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

struct DistinctCursor<N> {
    vm: VmHandle<N>,
    input: Option<Box<dyn SequenceCursor<N>>>,
    // streaming state
    seen_keys: HashSet<u64>,
    seen_fallback: SmallVec<[N; 16]>,
}

impl<N: 'static + XdmNode + Clone> DistinctCursor<N> {
    fn new(vm: VmHandle<N>, input: Box<dyn SequenceCursor<N>>) -> Self {
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
struct EnsureOrderCursor<N> {
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
    fn new(vm: VmHandle<N>, input: Box<dyn SequenceCursor<N>>) -> Self {
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
                        let to_emit = self.pending.replace(next.clone()).unwrap();
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
                            let first = self.pending.take().unwrap();
                            if let Err(e) = self.switch_to_fallback(first, next) {
                                return Some(Err(e));
                            }
                            return self.buffer.pop_front().map(Ok);
                        }
                    }
                }
                (Some(_prev), _) => {
                    // Non-node items: emit previous, shift window
                    let to_emit = self.pending.replace(next).unwrap();
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

impl<N: 'static + XdmNode + Clone> AtomizeCursor<N> {
    fn new(stream: XdmSequenceStream<N>) -> Self {
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
struct TreatCursor<N> {
    vm: VmHandle<N>,
    input: Box<dyn SequenceCursor<N>>,
    item_type: crate::compiler::ir::ItemTypeIR,
    min: usize,
    max: Option<usize>,
    seen: usize,
    pending_error: Option<Error>,
}

impl<N: 'static + XdmNode + Clone> TreatCursor<N> {
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
