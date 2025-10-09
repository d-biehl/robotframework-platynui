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
//! assert_eq!(root.children().count(), 2); // two child elements
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
//! let root = document.children().next().unwrap();
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
//! let attr_node = r.attributes().next().unwrap();
//! let child_node = r.children().next().unwrap();
//! assert_eq!(attr_node.compare_document_order(&child_node).unwrap(), core::cmp::Ordering::Less);
//! ```
use std::fmt;
use std::sync::{
    Arc, RwLock, Weak,
    atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering},
};

use crate::model::{NodeKind, QName, XdmNode};
use crate::xdm::XdmAtomicValue;

type AxisVecIter = std::vec::IntoIter<SimpleNode>;

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
    doc_id: RwLock<u64>,                 // creation order of document root; inherited by descendants
    doc_order: RwLock<Option<u64>>,      // pre-order position within document
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
        Arc::as_ptr(&self.0).hash(state)
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
        let cached = self.0.cached_text.read().map(|c| c.is_some()).unwrap_or(false);
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
        ds.field("attrs", &attr_count).field("namespaces", &ns_count).field("children", &child_count);
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
            doc_order: RwLock::new(None),
        }))
    }

    pub fn document() -> SimpleNodeBuilder {
        let b = SimpleNodeBuilder::new(NodeKind::Document, None, None);
        *b.node.0.doc_id.write().unwrap_or_else(|e| e.into_inner()) = Self::next_doc_id();
        b
    }
    pub fn element(name: &str) -> SimpleNodeBuilder {
        // Support prefixed element names; actual namespace URI resolution happens during build
        // once in-scope namespaces are attached or resolved via parent at attach time.
        let (prefix, local, ns_uri) = if let Some((pre, loc)) = name.split_once(':') {
            let uri = if pre == "xml" { Some(crate::consts::XML_URI.to_string()) } else { None };
            (Some(pre.to_string()), loc.to_string(), uri)
        } else {
            (None, name.to_string(), None)
        };
        SimpleNodeBuilder::new(NodeKind::Element, Some(QName { prefix, local, ns_uri }), None)
    }
    pub fn attribute(name: &str, value: &str) -> SimpleNode {
        // Support namespaced attributes via prefix:local; bind 'xml' to the canonical XML namespace URI.
        let (prefix, local, ns_uri) = if let Some((pre, loc)) = name.split_once(':') {
            let uri = if pre == "xml" { Some(crate::consts::XML_URI.to_string()) } else { None };
            (Some(pre.to_string()), loc.to_string(), uri)
        } else {
            (None, name.to_string(), None)
        };
        SimpleNode::new(NodeKind::Attribute, Some(QName { prefix, local, ns_uri }), Some(value.to_string()))
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
            Some(QName { prefix: None, local: target.to_string(), ns_uri: None }),
            Some(data.to_string()),
        )
    }
    pub fn namespace(prefix: &str, uri: &str) -> SimpleNode {
        SimpleNode::new(
            NodeKind::Namespace,
            Some(QName { prefix: Some(prefix.to_string()), local: prefix.to_string(), ns_uri: Some(uri.to_string()) }),
            Some(uri.to_string()),
        )
    }

    /// Resolve namespace prefix by walking ancestor chain (including self)
    pub fn lookup_namespace_uri(&self, prefix: &str) -> Option<String> {
        let mut cur: Option<SimpleNode> = Some(self.clone());
        while let Some(n) = cur {
            for ns in n.namespaces() {
                if let Some(name) = ns.name()
                    && name.prefix.as_deref() == Some(prefix)
                {
                    return ns.string_value().into();
                }
            }
            cur = n.parent();
        }
        None
    }

    fn set_doc_order(&self, value: u64) {
        *self.0.doc_order.write().unwrap_or_else(|e| e.into_inner()) = Some(value);
    }

    pub fn set_doc_id_recursive(&self, doc_id: u64) {
        *self.0.doc_id.write().unwrap_or_else(|e| e.into_inner()) = doc_id;
        {
            let attrs = self.0.attributes.read().unwrap_or_else(|e| e.into_inner()).clone();
            for attr in attrs {
                attr.set_doc_id_recursive(doc_id);
            }
        }
        {
            let namespaces = self.0.namespaces.read().unwrap_or_else(|e| e.into_inner()).clone();
            for ns in namespaces {
                ns.set_doc_id_recursive(doc_id);
            }
        }
        {
            let children = self.0.children.read().unwrap_or_else(|e| e.into_inner()).clone();
            for child in children {
                child.set_doc_id_recursive(doc_id);
            }
        }
    }

    fn assign_document_order_with_counter(&self, counter: &mut u64) {
        *counter += 1;
        self.set_doc_order(*counter);

        if matches!(self.kind(), NodeKind::Element) {
            let attrs = self.0.attributes.read().unwrap_or_else(|e| e.into_inner()).clone();
            for attr in attrs {
                attr.assign_document_order_with_counter(counter);
            }
            let namespaces = self.0.namespaces.read().unwrap_or_else(|e| e.into_inner()).clone();
            for ns in namespaces {
                ns.assign_document_order_with_counter(counter);
            }
        }

        let children = self.0.children.read().unwrap_or_else(|e| e.into_inner()).clone();
        for child in children {
            child.assign_document_order_with_counter(counter);
        }
    }

    fn assign_document_order(&self) {
        let mut counter = 0u64;
        self.assign_document_order_with_counter(&mut counter);
    }

    fn doc_order(&self) -> Option<u64> {
        *self.0.doc_order.read().unwrap_or_else(|e| e.into_inner())
    }
}

