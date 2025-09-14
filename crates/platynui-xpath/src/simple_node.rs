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
use std::sync::{
    Arc, RwLock, Weak,
    atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering},
};

use crate::model::{NodeKind, QName, XdmNode};

const XML_URI: &str = "http://www.w3.org/XML/1998/namespace";

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
    doc_id: RwLock<u64>, // creation order of document root; inherited by descendants
}

/// A simple Arc-backed node implementation.
#[derive(Clone)]
pub struct SimpleNode(pub(crate) Arc<Inner>);

impl PartialEq for SimpleNode {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for SimpleNode {}
impl std::hash::Hash for SimpleNode {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.0) as *const Inner).hash(state)
    }
}

impl fmt::Debug for SimpleNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let id = Arc::as_ptr(&self.0) as usize;
        let kind = &self.0.kind;
        let name = &self.0.name;
        let value = self.0.value.read().ok().and_then(|v| v.clone());
        let attr_count = self.0.attributes.read().map(|v| v.len()).unwrap_or(0);
        let ns_count = self.0.namespaces.read().map(|v| v.len()).unwrap_or(0);
        let child_count = self.0.children.read().map(|v| v.len()).unwrap_or(0);
        let cached = self
            .0
            .cached_text
            .read()
            .map(|c| c.is_some())
            .unwrap_or(false);
        let mut ds = f.debug_struct("SimpleNode");
        ds.field("id", &format_args!("0x{id:016x}"));
        ds.field("kind", kind);
        ds.field("name", name);
        if matches!(
            kind,
            NodeKind::Text
                | NodeKind::Attribute
                | NodeKind::Comment
                | NodeKind::ProcessingInstruction
                | NodeKind::Namespace
        ) {
            ds.field("value", &value);
        }
        ds.field("attrs", &attr_count)
            .field("namespaces", &ns_count)
            .field("children", &child_count);
        if matches!(kind, NodeKind::Element | NodeKind::Document) {
            ds.field("cached_text", &cached);
        }
        ds.finish()
    }
}

