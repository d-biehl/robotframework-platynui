use platynui_xpath::compile_xpath;
use platynui_xpath::model::{NodeKind, QName, XdmNode};
use platynui_xpath::runtime::{Error, StaticContext};
use platynui_xpath::xdm::{ExpandedName, XdmItem};
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
        vec![]
    }
    fn compare_document_order(&self, other: &Self) -> Result<std::cmp::Ordering, Error> {
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
    });
    if let Some(pp) = p {
        dom.nodes[pp].children.push(i);
    }
    i
}

fn sample_pair() -> (Node, Node) {
    let mut d1 = Dom { nodes: vec![] };
    let r1 = el(&mut d1, None, "r1");
    let a1 = el(&mut d1, Some(r1), "a");
    let n1 = Node {
        dom: Arc::new(d1),
        idx: a1,
    };

    let mut d2 = Dom { nodes: vec![] };
    let r2 = el(&mut d2, None, "r2");
    let a2 = el(&mut d2, Some(r2), "a");
    let n2 = Node {
        dom: Arc::new(d2),
        idx: a2,
    };
    (n1, n2)
}

#[rstest]
fn multiroot_union_errors() {
    let (n1, n2) = sample_pair();
    let sc = StaticContext::default();
    let exec = compile_xpath("$n1 | $n2", &sc).unwrap();
    let ctx = platynui_xpath::runtime::DynamicContextBuilder::<Node>::new()
        .with_variable(
            ExpandedName {
                ns_uri: None,
                local: "n1".into(),
            },
            vec![XdmItem::Node(n1)],
        )
        .with_variable(
            ExpandedName {
                ns_uri: None,
                local: "n2".into(),
            },
            vec![XdmItem::Node(n2)],
        )
        .build();
    let err = exec.evaluate(&ctx).expect_err("expected multiroot error");
    assert_eq!(err.code, "err:FOER0000");
}

#[rstest]
fn multiroot_node_before_errors() {
    let (n1, n2) = sample_pair();
    let sc = StaticContext::default();
    let exec = compile_xpath("$n1 << $n2", &sc).unwrap();
    let ctx = platynui_xpath::runtime::DynamicContextBuilder::<Node>::new()
        .with_variable(
            ExpandedName {
                ns_uri: None,
                local: "n1".into(),
            },
            vec![XdmItem::Node(n1)],
        )
        .with_variable(
            ExpandedName {
                ns_uri: None,
                local: "n2".into(),
            },
            vec![XdmItem::Node(n2)],
        )
        .build();
    let err = exec.evaluate(&ctx).expect_err("expected multiroot error");
    assert_eq!(err.code, "err:FOER0000");
}
