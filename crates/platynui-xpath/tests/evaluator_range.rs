use rstest::rstest;
use platynui_xpath::compile_xpath;
use platynui_xpath::runtime::StaticContext;
use platynui_xpath::xdm::{XdmItem, XdmAtomicValue};
use platynui_xpath::model::{XdmNode, NodeKind, QName};

#[derive(Debug, Clone, PartialEq, Eq)]
struct DummyNode;

impl XdmNode for DummyNode {
    fn kind(&self) -> NodeKind { NodeKind::Text }
    fn name(&self) -> Option<QName> { None }
    fn string_value(&self) -> String { String::new() }
    fn parent(&self) -> Option<Self> { None }
    fn children(&self) -> Vec<Self> { vec![] }
    fn attributes(&self) -> Vec<Self> { vec![] }
    fn compare_document_order(&self, _other: &Self) -> std::cmp::Ordering { std::cmp::Ordering::Equal }
}

fn atomics<N>(items: &Vec<XdmItem<N>>) -> Vec<XdmAtomicValue> {
    items.iter().filter_map(|it| if let XdmItem::Atomic(a) = it { Some(a.clone()) } else { None }).collect()
}

#[rstest]
fn test_range_to_operator_simple() {
    let exec = compile_xpath("1 to 3", &StaticContext::default()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build()).unwrap();
    let vals = atomics(&out);
    assert_eq!(vals, vec![XdmAtomicValue::Integer(1), XdmAtomicValue::Integer(2), XdmAtomicValue::Integer(3)]);
}

