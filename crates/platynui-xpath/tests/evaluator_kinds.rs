use rstest::{rstest, fixture};
use std::sync::Arc;
use platynui_xpath::compile_xpath;
use platynui_xpath::model::{XdmNode, NodeKind, QName};
use platynui_xpath::runtime::StaticContext;
use platynui_xpath::xdm::XdmItem;

#[derive(Debug, Clone)]
struct Dom { nodes: Vec<NodeRec> }

#[derive(Debug, Clone)]
struct NodeRec { kind: NodeKind, name: Option<QName>, value: String, parent: Option<usize>, children: Vec<usize>, attrs: Vec<usize> }

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
    fn compare_document_order(&self, other: &Self) -> std::cmp::Ordering { self.idx.cmp(&other.idx) }
}

fn doc(dom: &mut Dom) -> usize { let i=dom.nodes.len(); dom.nodes.push(NodeRec { kind: NodeKind::Document, name: None, value:String::new(), parent: None, children: vec![], attrs: vec![]}); i }
fn el(dom: &mut Dom, p: usize, local: &str) -> usize { let i=dom.nodes.len(); dom.nodes.push(NodeRec { kind: NodeKind::Element, name: Some(QName{prefix:None, local:local.into(), ns_uri:None}), value:String::new(), parent: Some(p), children: vec![], attrs: vec![]}); dom.nodes[p].children.push(i); i }
fn tx(dom: &mut Dom, p: usize, v: &str) -> usize { let i=dom.nodes.len(); dom.nodes.push(NodeRec { kind: NodeKind::Text, name: None, value: v.into(), parent: Some(p), children: vec![], attrs: vec![]}); dom.nodes[p].children.push(i); i }
fn cm(dom: &mut Dom, p: usize, v: &str) -> usize { let i=dom.nodes.len(); dom.nodes.push(NodeRec { kind: NodeKind::Comment, name: None, value: v.into(), parent: Some(p), children: vec![], attrs: vec![]}); dom.nodes[p].children.push(i); i }
fn pi(dom: &mut Dom, p: usize, target: &str, v: &str) -> usize { let i=dom.nodes.len(); dom.nodes.push(NodeRec { kind: NodeKind::ProcessingInstruction, name: Some(QName{prefix:None, local:target.into(), ns_uri: None}), value: v.into(), parent: Some(p), children: vec![], attrs: vec![]}); dom.nodes[p].children.push(i); i }

fn sample_document() -> Node {
    let mut d = Dom { nodes: vec![] };
    let d0 = doc(&mut d);
    let r = el(&mut d, d0, "root");
    cm(&mut d, r, "c1");
    pi(&mut d, r, "go", "run");
    tx(&mut d, r, "text");
    Node { dom: Arc::new(d), idx: r } // context item as the document element
}

fn kind_names(items: &Vec<XdmItem<Node>>) -> Vec<NodeKind> { items.iter().filter_map(|it| if let XdmItem::Node(n)=it { Some(n.kind()) } else { None }).collect() }

// --- Fixtures ---
#[fixture]
fn root_el() -> Node { sample_document() }

#[fixture]
fn sc() -> StaticContext { StaticContext::default() }

#[rstest]
fn kind_comment(root_el: Node, sc: StaticContext) {
    let exec = compile_xpath("comment()", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root_el)).unwrap();
    assert_eq!(kind_names(&out), vec![NodeKind::Comment]);
}

#[rstest]
#[case("processing-instruction()", NodeKind::ProcessingInstruction)]
#[case("processing-instruction('go')", NodeKind::ProcessingInstruction)]
fn kind_processing_instruction(#[case] expr: &str, #[case] expected: NodeKind, root_el: Node, sc: StaticContext) {
    let exec = compile_xpath(expr, &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root_el)).unwrap();
    assert_eq!(kind_names(&out), vec![expected]);
}

#[rstest]
fn absolute_slash_returns_document(root_el: Node, sc: StaticContext) {
    let exec = compile_xpath("/", &sc).unwrap();
    let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root_el)).unwrap();
    assert_eq!(kind_names(&out), vec![NodeKind::Document]);
}
