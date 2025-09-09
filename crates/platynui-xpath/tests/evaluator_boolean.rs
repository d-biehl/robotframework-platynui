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
        _ => panic!("expected boolean"),
    }
}

#[rstest]
fn and_or_with_numeric_and_string_ebv() {
    let sc = StaticContext::default();
    // numeric EBV: non-zero => true
    let exec = compile_xpath("(1 and 2) and ('x' and 'y')", &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&Default::default()).unwrap();
    assert!(as_bool(&out));

    // empty string is false
    let exec = compile_xpath("'a' and ''", &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&Default::default()).unwrap();
    assert!(!as_bool(&out));
}

// note: EBV error semantics for multi-item sequences are covered via other tests
