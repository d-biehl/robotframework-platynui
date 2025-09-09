use platynui_xpath::compile_xpath;
use platynui_xpath::model::{NodeKind, QName, XdmNode};
use platynui_xpath::runtime::StaticContext;
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use rstest::rstest;
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
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.dom, &other.dom) && self.idx == other.idx
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
        vec![]
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
    });
    if let Some(pp) = p {
        dom.nodes[pp].children.push(i);
    }
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
    });
    dom.nodes[p].children.push(i);
    i
}

fn sample() -> Node {
    let mut d = Dom { nodes: vec![] };
    let r = el(&mut d, None, "root");
    let e = el(&mut d, Some(r), "e");
    tx(&mut d, e, "alpha");
    Node {
        dom: Arc::new(d),
        idx: e,
    }
}

#[rstest]
fn node_atomization_in_general_comparison() {
    let sc = StaticContext::default();
    let ex = compile_xpath("text() = 'alpha'", &sc).unwrap();
    let ctx = platynui_xpath::runtime::DynamicContextBuilder::new()
        .with_context_item(XdmItem::Node(sample()))
        .build();
    let out: Vec<XdmItem<Node>> = ex.evaluate(&ctx).unwrap();
    match &out[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("bool"),
    }
}
