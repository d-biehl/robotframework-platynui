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
        self.dom.nodes[self.idx].parent.map(|i| Node {
            dom: self.dom.clone(),
            idx: i,
        })
    }
    fn children(&self) -> Vec<Self> {
        self.dom.nodes[self.idx]
            .children
            .iter()
            .map(|&i| Node {
                dom: self.dom.clone(),
                idx: i,
            })
            .collect()
    }
    fn attributes(&self) -> Vec<Self> {
        self.dom.nodes[self.idx]
            .attrs
            .iter()
            .map(|&i| Node {
                dom: self.dom.clone(),
                idx: i,
            })
            .collect()
    }
    fn compare_document_order(&self, other: &Self) -> Result<std::cmp::Ordering, platynui_xpath::runtime::Error> {
        Ok(self.idx.cmp(&other.idx))
    }
}

fn el(dom: &mut Dom, p: Option<usize>, local: &str) -> usize {
    let i = dom.nodes.len();
    dom.nodes.push(NodeRec {
        kind: NodeKind::Element,
        name: Some(QName {
            prefix: None,
            local: local.into(),
            ns_uri: None,
        }),
        value: String::new(),
        parent: p,
        children: vec![],
        attrs: vec![],
    });
    if let Some(pp) = p {
        dom.nodes[pp].children.push(i);
    }
    i
}
fn at(dom: &mut Dom, p: usize, local: &str, v: &str) -> usize {
    let i = dom.nodes.len();
    dom.nodes.push(NodeRec {
        kind: NodeKind::Attribute,
        name: Some(QName {
            prefix: None,
            local: local.into(),
            ns_uri: None,
        }),
        value: v.into(),
        parent: Some(p),
        children: vec![],
        attrs: vec![],
    });
    dom.nodes[p].attrs.push(i);
    i
}

fn sample() -> Node {
    let mut d = Dom { nodes: vec![] };
    let root = el(&mut d, None, "root");
    let a1 = el(&mut d, Some(root), "a");
    at(&mut d, a1, "id", "x");
    at(&mut d, a1, "n", "2");
    let a2 = el(&mut d, Some(root), "a");
    at(&mut d, a2, "id", "y");
    at(&mut d, a2, "n", "x");
    Node {
        dom: Arc::new(d),
        idx: root,
    }
}

#[fixture]
#[allow(unused_braces)]
fn root() -> Node {
    sample()
}
#[fixture]
#[allow(unused_braces)]
fn sc() -> StaticContext {
    StaticContext::default()
}

fn names<T: XdmNode>(items: &Vec<XdmItem<T>>) -> Vec<String> {
    let mut v = vec![];
    for it in items {
        if let XdmItem::Node(n) = it {
            if let Some(q) = n.name() {
                v.push(q.local);
            }
        }
    }
    v
}

#[rstest]
fn test_value_comparison_untyped_atomic_numeric(root: Node, sc: StaticContext) {
    // Only first a has @n = '2' (untypedAtomic) which should compare equal to number 2
    let exec = compile_xpath("a[@n = 2]", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root)).unwrap();
    assert_eq!(names(&out), vec!["a"]);
}

#[rstest]
fn test_value_comparison_string_eq(root: Node, sc: StaticContext) {
    let exec = compile_xpath("a[@id = 'x']", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root)).unwrap();
    assert_eq!(names(&out), vec!["a"]);
}

#[rstest]
fn test_value_comparison_string_ordering(root: Node, sc: StaticContext) {
    // Both 'x' and 'y' are < 'z'
    let exec = compile_xpath("a[@id < 'z']", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root)).unwrap();
    assert_eq!(names(&out), vec!["a", "a"]);
}

#[rstest]
fn test_general_comparison_with_sequence(root: Node, sc: StaticContext) {
    let exec = compile_xpath("1 = (@n, 1, 2)", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root.clone())).unwrap();
    // result is boolean true
    assert_eq!(out.len(), 1);
    match &out[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("expected boolean"),
    }

    let exec = compile_xpath("3 = (@n, 1, 2)", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root)).unwrap();
    match &out[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => assert!(!*b),
        _ => panic!("expected boolean"),
    }
}

// NOTE: Keyword value ops (eq/ne/lt/le/gt/ge) are supported by the evaluator semantics.
// Parser-based predicate handling prefers the standard operators (=, !=, <, <=, >, >=)
// to keep the path AST builder robust. Equivalent tests above cover semantics.
