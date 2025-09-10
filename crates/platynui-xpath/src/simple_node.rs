//! Simple in-memory tree implementation for `XdmNode` used in tests and quick prototypes.
//!
//! Focus:
//! - Ergonomic builder for quick test tree creation
//! - Stable `compare_document_order` (uses ancestry + sibling ordering)
//! - Thread-safe (Arc + RwLock) for parallel evaluator tests
//!
//! Example:
//! ```
//! use platynui_xpath::simple_node::{SimpleNode, elem, text, attr};
//! use platynui_xpath::XdmNode;
//!
//! // <root id="r"><child>Hello</child><child world="yes"/></root>
//! let root = elem("root")
//!     .attr(attr("id", "r"))
//!     .child(
//!         elem("child")
//!             .child(text("Hello"))
//!     )
//!     .child(
//!         elem("child").attr(attr("world", "yes"))
//!     )
//!     .build();
//!
//! assert_eq!(root.name().unwrap().local, "root");
//! assert_eq!(root.children().len(), 2); // two child elements
//! ```
//!
//! Document root & namespaces example:
//! ```
//! use platynui_xpath::simple_node::{doc, elem, text, attr, ns};
//! use platynui_xpath::XdmNode; // for children()/string_value()
//! let document = doc()
//!   .child(
//!     elem("root")
//!       .namespace(ns("p","urn:one"))
//!       .child(elem("child").child(text("Hi")))
//!   )
//!   .build();
//! let root = document.children()[0].clone();
//! assert_eq!(root.lookup_namespace_uri("p").as_deref(), Some("urn:one"));
//! assert_eq!(root.string_value(), "Hi");
//! ```
//!
//! Document order (attributes < children):
//! ```
//! use platynui_xpath::simple_node::{elem, attr};
//! use platynui_xpath::XdmNode; // trait import for compare_document_order
//! let r = elem("r")
//!   .attr(attr("a","1"))
//!   .child(elem("c"))
//!   .build();
//! let attr_node = r.attributes()[0].clone();
//! let child_node = r.children()[0].clone();
//! assert_eq!(attr_node.compare_document_order(&child_node).unwrap(), core::cmp::Ordering::Less);
//! ```
use std::fmt;
use std::sync::{Arc, Weak, RwLock};

use crate::model::{NodeKind, QName, XdmNode};

#[derive(Debug)]
pub(crate) struct Inner {
    kind: NodeKind,
    name: Option<QName>,
    value: RwLock<Option<String>>, // text / attribute / PI content
    parent: RwLock<Option<Weak<Inner>>>,
    attributes: RwLock<Vec<SimpleNode>>, // attribute nodes (NodeKind::Attribute)
    namespaces: RwLock<Vec<SimpleNode>>, // namespace nodes
    children: RwLock<Vec<SimpleNode>>,
    cached_text: RwLock<Option<String>>, // memoized string value for element/document
}

/// A simple Arc-backed node implementation.
#[derive(Clone)]
pub struct SimpleNode(pub(crate) Arc<Inner>);

impl PartialEq for SimpleNode { fn eq(&self, other: &Self) -> bool { Arc::ptr_eq(&self.0, &other.0) } }
impl Eq for SimpleNode {}
impl std::hash::Hash for SimpleNode { fn hash<H: std::hash::Hasher>(&self, state: &mut H) { (Arc::as_ptr(&self.0) as *const Inner).hash(state) } }

impl fmt::Debug for SimpleNode { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.debug_struct("SimpleNode")
        .field("kind", &self.0.kind)
        .field("name", &self.0.name)
        .field("value", &self.0.value)
        .finish() } }

impl SimpleNode {
    fn new(kind: NodeKind, name: Option<QName>, value: Option<String>) -> Self {
        SimpleNode(Arc::new(Inner {
            kind,
            name,
            value: RwLock::new(value),
            parent: RwLock::new(None),
            attributes: RwLock::new(Vec::new()),
            namespaces: RwLock::new(Vec::new()),
            children: RwLock::new(Vec::new()),
            cached_text: RwLock::new(None),
        }))
    }