impl fmt::Display for SimpleNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn qname_to_string(q: &QName) -> String {
            match (&q.prefix, &q.local) {
                (Some(p), local) if !p.is_empty() => format!("{}:{}", p, local),
                _ => q.local.clone(),
            }
        }
        fn clip(s: &str) -> String {
            if s.len() > crate::consts::DISPLAY_CLIP_MAX {
                let mut out = s.chars().take(crate::consts::DISPLAY_CLIP_MAX).collect::<String>();
                out.push('…');
                out
            } else {
                s.to_string()
            }
        }
        match self.kind() {
            NodeKind::Document => {
                let ch = self.children().count();
                write!(f, "document(children={})", ch)
            }
            NodeKind::Element => {
                let name = self.name().map(|q| qname_to_string(&q)).unwrap_or_else(|| "<unnamed>".to_string());
                let attrs = self.attributes().count();
                let ch = self.children().count();
                write!(f, "<{} attrs={} children={}>", name, attrs, ch)
            }
            NodeKind::Attribute => {
                let name = self.name().map(|q| qname_to_string(&q)).unwrap_or_else(|| "?".to_string());
                let val = clip(&self.string_value());
                write!(f, "@{}=\"{}\"", name, val)
            }
            NodeKind::Text => {
                let val = clip(&self.string_value());
                write!(f, "\"{}\"", val)
            }
            NodeKind::Comment => {
                let val = clip(&self.string_value());
                write!(f, "<!--{}-->", val)
            }
            NodeKind::ProcessingInstruction => {
                let target = self.name().map(|q| q.local).unwrap_or_default();
                let data = clip(&self.string_value());
                if target.is_empty() { write!(f, "<?{}?>", data) } else { write!(f, "<?{} {}?>", target, data) }
            }
            NodeKind::Namespace => {
                let prefix = self.name().and_then(|q| q.prefix).unwrap_or_default();
                let uri = self.string_value();
                if prefix.is_empty() {
                    write!(f, "xmlns=\"{}\"", uri)
                } else {
                    write!(f, "xmlns:{}=\"{}\"", prefix, uri)
                }
            }
        }
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
            NodeKind::Text | NodeKind::Comment | NodeKind::ProcessingInstruction | NodeKind::Attribute
        ) {
            *self.node.0.value.write().unwrap_or_else(|e| e.into_inner()) = Some(v.to_string());
        }
        self
    }
    pub fn build(self) -> SimpleNode {
        // finalize relationships
        {
            let mut id = self.node.0.doc_id.write().unwrap_or_else(|e| e.into_inner());
            if *id == 0 {
                *id = SimpleNode::next_doc_id();
            }
        }
        {
            let mut nss = self.node.0.namespaces.write().unwrap_or_else(|e| e.into_inner());
            for n in &self.pending_ns {
                *n.0.parent.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::downgrade(&self.node.0));
                let id = *self.node.0.doc_id.read().unwrap_or_else(|e| e.into_inner());
                n.set_doc_id_recursive(id);
            }
            nss.extend(self.pending_ns);
        }
        {
            let mut attrs = self.node.0.attributes.write().unwrap_or_else(|e| e.into_inner());
            for a in self.pending_attrs {
                // Resolve attribute namespace prefix using in-scope namespaces of the element.
                // Default namespace does not apply to attributes; only prefixed names are resolved.
                let mut pushed = false;
                if let Some(qn) = &a.0.name
                    && let Some(pref) = &qn.prefix
                {
                    let uri = if pref == "xml" {
                        Some(crate::consts::XML_URI.to_string())
                    } else {
                        self.node.lookup_namespace_uri(pref)
                    };
                    if let Some(ns_uri) = uri {
                        // Rebuild attribute node with resolved ns_uri
                        let val = a.0.value.read().unwrap_or_else(|e| e.into_inner()).clone();
                        let rebuilt = SimpleNode::new(
                            NodeKind::Attribute,
                            Some(QName { prefix: Some(pref.clone()), local: qn.local.clone(), ns_uri: Some(ns_uri) }),
                            val,
                        );
                        *rebuilt.0.parent.write().unwrap_or_else(|e| e.into_inner()) =
                            Some(Arc::downgrade(&self.node.0));
                        let id = *self.node.0.doc_id.read().unwrap_or_else(|e| e.into_inner());
                        rebuilt.set_doc_id_recursive(id);
                        attrs.push(rebuilt);
                        pushed = true;
                    }
                }
                if !pushed {
                    *a.0.parent.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::downgrade(&self.node.0));
                    let id = *self.node.0.doc_id.read().unwrap_or_else(|e| e.into_inner());
                    a.set_doc_id_recursive(id);
                    attrs.push(a);
                }
            }
        }
        {
            let mut ch = self.node.0.children.write().unwrap_or_else(|e| e.into_inner());
            for c in self.pending_children {
                *c.0.parent.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::downgrade(&self.node.0));
                let idc = *self.node.0.doc_id.read().unwrap_or_else(|e| e.into_inner());
                c.set_doc_id_recursive(idc);
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
                    let attrs = node.0.attributes.read().unwrap_or_else(|e| e.into_inner());
                    for (idx, a) in attrs.iter().enumerate() {
                        if let Some(q) = a.name()
                            && let Some(pref) = q.prefix.as_ref()
                        {
                            // Only replace if ns_uri is None
                            if q.ns_uri.is_none() {
                                let uri = if pref == "xml" {
                                    Some(crate::consts::XML_URI.to_string())
                                } else {
                                    node.lookup_namespace_uri(pref)
                                };
                                if let Some(ns_uri) = uri {
                                    let val = a.0.value.read().unwrap_or_else(|e| e.into_inner()).clone();
                                    let rebuilt = SimpleNode::new(
                                        NodeKind::Attribute,
                                        Some(QName {
                                            prefix: Some(pref.clone()),
                                            local: q.local.clone(),
                                            ns_uri: Some(ns_uri),
                                        }),
                                        val,
                                    );
                                    *rebuilt.0.parent.write().unwrap_or_else(|e| e.into_inner()) =
                                        Some(Arc::downgrade(&node.0));
                                    let id = *node.0.doc_id.read().unwrap_or_else(|e| e.into_inner());
                                    *rebuilt.0.doc_id.write().unwrap_or_else(|e| e.into_inner()) = id;
                                    to_replace.push((idx, rebuilt));
                                }
                            }
                        }
                    }
                }
                if !to_replace.is_empty() {
                    let mut attrs_w = node.0.attributes.write().unwrap_or_else(|e| e.into_inner());
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
        self.node.assign_document_order();
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
    type Children<'a>
        = AxisVecIter
    where
        Self: 'a;
    type Attributes<'a>
        = AxisVecIter
    where
        Self: 'a;
    type Namespaces<'a>
        = AxisVecIter
    where
        Self: 'a;

    fn kind(&self) -> NodeKind {
        self.0.kind.clone()
    }
    fn name(&self) -> Option<QName> {
        self.0.name.clone()
    }
    fn typed_value(&self) -> Vec<XdmAtomicValue> {
        match self.kind() {
            NodeKind::Text
            | NodeKind::Attribute
            | NodeKind::Comment
            | NodeKind::ProcessingInstruction
            | NodeKind::Namespace => {
                let value = self.0.value.read().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_default();
                vec![XdmAtomicValue::UntypedAtomic(value)]
            }
            NodeKind::Element | NodeKind::Document => {
                if let Some(cached) = self.0.cached_text.read().unwrap_or_else(|e| e.into_inner()).clone() {
                    return vec![XdmAtomicValue::UntypedAtomic(cached)];
                }

                let mut out = String::new();
                fn dfs(n: &SimpleNode, out: &mut String) {
                    if n.kind() == NodeKind::Text
                        && let Some(v) = &*n.0.value.read().unwrap_or_else(|e| e.into_inner())
                    {
                        out.push_str(v);
                    }
                    for c in n.children() {
                        dfs(&c, out);
                    }
                }
                dfs(self, &mut out);
                *self.0.cached_text.write().unwrap_or_else(|e| e.into_inner()) = Some(out.clone());
                vec![XdmAtomicValue::UntypedAtomic(out)]
            }
        }
    }
    fn parent(&self) -> Option<Self> {
        self.0.parent.read().ok()?.as_ref().and_then(|w| w.upgrade()).map(SimpleNode)
    }
    fn children(&self) -> Self::Children<'_> {
        self.0.children.read().map(|v| v.clone()).unwrap_or_default().into_iter()
    }
    fn attributes(&self) -> Self::Attributes<'_> {
        self.0.attributes.read().map(|v| v.clone()).unwrap_or_default().into_iter()
    }
    fn namespaces(&self) -> Self::Namespaces<'_> {
        // Start with stored namespaces, then ensure implicit xml binding exists and deduplicate by prefix.
        let stored: Vec<Self> = self.0.namespaces.read().map(|v| v.clone()).unwrap_or_default();
        let mut out: Vec<Self> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        // Canonical xml URI
        // 1) Add stored namespaces, but skip duplicates by prefix and ignore invalid attempts to override 'xml'
        for ns in stored {
            let name = ns.name();
            let prefix = name.as_ref().and_then(|q| q.prefix.clone()).unwrap_or_default();
            if prefix == "xml" {
                // Only accept if URI is canonical; otherwise ignore (reserved cannot be rebound)
                let uri = ns.string_value();
                if uri != crate::consts::XML_URI {
                    continue;
                }
            }
            if seen.insert(prefix.clone()) {
                out.push(ns);
            }
        }
        // 2) Synthesize xml binding if not present
        if !seen.contains("xml") {
            let xml = SimpleNode::namespace("xml", crate::consts::XML_URI);
            // set parent to this element for proper ancestry comparisons
            *xml.0.parent.write().unwrap_or_else(|e| e.into_inner()) = Some(std::sync::Arc::downgrade(&self.0));
            out.push(xml);
        }
        out.into_iter()
    }

    fn doc_order_key(&self) -> Option<u64> {
        let doc_id = *self.0.doc_id.read().ok()?;
        let ord = self.doc_order()?;
        Some((doc_id << 32) | (ord & 0xFFFF_FFFF))
    }

    fn compare_document_order(&self, other: &Self) -> Result<core::cmp::Ordering, crate::engine::runtime::Error> {
        let self_doc_id = *self.0.doc_id.read().unwrap_or_else(|e| e.into_inner());
        let other_doc_id = *other.0.doc_id.read().unwrap_or_else(|e| e.into_inner());
        if self_doc_id == other_doc_id
            && let (Some(a), Some(b)) = (self.doc_order(), other.doc_order())
        {
            return Ok(a.cmp(&b));
        }
        match crate::model::try_compare_by_ancestry(self, other) {
            Ok(ord) => Ok(ord),
            Err(e) => {
                if SIMPLE_NODE_CROSS_DOC_ORDER.load(AtomicOrdering::Relaxed) {
                    if self_doc_id != other_doc_id {
                        return Ok(self_doc_id.cmp(&other_doc_id));
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
