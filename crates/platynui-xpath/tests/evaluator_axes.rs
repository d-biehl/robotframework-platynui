use platynui_xpath::compile_xpath;
use platynui_xpath::model::{NodeKind, QName, XdmNode};
use platynui_xpath::runtime::StaticContext;
use platynui_xpath::xdm::XdmItem;
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
    fn compare_document_order(&self, other: &Self) -> std::cmp::Ordering {
        self.idx.cmp(&other.idx)
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
fn tx(dom: &mut Dom, p: usize, v: &str) -> usize {
    let i = dom.nodes.len();
    dom.nodes.push(NodeRec {
        kind: NodeKind::Text,
        name: None,
        value: v.into(),
        parent: Some(p),
        children: vec![],
        attrs: vec![],
    });
    dom.nodes[p].children.push(i);
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
fn test_descendant_axis_excludes_self(root: Node, sc: StaticContext) {
    // a/descendant::a should select only descendant a (not the self a)
    let exec = compile_xpath("a/descendant::a", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root)).unwrap();
    assert_eq!(names(&out), vec!["a"]);
}

#[rstest]
fn test_descendant_axis_on_c(root: Node, sc: StaticContext) {
    let exec = compile_xpath("a/descendant::c", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root)).unwrap();
    assert_eq!(names(&out), vec!["c"]);
}

#[rstest]
fn test_parent_and_ancestors(root: Node, sc: StaticContext) {
    let exec = compile_xpath("//c/parent::a", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root.clone())).unwrap();
    assert_eq!(names(&out), vec!["a"]);

    let exec = compile_xpath("//c/ancestor::a", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root.clone())).unwrap();
    // there are two ancestor a nodes in sample()
    assert_eq!(names(&out), vec!["a", "a"]);

    let exec = compile_xpath("//c/ancestor-or-self::*", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root)).unwrap();
    // self c plus ancestors a, a, root => 4 nodes total
    assert_eq!(out.len(), 4);
}

#[rstest]
fn test_following_and_preceding(root: Node, sc: StaticContext) {
    // There is a 'd' element after 'a' under root; it is in the following axis of 'a'
    let exec = compile_xpath("a/following::d", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root.clone())).unwrap();
    assert_eq!(names(&out), vec!["d"]);

    // second 'a' (sibling under root) has a preceding-sibling 'a'
    let exec = compile_xpath("a[2]/preceding-sibling::a", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root)).unwrap();
    assert_eq!(names(&out), vec!["a"]);
}
