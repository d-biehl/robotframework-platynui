use platynui_xpath::compile_xpath;
use platynui_xpath::model::{NodeKind, QName, XdmNode};
use platynui_xpath::runtime::StaticContext;
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use rstest::{fixture, rstest};
use std::sync::Arc;

#[derive(Debug, Clone)]
struct Dom {
    nodes: Vec<NodeRec>,
}

#[derive(Debug, Clone)]
struct NodeRec {
    kind: NodeKind,
    name: Option<QName>,
    value: String,
    parent: Option<usize>,
    children: Vec<usize>,
    attrs: Vec<usize>,
    nss: Vec<usize>,
}

#[derive(Debug, Clone)]
struct Node {
    dom: Arc<Dom>,
    idx: usize,
}

impl PartialEq for Node {
    fn eq(&self, o: &Self) -> bool {
        Arc::ptr_eq(&self.dom, &o.dom) && self.idx == o.idx
    }
}
impl Eq for Node {}

impl XdmNode for Node {
    fn kind(&self) -> NodeKind {
        self.dom.nodes[self.idx].kind.clone()
    }
    fn name(&self) -> Option<QName> {
        self.dom.nodes[self.idx].name.clone()
    }
    fn string_value(&self) -> String {
        self.dom.nodes[self.idx].value.clone()
    }
    fn parent(&self) -> Option<Self> {
        self.dom.nodes[self.idx].parent.map(|i| Node { dom: self.dom.clone(), idx: i })
    }
    fn children(&self) -> Vec<Self> {
        self.dom.nodes[self.idx]
            .children
            .iter()
            .map(|&i| Node { dom: self.dom.clone(), idx: i })
            .collect()
    }
    fn attributes(&self) -> Vec<Self> {
        self.dom.nodes[self.idx]
            .attrs
            .iter()
            .map(|&i| Node { dom: self.dom.clone(), idx: i })
            .collect()
    }
    fn namespaces(&self) -> Vec<Self> {
        self.dom.nodes[self.idx]
            .nss
            .iter()
            .map(|&i| Node { dom: self.dom.clone(), idx: i })
            .collect()
    }
    // Intentionally use default fallback (M6); same-root comparisons succeed
}

fn el(dom: &mut Dom, p: Option<usize>, local: &str) -> usize {
    let i = dom.nodes.len();
    dom.nodes.push(NodeRec {
        kind: NodeKind::Element,
        name: Some(QName { prefix: None, local: local.into(), ns_uri: None }),
        value: String::new(),
        parent: p,
        children: vec![],
        attrs: vec![],
        nss: vec![],
    });
    if let Some(pp) = p { dom.nodes[pp].children.push(i); }
    i
}
fn at(dom: &mut Dom, p: usize, local: &str, v: &str) -> usize {
    let i = dom.nodes.len();
    dom.nodes.push(NodeRec {
        kind: NodeKind::Attribute,
        name: Some(QName { prefix: None, local: local.into(), ns_uri: None }),
        value: v.into(),
        parent: Some(p),
        children: vec![],
        attrs: vec![],
        nss: vec![],
    });
    dom.nodes[p].attrs.push(i);
    i
}
fn tx(dom: &mut Dom, p: usize, v: &str) -> usize {
    let i = dom.nodes.len();
    dom.nodes.push(NodeRec {
        kind: NodeKind::Text,
        name: None,
        value: v.into(),
        parent: Some(p),
        children: vec![],
        attrs: vec![],
        nss: vec![],
    });
    dom.nodes[p].children.push(i);
    i
}

fn ns(dom: &mut Dom, p: usize, prefix: &str, uri: &str) -> usize {
    let i = dom.nodes.len();
    dom.nodes.push(NodeRec {
        kind: NodeKind::Namespace,
        name: Some(QName { prefix: None, local: prefix.into(), ns_uri: Some(uri.into()) }),
        value: String::new(),
        parent: Some(p),
        children: vec![],
        attrs: vec![],
        nss: vec![],
    });
    dom.nodes[p].nss.push(i);
    i
}

fn sample() -> Node {
    let mut d = Dom { nodes: vec![] };
    let root = el(&mut d, None, "root");
    let a1 = el(&mut d, Some(root), "a");
    at(&mut d, a1, "id", "x");
    let a2 = el(&mut d, Some(a1), "a");
    let c = el(&mut d, Some(a2), "c");
    tx(&mut d, c, "hi");
    let _d = el(&mut d, Some(root), "d");
    let _a3 = el(&mut d, Some(root), "a");
    Node { dom: Arc::new(d), idx: root }
}