impl SimpleNode {
    fn next_doc_id() -> u64 {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        COUNTER.fetch_add(1, AtomicOrdering::Relaxed)
    }
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
            doc_id: RwLock::new(0),
        }))
    }

    pub fn document() -> SimpleNodeBuilder {
        let b = SimpleNodeBuilder::new(NodeKind::Document, None, None);
        *b.node.0.doc_id.write().unwrap() = Self::next_doc_id();
        b
    }
    pub fn element(name: &str) -> SimpleNodeBuilder {
        // Support prefixed element names; actual namespace URI resolution happens during build
        // once in-scope namespaces are attached or resolved via parent at attach time.
        let (prefix, local, ns_uri) = if let Some((pre, loc)) = name.split_once(':') {
            let uri = if pre == "xml" {
                Some(XML_URI.to_string())
            } else {
                None
            };
            (Some(pre.to_string()), loc.to_string(), uri)
        } else {
            (None, name.to_string(), None)
        };
        SimpleNodeBuilder::new(
            NodeKind::Element,
            Some(QName {
                prefix,
                local,
                ns_uri,
            }),
            None,
        )
    }
    pub fn attribute(name: &str, value: &str) -> SimpleNode {
        // Support namespaced attributes via prefix:local; bind 'xml' to the canonical XML namespace URI.
        let (prefix, local, ns_uri) = if let Some((pre, loc)) = name.split_once(':') {
            let uri = if pre == "xml" {
                Some(XML_URI.to_string())
            } else {
                None
            };
            (Some(pre.to_string()), loc.to_string(), uri)
        } else {
            (None, name.to_string(), None)
        };
        SimpleNode::new(
            NodeKind::Attribute,
            Some(QName {
                prefix,
                local,
                ns_uri,
            }),
            Some(value.to_string()),
        )
    }
    pub fn text(value: &str) -> SimpleNode {
        SimpleNode::new(NodeKind::Text, None, Some(value.to_string()))
    }
    pub fn comment(value: &str) -> SimpleNode {
        SimpleNode::new(NodeKind::Comment, None, Some(value.to_string()))
    }
    pub fn pi(target: &str, data: &str) -> SimpleNode {
        SimpleNode::new(
            NodeKind::ProcessingInstruction,
            Some(QName {
                prefix: None,
                local: target.to_string(),
                ns_uri: None,
            }),
            Some(data.to_string()),
        )
    }
    pub fn namespace(prefix: &str, uri: &str) -> SimpleNode {
        SimpleNode::new(
            NodeKind::Namespace,
            Some(QName {
                prefix: Some(prefix.to_string()),
                local: prefix.to_string(),
                ns_uri: Some(uri.to_string()),
            }),
            Some(uri.to_string()),
        )
    }

    /// Resolve namespace prefix by walking ancestor chain (including self)
    pub fn lookup_namespace_uri(&self, prefix: &str) -> Option<String> {
        let mut cur: Option<SimpleNode> = Some(self.clone());
        while let Some(n) = cur {
            for ns in n.namespaces() {
                if let Some(name) = ns.name() {
                    if name.prefix.as_deref() == Some(prefix) {
                        return ns.string_value().into();
                    }
                }
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
    fn new(kind: NodeKind, name: Option<QName>, value: Option<String>) -> Self {
        Self {
            node: SimpleNode::new(kind, name, value),
            pending_children: Vec::new(),
            pending_attrs: Vec::new(),
            pending_ns: Vec::new(),
        }
    }

    pub fn child(mut self, child_builder: impl Into<SimpleNodeOrBuilder>) -> Self {
        match child_builder.into() {
            SimpleNodeOrBuilder::Built(n) => self.pending_children.push(n),
            SimpleNodeOrBuilder::Builder(b) => self.pending_children.push(b.build()),
        }
        self
    }
    pub fn children<I: IntoIterator<Item = SimpleNodeOrBuilder>>(mut self, it: I) -> Self {
        for c in it {
            match c {
                SimpleNodeOrBuilder::Built(n) => self.pending_children.push(n),
                SimpleNodeOrBuilder::Builder(b) => self.pending_children.push(b.build()),
            }
        }
        self
    }
    pub fn attr(mut self, attr: SimpleNode) -> Self {
        debug_assert!(attr.kind() == NodeKind::Attribute);
        self.pending_attrs.push(attr);
        self
    }
    pub fn attrs<I: IntoIterator<Item = SimpleNode>>(mut self, attrs: I) -> Self {
        for a in attrs {
            debug_assert!(a.kind() == NodeKind::Attribute);
            self.pending_attrs.push(a);
        }
        self
    }
    pub fn namespace(mut self, ns: SimpleNode) -> Self {
        debug_assert!(ns.kind() == NodeKind::Namespace);
        self.pending_ns.push(ns);
        self
    }
    pub fn namespaces<I: IntoIterator<Item = SimpleNode>>(mut self, it: I) -> Self {
        for n in it {
            debug_assert!(n.kind() == NodeKind::Namespace);
            self.pending_ns.push(n);
        }
        self
    }
    pub fn value(self, v: &str) -> Self {
        if matches!(
            self.node.kind(),
            NodeKind::Text
                | NodeKind::Comment
                | NodeKind::ProcessingInstruction
                | NodeKind::Attribute
        ) {
            *self.node.0.value.write().unwrap() = Some(v.to_string());
        }
        self
    }
    pub fn build(self) -> SimpleNode {
        // finalize relationships
        {
            let mut nss = self.node.0.namespaces.write().unwrap();
            for n in &self.pending_ns {
                *n.0.parent.write().unwrap() = Some(Arc::downgrade(&self.node.0));
                let id = *self.node.0.doc_id.read().unwrap();
                *n.0.doc_id.write().unwrap() = id;
            }
            nss.extend(self.pending_ns);
        }
        {
            let mut attrs = self.node.0.attributes.write().unwrap();
            for a in self.pending_attrs {
                // Resolve attribute namespace prefix using in-scope namespaces of the element.
                // Default namespace does not apply to attributes; only prefixed names are resolved.
                let mut pushed = false;
                if let Some(qn) = &a.0.name {
                    if let Some(pref) = &qn.prefix {
                        let uri = if pref == "xml" {
                            Some(XML_URI.to_string())
                        } else {
                            self.node.lookup_namespace_uri(pref)
                        };
                        if let Some(ns_uri) = uri {
                            // Rebuild attribute node with resolved ns_uri
                            let val = a.0.value.read().unwrap().clone();
                            let rebuilt = SimpleNode::new(
                                NodeKind::Attribute,
                                Some(QName {
                                    prefix: Some(pref.clone()),
                                    local: qn.local.clone(),
                                    ns_uri: Some(ns_uri),
                                }),
                                val,
                            );
                            *rebuilt.0.parent.write().unwrap() = Some(Arc::downgrade(&self.node.0));
                            let id = *self.node.0.doc_id.read().unwrap();
                            *rebuilt.0.doc_id.write().unwrap() = id;
                            attrs.push(rebuilt);
                            pushed = true;
                        }
                    }
                }
                if !pushed {
                    *a.0.parent.write().unwrap() = Some(Arc::downgrade(&self.node.0));
                    let id = *self.node.0.doc_id.read().unwrap();
                    *a.0.doc_id.write().unwrap() = id;
                    attrs.push(a);
                }
            }
        }
        {
            let mut ch = self.node.0.children.write().unwrap();
            for c in self.pending_children {
                *c.0.parent.write().unwrap() = Some(Arc::downgrade(&self.node.0));
                let idc = *self.node.0.doc_id.read().unwrap();
                *c.0.doc_id.write().unwrap() = idc;
                ch.push(c);
            }
        }
        // Precompute cached text for element/document
        if matches!(self.node.kind(), NodeKind::Element | NodeKind::Document) {
            let _ = self.node.string_value();
        }
        // Post-pass: resolve attribute namespace URIs using ancestor bindings now that parent links exist.
        fn resolve_attr_ns_deep(node: &SimpleNode) {
            use crate::model::NodeKind;
            if matches!(node.kind(), NodeKind::Element) {
                // Resolve on this element
                let mut to_replace: Vec<(usize, SimpleNode)> = Vec::new();
                {
                    let attrs = node.0.attributes.read().unwrap();
                    for (idx, a) in attrs.iter().enumerate() {
                        if let Some(q) = a.name() {
                            if let Some(pref) = q.prefix.as_ref() {
                                // Only replace if ns_uri is None
                                if q.ns_uri.is_none() {
                                    let uri = if pref == "xml" {
                                        Some(XML_URI.to_string())
                                    } else {
                                        node.lookup_namespace_uri(pref)
                                    };
                                    if let Some(ns_uri) = uri {
                                        let val = a.0.value.read().unwrap().clone();
                                        let rebuilt = SimpleNode::new(
                                            NodeKind::Attribute,
                                            Some(QName {
                                                prefix: Some(pref.clone()),
                                                local: q.local.clone(),
                                                ns_uri: Some(ns_uri),
                                            }),
                                            val,
                                        );
                                        *rebuilt.0.parent.write().unwrap() =
                                            Some(Arc::downgrade(&node.0));
                                        let id = *node.0.doc_id.read().unwrap();
                                        *rebuilt.0.doc_id.write().unwrap() = id;
                                        to_replace.push((idx, rebuilt));
                                    }
                                }
                            }
                        }
                    }
                }
                if !to_replace.is_empty() {
                    let mut attrs_w = node.0.attributes.write().unwrap();
                    for (idx, new_attr) in to_replace {
                        attrs_w[idx] = new_attr;
                    }
                }
            }
            // Recurse into children
            let children = node.children();
            for c in children {
                resolve_attr_ns_deep(&c);
            }
        }
        resolve_attr_ns_deep(&self.node);
        self.node
    }
}

pub enum SimpleNodeOrBuilder {
    Built(SimpleNode),
    Builder(SimpleNodeBuilder),
}
impl From<SimpleNode> for SimpleNodeOrBuilder {
    fn from(n: SimpleNode) -> Self {
        SimpleNodeOrBuilder::Built(n)
    }
}
impl From<SimpleNodeBuilder> for SimpleNodeOrBuilder {
    fn from(b: SimpleNodeBuilder) -> Self {
        SimpleNodeOrBuilder::Builder(b)
    }
}

// Convenience helper functions for concise test code
pub fn elem(name: &str) -> SimpleNodeBuilder {
    SimpleNode::element(name)
}
pub fn text(v: &str) -> SimpleNode {
    SimpleNode::text(v)
}
pub fn attr(name: &str, v: &str) -> SimpleNode {
    SimpleNode::attribute(name, v)
}
pub fn comment(v: &str) -> SimpleNode {
    SimpleNode::comment(v)
}
pub fn ns(prefix: &str, uri: &str) -> SimpleNode {
    SimpleNode::namespace(prefix, uri)
}
pub fn doc() -> SimpleNodeBuilder {
    SimpleNode::document()
}

impl XdmNode for SimpleNode {
    fn kind(&self) -> NodeKind {
        self.0.kind.clone()
    }
    fn name(&self) -> Option<QName> {
        self.0.name.clone()
    }
    fn string_value(&self) -> String {
        match self.kind() {
            NodeKind::Text
            | NodeKind::Attribute
            | NodeKind::Comment
            | NodeKind::ProcessingInstruction
            | NodeKind::Namespace => self.0.value.read().unwrap().clone().unwrap_or_default(),
            NodeKind::Element | NodeKind::Document => {
                // Memoized
                if let Some(cached) = self.0.cached_text.read().unwrap().clone() {
                    return cached;
                }
                let mut out = String::new();
                fn dfs(n: &SimpleNode, out: &mut String) {
                    if n.kind() == NodeKind::Text {
                        if let Some(v) = &*n.0.value.read().unwrap() {
                            out.push_str(v);
                        }
                    }
                    for c in n.children() {
                        dfs(&c, out);
                    }
                }
                dfs(self, &mut out);
                *self.0.cached_text.write().unwrap() = Some(out.clone());
                out
            }
        }
    }
    fn parent(&self) -> Option<Self> {
        self.0
            .parent
            .read()
            .ok()?
            .as_ref()
            .and_then(|w| w.upgrade())
            .map(|inner| SimpleNode(inner))
    }
    fn children(&self) -> Vec<Self> {
        self.0
            .children
            .read()
            .map(|v| v.clone())
            .unwrap_or_default()
    }
    fn attributes(&self) -> Vec<Self> {
        self.0
            .attributes
            .read()
            .map(|v| v.clone())
            .unwrap_or_default()
    }
    fn namespaces(&self) -> Vec<Self> {
        // Start with stored namespaces, then ensure implicit xml binding exists and deduplicate by prefix.
        let stored: Vec<Self> = self
            .0
            .namespaces
            .read()
            .map(|v| v.clone())
            .unwrap_or_default();
        let mut out: Vec<Self> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        // Canonical xml URI
        const XML_URI: &str = "http://www.w3.org/XML/1998/namespace";
        // 1) Add stored namespaces, but skip duplicates by prefix and ignore invalid attempts to override 'xml'
        for ns in stored {
            let name = ns.name();
            let prefix = name
                .as_ref()
                .and_then(|q| q.prefix.clone())
                .unwrap_or_default();
            if prefix == "xml" {
                // Only accept if URI is canonical; otherwise ignore (reserved cannot be rebound)
                let uri = ns.string_value();
                if uri != XML_URI {
                    continue;
                }
            }
            if seen.insert(prefix.clone()) {
                out.push(ns);
            }
        }
        // 2) Synthesize xml binding if not present
        if !seen.contains("xml") {
            let xml = SimpleNode::namespace("xml", XML_URI);
            // set parent to this element for proper ancestry comparisons
            *xml.0.parent.write().unwrap() = Some(std::sync::Arc::downgrade(&self.0));
            out.push(xml);
        }
        out
    }

    fn compare_document_order(
        &self,
        other: &Self,
    ) -> Result<core::cmp::Ordering, crate::runtime::Error> {
        match crate::model::try_compare_by_ancestry(self, other) {
            Ok(ord) => Ok(ord),
            Err(e) => {
                if SIMPLE_NODE_CROSS_DOC_ORDER.load(AtomicOrdering::Relaxed) {
                    let a = *self.0.doc_id.read().unwrap();
                    let b = *other.0.doc_id.read().unwrap();
                    if a != b {
                        return Ok(a.cmp(&b));
                    }
                    let pa = Arc::as_ptr(&self.0) as usize;
                    let pb = Arc::as_ptr(&other.0) as usize;
                    Ok(pa.cmp(&pb))
                } else {
                    Err(e)
                }
            }
        }
    }
}

// Global opt-in for cross-document order on SimpleNode. Off by default to preserve prior semantics.
static SIMPLE_NODE_CROSS_DOC_ORDER: AtomicBool = AtomicBool::new(false);

/// Enable or disable cross-document total order for SimpleNode.
/// When enabled, nodes from different document roots are compared by creation order of the
/// document (and raw pointer address as a stable tie-breaker within the process).
pub fn set_cross_document_order(enable: bool) {
    SIMPLE_NODE_CROSS_DOC_ORDER.store(enable, AtomicOrdering::Relaxed);
}

// Tests relocated to integration file.
