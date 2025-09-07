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
    fn compare_document_order(&self, _other: &Self) -> std::cmp::Ordering {
        std::cmp::Ordering::Equal
    }
}

fn as_bool<N>(items: &Vec<XdmItem<N>>) -> bool {
    assert_eq!(items.len(), 1, "expected single item");
    match &items[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => *b,
        _ => panic!("not boolean"),
    }
}

#[rstest]
#[case("true()", true)]
#[case("false()", false)]
fn fn_true_false(#[case] expr: &str, #[case] expected: bool) {
    let sc = StaticContext::default();
    let ctx = platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build();
    let exec = compile_xpath(expr, &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx).unwrap();
    assert_eq!(as_bool(&out), expected, "expr: {}", expr);
}

#[rstest]
#[case("not(true())", false)]
#[case("not('')", true)]
fn fn_not(#[case] expr: &str, #[case] expected: bool) {
    let sc = StaticContext::default();
    let ctx = platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build();
    let exec = compile_xpath(expr, &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx).unwrap();
    assert_eq!(as_bool(&out), expected);
}
