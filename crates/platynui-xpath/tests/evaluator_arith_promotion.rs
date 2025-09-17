use platynui_xpath::{
    evaluate_expr,
    runtime::DynamicContext,
    xdm::{XdmAtomicValue, XdmItem},
};
use rstest::rstest;

#[derive(Clone, Debug, PartialEq, Eq)]
struct DummyNode;
impl platynui_xpath::model::XdmNode for DummyNode {
    type Children<'a> = std::iter::Empty<Self> where Self: 'a;
    type Attributes<'a> = std::iter::Empty<Self> where Self: 'a;
    type Namespaces<'a> = std::iter::Empty<Self> where Self: 'a;

    fn kind(&self) -> platynui_xpath::model::NodeKind {
        platynui_xpath::model::NodeKind::Document
    }
    fn string_value(&self) -> String {
        String::new()
    }
    fn parent(&self) -> Option<Self> {
        None
    }
    fn children(&self) -> Self::Children<'_> {
        std::iter::empty()
    }
    fn attributes(&self) -> Self::Attributes<'_> {
        std::iter::empty()
    }
    fn namespaces(&self) -> Self::Namespaces<'_> {
        std::iter::empty()
    }
    fn compare_document_order(
        &self,
        _other: &Self,
    ) -> Result<std::cmp::Ordering, platynui_xpath::engine::runtime::Error> {
        Ok(std::cmp::Ordering::Equal)
    }
    fn name(&self) -> Option<platynui_xpath::QName> {
        None
    }
}

fn eval(expr: &str) -> XdmAtomicValue {
    let ctx: DynamicContext<DummyNode> = DynamicContext::default();
    let seq = evaluate_expr(expr, &ctx).expect("eval");
    match seq.first() {
        Some(XdmItem::Atomic(a)) => a.clone(),
        _ => panic!("expected atomic"),
    }
}

#[rstest]
#[case("1 + 2", XdmAtomicValue::Integer(3))]
#[case("1 + 2.5", XdmAtomicValue::Decimal(3.5))]
#[case("2.5 + 3.0", XdmAtomicValue::Decimal(5.5))]
#[case("3.0 + 4.0", XdmAtomicValue::Decimal(7.0))]
#[case("5.0 div 2", XdmAtomicValue::Decimal(2.5))]
#[case("5 idiv 2", XdmAtomicValue::Integer(2))]
#[case("10.0 mod 4", XdmAtomicValue::Decimal(2.0))]
#[case("1.5 + 1.5", XdmAtomicValue::Decimal(3.0))]
#[case("1.5 + 1.5 - 1.0", XdmAtomicValue::Decimal(2.0))]
fn arithmetic_promotion_cases(#[case] expr: &str, #[case] expected: XdmAtomicValue) {
    let got = eval(expr);
    match (got, expected) {
        (XdmAtomicValue::Integer(a), XdmAtomicValue::Integer(b)) => assert_eq!(a, b),
        (XdmAtomicValue::Decimal(a), XdmAtomicValue::Decimal(b)) => assert!((a - b).abs() < 1e-9),
        (XdmAtomicValue::Double(a), XdmAtomicValue::Double(b)) => assert!((a - b).abs() < 1e-12),
        (other, exp) => panic!("type mismatch: got {:?}, expected {:?}", other, exp),
    }
}
