use rstest::{rstest, fixture};
use std::sync::Arc;
use platynui_xpath::compile_xpath;
use platynui_xpath::model::{XdmNode, NodeKind, QName};
use platynui_xpath::runtime::StaticContext;
use platynui_xpath::xdm::XdmItem;

#[derive(Debug, Clone)]
struct Dom { nodes: Vec<NodeRec> }

#[derive(Debug, Clone)]
struct NodeRec {
    kind: NodeKind,
    name: Option<QName>,
    value: String,
    parent: Option<usize>,
    children: Vec<usize>,
    attrs: Vec<usize>,
}

#[derive(Debug, Clone)]
struct Node { dom: Arc<Dom>, idx: usize }

impl PartialEq for Node { fn eq(&self, o: &Self) -> bool { Arc::ptr_eq(&self.dom, &o.dom) && self.idx == o.idx } }
impl Eq for Node {}

impl XdmNode for Node {
    fn kind(&self) -> NodeKind { self.dom.nodes[self.idx].kind.clone() }
    fn name(&self) -> Option<QName> { self.dom.nodes[self.idx].name.clone() }
    fn string_value(&self) -> String { self.dom.nodes[self.idx].value.clone() }
    fn parent(&self) -> Option<Self> { self.dom.nodes[self.idx].parent.map(|i| Node { dom: self.dom.clone(), idx: i }) }
    fn children(&self) -> Vec<Self> { self.dom.nodes[self.idx].children.iter().map(|&i| Node { dom: self.dom.clone(), idx: i }).collect() }
    fn attributes(&self) -> Vec<Self> { self.dom.nodes[self.idx].attrs.iter().map(|&i| Node { dom: self.dom.clone(), idx: i }).collect() }
    fn namespaces(&self) -> Vec<Self> { Vec::new() }
    fn compare_document_order(&self, other: &Self) -> std::cmp::Ordering { self.idx.cmp(&other.idx) }
}

fn el_ns(dom: &mut Dom, parent: Option<usize>, local: &str, ns_uri: Option<&str>, prefix: Option<&str>) -> usize {
    let idx = dom.nodes.len();
    dom.nodes.push(NodeRec { kind: NodeKind::Element, name: Some(QName { prefix: prefix.map(|s| s.to_string()), local: local.into(), ns_uri: ns_uri.map(|s| s.to_string()) }), value: String::new(), parent, children: vec![], attrs: vec![] });
    if let Some(p) = parent { dom.nodes[p].children.push(idx); }
    idx
}

fn at_ns(dom: &mut Dom, parent: usize, local: &str, ns_uri: Option<&str>, prefix: Option<&str>, value: &str) -> usize {
    let idx = dom.nodes.len();
    dom.nodes.push(NodeRec { kind: NodeKind::Attribute, name: Some(QName { prefix: prefix.map(|s| s.to_string()), local: local.into(), ns_uri: ns_uri.map(|s| s.to_string()) }), value: value.into(), parent: Some(parent), children: vec![], attrs: vec![] });
    dom.nodes[parent].attrs.push(idx);
    idx
}

fn tx(dom: &mut Dom, parent: usize, value: &str) -> usize {
    let idx = dom.nodes.len();
    dom.nodes.push(NodeRec { kind: NodeKind::Text, name: None, value: value.into(), parent: Some(parent), children: vec![], attrs: vec![] });
    dom.nodes[parent].children.push(idx);
    idx
}

fn sample_ns_tree() -> Node {
    let mut d = Dom { nodes: vec![] };
    let root = el_ns(&mut d, None, "root", None, None);
    let b_ns = el_ns(&mut d, Some(root), "book", Some("http://ex/ns"), Some("ns"));
    at_ns(&mut d, b_ns, "id", None, None, "x");
    tx(&mut d, b_ns, "Hello");
    let _b_no = el_ns(&mut d, Some(root), "book", None, None);
    Node { dom: Arc::new(d), idx: root }
}

fn names<T: XdmNode>(items: &Vec<XdmItem<T>>) -> Vec<String> {
    let mut v = vec![];
    for it in items {
        if let XdmItem::Node(n) = it { if let Some(q) = n.name() { v.push(q.local); } }
    }
    v
}

#[fixture]
fn root() -> Node { sample_ns_tree() }
#[fixture]
fn sc() -> StaticContext { StaticContext::default() }

#[rstest]
fn default_element_namespace_and_attribute_no_default(root: Node, mut sc: StaticContext) {
    sc.namespaces.by_prefix.insert("".into(), "http://ex/ns".into());
    // book should match only ns book due to default element ns
    let exec = compile_xpath("book", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root.clone())).unwrap();
    assert_eq!(names(&out), vec!["book"]);

    // Attribute should not use default element ns
    let exec = compile_xpath("book/@id", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root.clone())).unwrap();
    assert_eq!(names(&out), vec!["id"]);
}

#[rstest]
fn prefixed_qname_and_local_wildcard(root: Node, mut sc: StaticContext) {
    sc.namespaces.by_prefix.insert("ns".into(), "http://ex/ns".into());
    // ns:book matches namespaced element
    let exec = compile_xpath("ns:book", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root.clone())).unwrap();
    assert_eq!(names(&out), vec!["book"]);

    // *:book matches both namespaced and non-namespaced book
    let exec = compile_xpath("*:book", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root)).unwrap();
    assert_eq!(names(&out), vec!["book","book"]);
}

#[rstest]
fn kind_text_and_node_any(root: Node, mut sc: StaticContext) {
    sc.namespaces.by_prefix.insert("ns".into(), "http://ex/ns".into());

    // text() returns text node children under ns:book
    let exec = compile_xpath("ns:book/text()", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root.clone())).unwrap();
    assert_eq!(out.len(), 1);

    // node() returns all child nodes under ns:book (text + attributes are picked via child axis? Only children => element/text/PI/comment; attributes via attribute axis)
    let exec = compile_xpath("ns:book/node()", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root)).unwrap();
    // one text() child in our sample
    assert_eq!(out.len(), 1);
}
