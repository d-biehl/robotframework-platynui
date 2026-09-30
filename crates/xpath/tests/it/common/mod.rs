//! Shared helpers for the xpath integration tests (`use crate::common::...`).

#![allow(dead_code)] // not every integration test binary uses every helper

use std::iter::Peekable;
use std::str::Chars;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use platynui_xpath::engine::runtime::StaticContextBuilder;
use platynui_xpath::simple_node::{SimpleNodeBuilder, attr, doc, elem};
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use platynui_xpath::{
    DynamicContext, DynamicContextBuilder, ExpandedName, NodeKind, QName, SimpleNode, XdmNode, compile_with_context,
    evaluate, evaluate_expr, evaluate_first_expr,
};

/// Evaluates `xpath` against `ctx` and returns the single integer it produces.
///
/// Panics with the offending xpath string if compilation fails, evaluation
/// fails, or the sequence is not exactly one integer atomic.
pub fn eval_single_integer(xpath: &str, ctx: &DynamicContext<SimpleNode>) -> i64 {
    let seq = evaluate_expr::<SimpleNode>(xpath, ctx).unwrap_or_else(|e| panic!("evaluate `{xpath}`: {e:?}"));
    match &seq[..] {
        [XdmItem::Atomic(XdmAtomicValue::Integer(n))] => *n,
        other => panic!("expected single integer for `{xpath}`, got: {other:?}"),
    }
}