    pub fn document() -> SimpleNodeBuilder { SimpleNodeBuilder::new(NodeKind::Document, None, None) }
    pub fn element(name: &str) -> SimpleNodeBuilder { SimpleNodeBuilder::new(NodeKind::Element, Some(QName { prefix: None, local: name.to_string(), ns_uri: None }), None) }
    pub fn attribute(name: &str, value: &str) -> SimpleNode { SimpleNode::new(NodeKind::Attribute, Some(QName { prefix: None, local: name.to_string(), ns_uri: None }), Some(value.to_string())) }
    pub fn text(value: &str) -> SimpleNode { SimpleNode::new(NodeKind::Text, None, Some(value.to_string())) }
    pub fn comment(value: &str) -> SimpleNode { SimpleNode::new(NodeKind::Comment, None, Some(value.to_string())) }
    pub fn pi(target: &str, data: &str) -> SimpleNode { SimpleNode::new(NodeKind::ProcessingInstruction, Some(QName { prefix: None, local: target.to_string(), ns_uri: None }), Some(data.to_string())) }
    pub fn namespace(prefix: &str, uri: &str) -> SimpleNode { SimpleNode::new(NodeKind::Namespace, Some(QName { prefix: Some(prefix.to_string()), local: prefix.to_string(), ns_uri: Some(uri.to_string()) }), Some(uri.to_string())) }

    /// Resolve namespace prefix by walking ancestor chain (including self)
    pub fn lookup_namespace_uri(&self, prefix: &str) -> Option<String> {
        let mut cur: Option<SimpleNode> = Some(self.clone());
        while let Some(n) = cur {
            for ns in n.namespaces() {
                if let Some(name) = ns.name() { if name.prefix.as_deref()==Some(prefix) { return ns.string_value().into(); } }
            }
            cur = n.parent();
        }
        None
    }
}

pub struct SimpleNodeBuilder {
    node: SimpleNode,
    pending_children: Vec<SimpleNode>,
    pending_attrs: Vec<SimpleNode>,
    pending_ns: Vec<SimpleNode>,
}

impl SimpleNodeBuilder {
    fn new(kind: NodeKind, name: Option<QName>, value: Option<String>) -> Self { Self { node: SimpleNode::new(kind, name, value), pending_children: Vec::new(), pending_attrs: Vec::new(), pending_ns: Vec::new() } }

    pub fn child(mut self, child_builder: impl Into<SimpleNodeOrBuilder>) -> Self {
        match child_builder.into() { SimpleNodeOrBuilder::Built(n) => self.pending_children.push(n), SimpleNodeOrBuilder::Builder(b) => self.pending_children.push(b.build()) }
        self
    }
    pub fn children<I: IntoIterator<Item=SimpleNodeOrBuilder>>(mut self, it: I) -> Self {
        for c in it { match c { SimpleNodeOrBuilder::Built(n) => self.pending_children.push(n), SimpleNodeOrBuilder::Builder(b) => self.pending_children.push(b.build()) } }
        self
    }
    pub fn attr(mut self, attr: SimpleNode) -> Self { debug_assert!(attr.kind()==NodeKind::Attribute); self.pending_attrs.push(attr); self }
    pub fn attrs<I: IntoIterator<Item=SimpleNode>>(mut self, attrs: I) -> Self { for a in attrs { debug_assert!(a.kind()==NodeKind::Attribute); self.pending_attrs.push(a); } self }
    pub fn namespace(mut self, ns: SimpleNode) -> Self { debug_assert!(ns.kind()==NodeKind::Namespace); self.pending_ns.push(ns); self }
    pub fn namespaces<I: IntoIterator<Item=SimpleNode>>(mut self, it: I) -> Self { for n in it { debug_assert!(n.kind()==NodeKind::Namespace); self.pending_ns.push(n); } self }
    pub fn value(self, v: &str) -> Self { if matches!(self.node.kind(), NodeKind::Text|NodeKind::Comment|NodeKind::ProcessingInstruction|NodeKind::Attribute) { *self.node.0.value.write().unwrap() = Some(v.to_string()); } self }
    pub fn build(self) -> SimpleNode { // finalize relationships
        {
            let mut attrs = self.node.0.attributes.write().unwrap();
            for a in &self.pending_attrs { *a.0.parent.write().unwrap() = Some(Arc::downgrade(&self.node.0)); }
            attrs.extend(self.pending_attrs);
        }
        {
            let mut nss = self.node.0.namespaces.write().unwrap();
            for n in &self.pending_ns { *n.0.parent.write().unwrap() = Some(Arc::downgrade(&self.node.0)); }
            nss.extend(self.pending_ns);
        }
        {
            let mut ch = self.node.0.children.write().unwrap();
            for c in &self.pending_children { *c.0.parent.write().unwrap() = Some(Arc::downgrade(&self.node.0)); }
            ch.extend(self.pending_children);
        }
        // Precompute cached text for element/document
        if matches!(self.node.kind(), NodeKind::Element | NodeKind::Document) {
            let _ = self.node.string_value();
        }
        self.node
    }
}

