use rstest::{rstest, fixture};
use std::sync::Arc;
use platynui_xpath::compile_xpath;
use platynui_xpath::model::{XdmNode, NodeKind, QName};
use platynui_xpath::runtime::{StaticContext, DynamicContextBuilder};
use platynui_xpath::xdm::{XdmItem, XdmAtomicValue, ExpandedName};

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

fn el(dom: &mut Dom, p: Option<usize>, local: &str) -> usize { let i = dom.nodes.len(); dom.nodes.push(NodeRec { kind: NodeKind::Element, name: Some(QName{prefix:None, local:local.into(), ns_uri:None}), value:String::new(), parent:p, children:vec![], attrs:vec![]}); if let Some(pp)=p{dom.nodes[pp].children.push(i);} i }
fn at(dom: &mut Dom, p: usize, local: &str, v: &str) -> usize { let i=dom.nodes.len(); dom.nodes.push(NodeRec{kind:NodeKind::Attribute, name:Some(QName{prefix:None, local:local.into(), ns_uri:None}), value:v.into(), parent:Some(p), children:vec![], attrs:vec![]}); dom.nodes[p].attrs.push(i); i }
fn sample() -> Node { let mut d=Dom{nodes:vec![]}; let r=el(&mut d,None,"root"); let a1=el(&mut d,Some(r),"a"); at(&mut d,a1,"id","x"); let _a2=el(&mut d,Some(r),"a"); Node{dom:Arc::new(d), idx:r} }

#[fixture]
fn root() -> Node { sample() }
#[fixture]
fn sc() -> StaticContext { StaticContext::default() }

fn get_num<N: core::fmt::Debug>(items: &Vec<XdmItem<N>>) -> f64 {
    match &items[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(i)) => *i as f64,
        XdmItem::Atomic(XdmAtomicValue::Double(d)) => *d,
        other => panic!("expected number, got {:?}", other),
    }
}

#[rstest]
fn variable_arithmetic(root: Node, sc: StaticContext) {
    // $x + 2 where $x := 3
    let exec = compile_xpath("$x + 2", &sc).unwrap();
    let ctx = DynamicContextBuilder::new()
        .with_context_item(XdmItem::Node(root))
        .with_variable(ExpandedName::new(None, "x"), vec![XdmItem::Atomic(XdmAtomicValue::Integer(3))])
        .build();
    let out: Vec<XdmItem<Node>> = exec.evaluate(&ctx).unwrap();
    assert_eq!(get_num(&out), 5.0);
}

#[rstest]
fn variable_in_predicate(root: Node, sc: StaticContext) {
    // a[@id = $id] where $id := 'x'
    let exec = compile_xpath("a[@id = $id]", &sc).unwrap();
    let ctx = DynamicContextBuilder::new()
        .with_context_item(XdmItem::Node(root))
        .with_variable(ExpandedName::new(None, "id"), vec![XdmItem::Atomic(XdmAtomicValue::String("x".into()))])
        .build();
    let out: Vec<XdmItem<Node>> = exec.evaluate(&ctx).unwrap();
    assert_eq!(out.len(), 1);
}
