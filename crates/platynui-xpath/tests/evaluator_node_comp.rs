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
        Vec::new()
    }
    fn compare_document_order(&self, other: &Self) -> std::cmp::Ordering {
        self.idx.cmp(&other.idx)
    }
}

fn el(dom: &mut Dom, parent: Option<usize>, local: &str) -> usize {
    let idx = dom.nodes.len();
    dom.nodes.push(NodeRec {
        kind: NodeKind::Element,
        name: Some(QName {
            prefix: None,
            local: local.into(),
            ns_uri: None,
        }),
        value: String::new(),
        parent,
        children: vec![],
    });
    if let Some(p) = parent {
        dom.nodes[p].children.push(idx);
    }
    idx
}

fn sample_tree() -> Node {
    // root
    // ├─ a (idx 1)
    // └─ a (idx 2)
    //    └─ c (idx 3)
    let mut d = Dom { nodes: vec![] };
    let root = el(&mut d, None, "root");
    let _a1 = el(&mut d, Some(root), "a");
    let a2 = el(&mut d, Some(root), "a");
    let _c = el(&mut d, Some(a2), "c");
    Node {
        dom: Arc::new(d),
        idx: root,
    }
}

fn as_bool<N>(items: &Vec<XdmItem<N>>) -> bool {
    assert_eq!(items.len(), 1, "expected single item result");
    match &items[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => *b,
        _ => panic!("expected boolean"),
    }
}

#[fixture]
#[allow(unused_braces)]
fn root() -> Node {
    sample_tree()
}
#[fixture]
#[allow(unused_braces)]
fn sc() -> StaticContext {
    StaticContext::default()
}

#[rstest]
fn node_is_true(root: Node, sc: StaticContext) {
    let exec = compile_xpath("a[1] is a[1]", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root)).unwrap();
    assert!(as_bool(&out));
}

#[rstest]
fn node_is_false(root: Node, sc: StaticContext) {
    let exec = compile_xpath("a[1] is a[2]", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root)).unwrap();
    assert!(!as_bool(&out));
}

#[rstest]
#[case::a_before_c("a[1] << //c")]
#[case::c_after_a("//c >> a[1]")]
fn node_before_after(#[case] expr: &str, root: Node, sc: StaticContext) {
    let exec = compile_xpath(expr, &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root)).unwrap();
    assert!(as_bool(&out));
}
