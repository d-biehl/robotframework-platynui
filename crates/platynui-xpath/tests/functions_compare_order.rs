use platynui_xpath::compile_xpath;
use platynui_xpath::model::{NodeKind, QName, XdmNode};
use platynui_xpath::runtime::StaticContext;
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use rstest::rstest;

#[derive(Debug, Clone, PartialEq, Eq)]
struct DummyNode;
impl XdmNode for DummyNode {
    fn kind(&self) -> NodeKind {
        NodeKind::Text
    }
    fn name(&self) -> Option<QName> {
        None
    }
    fn string_value(&self) -> String {
        String::new()
    }
    fn parent(&self) -> Option<Self> {
        None
    }
    fn children(&self) -> Vec<Self> {
        vec![]
    }
    fn attributes(&self) -> Vec<Self> {
        vec![]
    }
}

fn as_int<N>(items: &Vec<XdmItem<N>>) -> i64 {
    match &items[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(i)) => *i,
        _ => panic!("int expected"),
    }
}

#[rstest]
fn compare_returns_order_codes() {
    let sc = StaticContext::default();
    let ex1 = compile_xpath("compare('a','b')", &sc).unwrap();
    let out1: Vec<XdmItem<DummyNode>> = ex1.evaluate(&Default::default()).unwrap();
    assert_eq!(as_int(&out1), -1);

    let ex2 = compile_xpath("compare('b','a')", &sc).unwrap();
    let out2: Vec<XdmItem<DummyNode>> = ex2.evaluate(&Default::default()).unwrap();
    assert_eq!(as_int(&out2), 1);
}
