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

fn as_string<N>(items: &Vec<XdmItem<N>>) -> String {
    match &items[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => s.clone(),
        _ => panic!("string expected"),
    }
}

#[rstest]
fn substring_out_of_range_semantics() {
    let sc = StaticContext::default();
    // start < 1 behaves as 1; length respected and floored
    let ex = compile_xpath("substring('abcdef', 0, 2)", &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = ex.evaluate(&Default::default()).unwrap();
    assert_eq!(as_string(&out), "ab");

    // negative length -> empty
    let ex = compile_xpath("substring('abcdef', 2, -1)", &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = ex.evaluate(&Default::default()).unwrap();
    assert_eq!(as_string(&out), "");
}
