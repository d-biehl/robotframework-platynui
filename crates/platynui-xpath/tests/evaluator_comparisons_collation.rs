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
        _ => panic!("bool expected"),
    }
}

#[rstest]
fn value_and_general_comparisons_use_default_collation() {
    let sc = StaticContext::default();
    // value comparison eq
    let ex_eq = compile_xpath("'A' eq 'a'", &sc).unwrap();
    // general comparison =
    let ex_gen = compile_xpath("'A' = 'a'", &sc).unwrap();

    // Without collation override → false
    let out_eq: Vec<XdmItem<DummyNode>> = ex_eq.evaluate(&Default::default()).unwrap();
    assert!(!as_bool(&out_eq));
    let out_gen: Vec<XdmItem<DummyNode>> = ex_gen.evaluate(&Default::default()).unwrap();
    assert!(!as_bool(&out_gen));

    // With simple-case collation as default → true
    let ctx = platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new()
        .with_default_collation("urn:platynui:collation:simple-case")
        .build();
    let out_eq: Vec<XdmItem<DummyNode>> = ex_eq.evaluate(&ctx).unwrap();
    assert!(as_bool(&out_eq));
    let out_gen: Vec<XdmItem<DummyNode>> = ex_gen.evaluate(&ctx).unwrap();
    assert!(as_bool(&out_gen));
}

#[rstest]
fn general_comparison_ignores_type_errors() {
    let sc = StaticContext::default();
    // Comparing strings to numbers via general '=' should not error; should just be false
    let ex = compile_xpath("('a','b') = (1,2)", &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = ex.evaluate(&Default::default()).unwrap();
    assert!(matches!(
        &out[0],
        XdmItem::Atomic(XdmAtomicValue::Boolean(false))
    ));
}
