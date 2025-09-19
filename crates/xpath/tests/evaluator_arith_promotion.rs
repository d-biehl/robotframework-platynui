use platynui_xpath::{
    SimpleNode, evaluate_expr,
    runtime::DynamicContext,
    xdm::{XdmAtomicValue, XdmItem},
};
use rstest::rstest;

fn eval(expr: &str) -> XdmAtomicValue {
    let ctx: DynamicContext<SimpleNode> = DynamicContext::default();
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