/// Evaluates `xpath` against `ctx` and returns the single boolean it produces.
pub fn eval_single_boolean(xpath: &str, ctx: &DynamicContext<SimpleNode>) -> bool {
    let seq = evaluate_expr::<SimpleNode>(xpath, ctx).unwrap_or_else(|e| panic!("evaluate `{xpath}`: {e:?}"));
    match &seq[..] {
        [XdmItem::Atomic(XdmAtomicValue::Boolean(b))] => *b,
        other => panic!("expected single boolean for `{xpath}`, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// Trees of the document-order specs
// ---------------------------------------------------------------------------------------------

/// Builds a document from the bracket notation of the specs, such as `W:[P1:[B1,B2],B3,P2:[B4,B5]]`.
///
/// Every label is an element whose `id` attribute is the label. The element's name is the label
/// without its trailing digits and underscores, so `B0_3` is a `B` and `r` is an `r`.
pub fn tree(notation: &str) -> SimpleNode {
    let mut chars = notation.chars().peekable();
    let root = parse_element(&mut chars, notation);
    assert!(chars.next().is_none(), "trailing input in `{notation}`");
    doc().child(root).build()
}

fn parse_element(chars: &mut Peekable<Chars<'_>>, notation: &str) -> SimpleNodeBuilder {
    let mut label = String::new();
    while let Some(&c) = chars.peek() {
        if !(c.is_ascii_alphanumeric() || c == '_') {
            break;
        }
        label.push(c);
        chars.next();
    }
    let name = label.trim_end_matches(|c: char| c.is_ascii_digit() || c == '_');
    assert!(!name.is_empty(), "expected an element label in `{notation}`");
    let mut element = elem(name).attr(attr("id", &label));
    if chars.peek() == Some(&':') {
        chars.next();
        assert_eq!(chars.next(), Some('['), "expected `[` after `{label}:` in `{notation}`");
        loop {
            element = element.child(parse_element(chars, notation));
            match chars.next() {
                Some(',') => {}
                Some(']') => break,
                other => panic!("expected `,` or `]` in `{notation}`, got {other:?}"),
            }
        }
    }
    element
}

/// The wide tree of the first-match bounds: `W` with 50 `P` children (`P0`…`P49`) of 20 `B`
/// children each (`B0_0`…`B49_19`), 1,052 nodes with the document.
pub fn wide_tree() -> SimpleNode {
    let mut window = elem("W").attr(attr("id", "W"));
    for p in 0..50 {
        let mut panel = elem("P").attr(attr("id", &format!("P{p}")));
        for b in 0..20 {
            panel = panel.child(elem("B").attr(attr("id", &format!("B{p}_{b}"))));
        }
        window = window.child(panel);
    }
    doc().child(window).build()
}

/// Two separate documents, `d1:[x1]` and `d2:[x2]`, each with one `x`.
pub fn two_documents() -> (SimpleNode, SimpleNode) {
    (tree("d1:[x1]"), tree("d2:[x2]"))
}

/// The keyless tree `r:[a,b,c]` in which `b` is missing from the list of `r`, as a view of its
/// document and a view of `b`.
pub fn keyless_tree_missing_b() -> (Keyless, Keyless) {
    let document = tree("r:[a,b,c]");
    let model = Keyless::hiding(&document, "b");
    let b = model.view(&find(&document, "b"));
    (model, b)
}

/// The labels of a tree's document and elements in document order.
pub fn preorder_labels<N: XdmNode>(root: &N) -> Vec<String> {
    fn walk<N: XdmNode>(node: &N, out: &mut Vec<String>) {
        out.push(label(node));
        for child in node.children() {
            if matches!(child.kind(), NodeKind::Element) {
                walk(&child, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, &mut out);
    out
}

/// How a test names a node: an element by its `id` (by its name without one), the document as
/// `#doc`, an attribute as `<owner>@<name>`, and any other node by its string value.
pub fn label<N: XdmNode>(node: &N) -> String {
    match node.kind() {
        NodeKind::Document => "#doc".to_string(),
        NodeKind::Element => node
            .attributes()
            .find(|a| a.name().is_some_and(|q| q.local == "id"))
            .map_or_else(|| node.name().map(|q| q.local).unwrap_or_default(), |a| a.string_value()),
        NodeKind::Attribute => {
            let owner = node.parent().map(|p| label(&p)).unwrap_or_default();
            format!("{owner}@{}", node.name().map(|q| q.local).unwrap_or_default())
        }
        _ => node.string_value(),
    }
}

/// The labels of a result: nodes by [`label`], atomic values by their string value.
pub fn labels<N: XdmNode>(items: &[XdmItem<N>]) -> Vec<String> {
    items
        .iter()
        .map(|item| match item {
            XdmItem::Node(n) => label(n),
            XdmItem::Atomic(a) => atomic_text(a),
        })
        .collect()
}

fn atomic_text(value: &XdmAtomicValue) -> String {
    match value {
        XdmAtomicValue::Integer(i) => i.to_string(),
        XdmAtomicValue::Boolean(b) => b.to_string(),
        XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => s.clone(),
        other => format!("{other:?}"),
    }
}

fn context<N: 'static + XdmNode>(node: &N) -> DynamicContext<N> {
    DynamicContextBuilder::default().with_context_item(XdmItem::Node(node.clone())).build()
}

/// Evaluates `xpath` with `node` as the context item and returns the labels of the result.
pub fn eval_labels<N: 'static + XdmNode>(xpath: &str, node: &N) -> Vec<String> {
    let items = evaluate_expr::<N>(xpath, &context(node)).unwrap_or_else(|e| panic!("evaluate `{xpath}`: {e:?}"));
    labels(&items)
}

/// Evaluates `xpath` with `node` as the context item and returns the label of its first item,
/// taken from the stream as `evaluate_single` does.
pub fn first_label<N: 'static + XdmNode>(xpath: &str, node: &N) -> Option<String> {
    let first = evaluate_first_expr::<N>(xpath, &context(node)).unwrap_or_else(|e| panic!("evaluate `{xpath}`: {e:?}"));
    first.map(|item| labels(&[item]).remove(0))
}

/// Evaluates `xpath` with `node` as the context item and `$other` bound to `other`.
pub fn eval_labels_with_other<N: 'static + XdmNode>(xpath: &str, node: &N, other: &N) -> Vec<String> {
    let name = ExpandedName { ns_uri: None, local: "other".to_string() };
    let static_ctx = StaticContextBuilder::new().with_variable(name.clone()).build();
    let compiled = compile_with_context(xpath, &static_ctx).unwrap_or_else(|e| panic!("compile `{xpath}`: {e:?}"));
    let ctx = DynamicContextBuilder::default()
        .with_context_item(XdmItem::Node(node.clone()))
        .with_variable(name, vec![XdmItem::Node(other.clone())])
        .build();
    let items = evaluate::<N>(&compiled, &ctx).unwrap_or_else(|e| panic!("evaluate `{xpath}`: {e:?}"));
    labels(&items)
}

/// The first node of `root`'s subtree, in document order, whose label is `wanted`.
pub fn find<N: XdmNode>(root: &N, wanted: &str) -> N {
    fn walk<N: XdmNode>(node: &N, wanted: &str) -> Option<N> {
        if label(node) == wanted {
            return Some(node.clone());
        }
        node.children().find_map(|child| walk(&child, wanted))
    }
    walk(root, wanted).unwrap_or_else(|| panic!("no node labelled `{wanted}`"))
}

// ---------------------------------------------------------------------------------------------
// A model without document-order keys
// ---------------------------------------------------------------------------------------------

/// A [`SimpleNode`] tree seen through a model without document-order keys, as every real provider
/// is: it orders by ancestry. It can also record whose list of children was read, and leave one
/// node out of its parent's list.
#[derive(Clone)]
pub struct Keyless {
    node: SimpleNode,
    model: Arc<KeylessModel>,
}

#[derive(Default)]
struct KeylessModel {
    reads: Option<Mutex<Vec<String>>>,
    hidden: Option<String>,
    /// A cancellation flag to set once this many lists of children have been read.
    cancel: Option<(Arc<AtomicBool>, usize)>,
    lists_read: AtomicUsize,
}

impl Keyless {
    /// The keyless view of `node`'s tree.
    pub fn new(node: &SimpleNode) -> Self {
        Self { node: node.clone(), model: Arc::new(KeylessModel::default()) }
    }

    /// The keyless view of `node`'s tree that records every read of a list of children.
    pub fn recording(node: &SimpleNode) -> Self {
        Self {
            node: node.clone(),
            model: Arc::new(KeylessModel { reads: Some(Mutex::new(Vec::new())), ..KeylessModel::default() }),
        }
    }

    /// The keyless view of `node`'s tree in which the node labelled `hidden` is missing from its
    /// parent's list of children. It still reaches its parent.
    pub fn hiding(node: &SimpleNode, hidden: &str) -> Self {
        Self {
            node: node.clone(),
            model: Arc::new(KeylessModel { hidden: Some(hidden.to_string()), ..KeylessModel::default() }),
        }
    }

    /// The keyless view of `node`'s tree that sets `flag` once it has read `after` lists of
    /// children, to cancel an evaluation while it runs.
    pub fn cancelling(node: &SimpleNode, flag: &Arc<AtomicBool>, after: usize) -> Self {
        Self {
            node: node.clone(),
            model: Arc::new(KeylessModel { cancel: Some((Arc::clone(flag), after)), ..KeylessModel::default() }),
        }
    }

    /// The same model's view of another node of the tree.
    pub fn view(&self, node: &SimpleNode) -> Self {
        Self { node: node.clone(), model: Arc::clone(&self.model) }
    }

    /// The labels of the nodes whose children were read, once each, in the order of their first read.
    pub fn distinct_reads(&self) -> Vec<String> {
        let reads = self.model.reads.as_ref().expect("a recording model");
        let reads = reads.lock().unwrap_or_else(PoisonError::into_inner);
        let mut distinct: Vec<String> = Vec::new();
        for read in reads.iter() {
            if !distinct.contains(read) {
                distinct.push(read.clone());
            }
        }
        distinct
    }

    /// Forgets the reads recorded so far.
    pub fn clear_reads(&self) {
        if let Some(reads) = &self.model.reads {
            reads.lock().unwrap_or_else(PoisonError::into_inner).clear();
        }
    }

    /// The node this view wraps.
    pub fn inner(&self) -> &SimpleNode {
        &self.node
    }
}

impl PartialEq for Keyless {
    fn eq(&self, other: &Self) -> bool {
        self.node == other.node
    }
}

impl Eq for Keyless {}

impl std::fmt::Debug for Keyless {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Keyless({})", label(&self.node))
    }
}

impl XdmNode for Keyless {
    type Children<'a> = std::vec::IntoIter<Keyless>;
    type Attributes<'a> = std::vec::IntoIter<Keyless>;
    type Namespaces<'a> = std::vec::IntoIter<Keyless>;

    fn kind(&self) -> NodeKind {
        self.node.kind()
    }

    fn name(&self) -> Option<QName> {
        self.node.name()
    }

    fn typed_value(&self) -> Vec<XdmAtomicValue> {
        self.node.typed_value()
    }

    fn parent(&self) -> Option<Self> {
        self.node.parent().map(|parent| self.view(&parent))
    }

    fn children(&self) -> Self::Children<'_> {
        if let Some(reads) = &self.model.reads {
            reads.lock().unwrap_or_else(PoisonError::into_inner).push(label(&self.node));
        }
        if let Some((flag, after)) = &self.model.cancel
            && self.model.lists_read.fetch_add(1, Ordering::SeqCst) + 1 >= *after
        {
            flag.store(true, Ordering::SeqCst);
        }
        self.node
            .children()
            .filter(|child| self.model.hidden.as_deref() != Some(label(child).as_str()))
            .map(|child| self.view(&child))
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn attributes(&self) -> Self::Attributes<'_> {
        self.node.attributes().map(|a| self.view(&a)).collect::<Vec<_>>().into_iter()
    }

    fn namespaces(&self) -> Self::Namespaces<'_> {
        self.node.namespaces().map(|n| self.view(&n)).collect::<Vec<_>>().into_iter()
    }

    fn identity_hint(&self) -> Option<u64> {
        self.node.identity_hint()
    }
}
