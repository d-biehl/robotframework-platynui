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
    let r = el(&mut d, None, "root");
    let a = el(&mut d, Some(r), "a");
    at(&mut d, a, "id", "x");
    Node {
        dom: Arc::new(d),
        idx: r,
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

#[rstest]
fn namespace_axis_on_attribute_is_empty(root: Node, sc: StaticContext) {
    let ex = compile_xpath("//@id/namespace::*", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = ex.evaluate_on(Some(root)).unwrap();
    assert!(out.is_empty());
}
