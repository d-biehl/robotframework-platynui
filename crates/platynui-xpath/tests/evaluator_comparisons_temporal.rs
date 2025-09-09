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

fn as_bool<N>(items: &Vec<XdmItem<N>>) -> bool {
    match &items[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => *b,
        _ => panic!("boolean expected"),
    }
}

#[rstest]
fn value_eq_same_lexical() {
    let sc = StaticContext::default();
    let exec = compile_xpath(
        "('2024-01-02T03:00:00+00:00' cast as xs:dateTime) eq ('2024-01-02T03:00:00+00:00' cast as xs:dateTime)",
        &sc,
    ).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&Default::default()).unwrap();
    assert!(as_bool(&out));
}

#[rstest]
fn value_lt_datetime() {
    let sc = StaticContext::default();
    let exec = compile_xpath(
        "('2024-01-01T00:00:00+00:00' cast as xs:dateTime) lt ('2024-01-01T00:00:01+00:00' cast as xs:dateTime)",
        &sc,
    ).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&Default::default()).unwrap();
    assert!(as_bool(&out));
}
