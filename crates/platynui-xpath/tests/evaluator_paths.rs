use platynui_xpath::compile_xpath;
use platynui_xpath::model::{NodeKind, QName, XdmNode};
use platynui_xpath::runtime::StaticContext;
use rstest::{fixture, rstest};
use std::sync::Arc;

#[derive(Debug, Clone)]
struct Dom {
    nodes: Vec<NodeRecord>,
}

#[derive(Debug, Clone)]
struct NodeRecord {
    kind: NodeKind,
    name: Option<QName>,
    value: String,
    parent: Option<usize>,
    children: Vec<usize>,
    attributes: Vec<usize>,
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
            .attributes
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

fn el(dom: &mut Dom, parent: Option<usize>, local: &str) -> usize {
    let idx = dom.nodes.len();
    dom.nodes.push(NodeRecord {
        kind: NodeKind::Element,
        name: Some(QName {
            prefix: None,
            local: local.into(),
            ns_uri: None,
        }),
        value: String::new(),
        parent,
        children: vec![],
        attributes: vec![],
    });
    if let Some(p) = parent {
        dom.nodes[p].children.push(idx);
    }
    idx
}

fn tx(dom: &mut Dom, parent: usize, value: &str) -> usize {
    let idx = dom.nodes.len();
    dom.nodes.push(NodeRecord {
        kind: NodeKind::Text,
        name: None,
        value: value.into(),
        parent: Some(parent),
        children: vec![],
        attributes: vec![],
    });
    dom.nodes[parent].children.push(idx);
    idx
}

fn at(dom: &mut Dom, parent: usize, local: &str, value: &str) -> usize {
    let idx = dom.nodes.len();
    dom.nodes.push(NodeRecord {
        kind: NodeKind::Attribute,
        name: Some(QName {
            prefix: None,
            local: local.into(),
            ns_uri: None,
        }),
        value: value.into(),
        parent: Some(parent),
        children: vec![],
        attributes: vec![],
    });
    dom.nodes[parent].attributes.push(idx);
    idx
}

fn sample_tree() -> Node {
    let mut dom = Dom { nodes: vec![] };
    let root = el(&mut dom, None, "root");
    let a1 = el(&mut dom, Some(root), "a");
    at(&mut dom, a1, "id", "x");
    let _b1 = el(&mut dom, Some(a1), "b");
    let a2 = el(&mut dom, Some(root), "a");
    at(&mut dom, a2, "id", "y");
    let c1 = el(&mut dom, Some(a2), "c");
    tx(&mut dom, c1, "hello");
    Node {
        dom: Arc::new(dom),
        idx: root,
    }
}

fn names(items: &Vec<platynui_xpath::xdm::XdmItem<Node>>) -> Vec<String> {
    use platynui_xpath::xdm::XdmItem;
    let mut out = vec![];
    for it in items {
        if let XdmItem::Node(n) = it {
            if let Some(q) = n.name() {
                out.push(q.local);
            }
        }
    }
    out
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
#[case::select_a("a", vec!["a","a"]) ]
#[case::select_ab("a/b", vec!["b"]) ]
#[case::select_attr_id("a/@id", vec!["id","id"]) ]
#[case::select_desc_c("//c", vec!["c"]) ]
fn test_basic_paths(
    #[case] expr: &str,
    #[case] expected: Vec<&str>,
    root: Node,
    sc: StaticContext,
) {
    let exec = compile_xpath(expr, &sc).expect("compile");
    let res = exec.evaluate_on(Some(root)).expect("eval");
    let got = names(&res);
    let expected: Vec<String> = expected.into_iter().map(|s| s.to_string()).collect();
    assert_eq!(got, expected);
}

#[rstest]
fn test_predicates_indexing_and_value(root: Node, sc: StaticContext) {
    let exec = compile_xpath("a[2]", &sc).expect("compile");
    let res = exec.evaluate_on(Some(root.clone())).expect("eval");
    let got = names(&res);
    assert_eq!(got, vec!["a"]);
}

#[rstest]
fn test_predicate_attribute_equality(root: Node, sc: StaticContext) {
    let exec = compile_xpath("a[@id = 'x']", &sc).expect("compile");
    let res = exec.evaluate_on(Some(root)).expect("eval");
    let got = names(&res);
    assert_eq!(got, vec!["a"]);
}