pub enum SimpleNodeOrBuilder { Built(SimpleNode), Builder(SimpleNodeBuilder) }
impl From<SimpleNode> for SimpleNodeOrBuilder { fn from(n: SimpleNode) -> Self { SimpleNodeOrBuilder::Built(n) } }
impl From<SimpleNodeBuilder> for SimpleNodeOrBuilder { fn from(b: SimpleNodeBuilder) -> Self { SimpleNodeOrBuilder::Builder(b) } }

// Convenience helper functions for concise test code
pub fn elem(name: &str) -> SimpleNodeBuilder { SimpleNode::element(name) }
pub fn text(v: &str) -> SimpleNode { SimpleNode::text(v) }
pub fn attr(name: &str, v: &str) -> SimpleNode { SimpleNode::attribute(name, v) }
pub fn comment(v: &str) -> SimpleNode { SimpleNode::comment(v) }
pub fn ns(prefix: &str, uri: &str) -> SimpleNode { SimpleNode::namespace(prefix, uri) }
pub fn doc() -> SimpleNodeBuilder { SimpleNode::document() }

impl XdmNode for SimpleNode {
    fn kind(&self) -> NodeKind { self.0.kind.clone() }
    fn name(&self) -> Option<QName> { self.0.name.clone() }
    fn string_value(&self) -> String {
        match self.kind() {
            NodeKind::Text | NodeKind::Attribute | NodeKind::Comment | NodeKind::ProcessingInstruction | NodeKind::Namespace => self.0.value.read().unwrap().clone().unwrap_or_default(),
            NodeKind::Element | NodeKind::Document => {
                // Memoized
                if let Some(cached) = self.0.cached_text.read().unwrap().clone() { return cached; }
                let mut out = String::new();
                fn dfs(n: &SimpleNode, out: &mut String) {
                    if n.kind()==NodeKind::Text { if let Some(v)=&*n.0.value.read().unwrap() { out.push_str(v); } }
                    for c in n.children() { dfs(&c, out); }
                }
                dfs(self, &mut out);
                *self.0.cached_text.write().unwrap() = Some(out.clone());
                out
            }
        }
    }
    fn parent(&self) -> Option<Self> { self.0.parent.read().ok()?.as_ref().and_then(|w| w.upgrade()).map(|inner| SimpleNode(inner)) }
    fn children(&self) -> Vec<Self> { self.0.children.read().map(|v| v.clone()).unwrap_or_default() }
    fn attributes(&self) -> Vec<Self> { self.0.attributes.read().map(|v| v.clone()).unwrap_or_default() }
    fn namespaces(&self) -> Vec<Self> { self.0.namespaces.read().map(|v| v.clone()).unwrap_or_default() }
}


// Tests relocated to integration file.
