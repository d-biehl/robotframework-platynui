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

#[rstest]
fn numeric_value_comparisons_with_promotion() {
    let mut sc = StaticContext::default();
    sc.namespaces
        .by_prefix
        .insert("xs".into(), "http://www.w3.org/2001/XMLSchema".into());

    // integer vs double
    let ex = compile_xpath("1 eq 1.0", &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = ex.evaluate(&Default::default()).unwrap();
    match &out[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("bool"),
    }

    // decimal vs integer
    let ex = compile_xpath("('1' cast as xs:decimal) eq 1", &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = ex.evaluate(&Default::default()).unwrap();
    match &out[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("bool"),
    }
}

#[rstest]
fn general_comparison_with_sequence_semantics() {
    let sc = StaticContext::default();
    // ('a','b') = 'z' → false; no error
    let ex = compile_xpath("('a','b') = 'z'", &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = ex.evaluate(&Default::default()).unwrap();
    match &out[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => assert!(!*b),
        _ => panic!("bool"),
    }
}