#[fixture]
#[allow(unused_braces)]
fn root() -> Node { sample() }

#[fixture]
#[allow(unused_braces)]
fn sc() -> StaticContext { StaticContext::default() }

fn as_bool<N>(items: &Vec<XdmItem<N>>) -> bool {
    match &items[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => *b,
        _ => panic!("expected boolean"),
    }
}

fn names<T: XdmNode>(items: &Vec<XdmItem<T>>) -> Vec<String> {
    let mut v = vec![];
    for it in items {
        if let XdmItem::Node(n) = it
            && let Some(q) = n.name() { v.push(q.local); }
    }
    v
}

#[rstest]
fn default_fallback_orders_descendants_and_siblings(root: Node, sc: StaticContext) {
    let exec = compile_xpath("//a", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root.clone())).unwrap();
    // Expect three 'a' in document order: root/a1, a2 (descendant), a3 (sibling under root)
    assert_eq!(names(&out), vec!["a", "a", "a"]);
}

#[rstest]
fn default_fallback_supports_node_comparisons(root: Node, sc: StaticContext) {
    let exec = compile_xpath("//a[1] << //a[2]", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root)).unwrap();
    assert!(as_bool(&out));
}

#[rstest]
fn attributes_precede_children_in_fallback(sc: StaticContext) {
    // Build a minimal DOM: <e id="x"><c/></e>
    let mut d = Dom { nodes: vec![] };
    let e = el(&mut d, None, "e");
    let _attr = at(&mut d, e, "id", "x");
    let _c = el(&mut d, Some(e), "c");
    let e_node = Node { dom: Arc::new(d), idx: e };

    // Evaluate on context item 'e': union of attributes and child nodes
    let exec = compile_xpath("@* | node()", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec
        .evaluate(&platynui_xpath::runtime::DynamicContextBuilder::<Node>::new().with_context_item(XdmItem::Node(e_node)).build())
        .unwrap();
    // Expect first item(s) to be attributes, then children (element/text)
    let kinds: Vec<NodeKind> = out
        .iter()
        .filter_map(|it| match it { XdmItem::Node(n) => Some(n.kind()), _ => None })
        .collect();
    assert!(matches!(kinds.first(), Some(NodeKind::Attribute)));
}

#[rstest]
fn namespaces_between_attributes_and_children(sc: StaticContext) {
    // <e id="x" xmlns:p="urn:p"><c/></e>
    let mut d = Dom { nodes: vec![] };
    let e = el(&mut d, None, "e");
    let _attr = at(&mut d, e, "id", "x");
    let _ns = ns(&mut d, e, "p", "urn:p");
    let _c = el(&mut d, Some(e), "c");
    let e_node = Node { dom: Arc::new(d), idx: e };

    let exec = compile_xpath("@* | namespace::* | node()", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec
        .evaluate(&platynui_xpath::runtime::DynamicContextBuilder::<Node>::new().with_context_item(XdmItem::Node(e_node)).build())
        .unwrap();
    let kinds: Vec<NodeKind> = out
        .iter()
        .filter_map(|it| match it { XdmItem::Node(n) => Some(n.kind()), _ => None })
        .collect();
    assert!(kinds.len() >= 2);
    assert!(matches!(kinds.first(), Some(NodeKind::Attribute)));
    assert!(matches!(kinds.get(1), Some(NodeKind::Namespace)));
}

#[rstest]
fn deep_sibling_divergence_ordering(sc: StaticContext) {
    // Build deep divergent leaves l1 (under a1) and l2 (under a2)
    let mut d = Dom { nodes: vec![] };
    let root = el(&mut d, None, "root");
    let a1 = el(&mut d, Some(root), "a1");
    let a2 = el(&mut d, Some(root), "a2");
    let x1 = el(&mut d, Some(a1), "x1");
    let _x2 = el(&mut d, Some(x1), "x2");
    let _l1 = el(&mut d, Some(_x2), "l1");
    let y1 = el(&mut d, Some(a2), "y1");
    let _y2 = el(&mut d, Some(y1), "y2");
    let _l2 = el(&mut d, Some(_y2), "l2");
    let root_node = Node { dom: Arc::new(d), idx: root };

    let exec = compile_xpath("//l1 << //l2", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root_node)).unwrap();
    match &out[0] { XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => assert!(*b), _ => panic!("bool") }
}

// Note: Multi-root sequences with default fallback now panic (enforced adapter override).
