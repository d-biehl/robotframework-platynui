use platynui_xpath::{
    evaluator::evaluate_expr,
    runtime::DynamicContextBuilder,
    xdm::{XdmAtomicValue, XdmItem},
};
use rstest::rstest;

fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContextBuilder::new().build()
}

fn eval(expr: &str) -> Vec<XdmItem<platynui_xpath::model::simple::SimpleNode>> {
    evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx()).unwrap()
}

#[rstest]
#[case("avg((1, 2.0, 3))", 2.0)]
#[case("avg((1.5, 2.5))", 2.0)]
fn avg_mixed_cases(#[case] expr: &str, #[case] expected: f64) {
    let r = eval(expr);
    let v = &r[0];
    let got = match v {
        XdmItem::Atomic(XdmAtomicValue::Double(d)) => *d,
        XdmItem::Atomic(XdmAtomicValue::Float(f)) => *f as f64,
        XdmItem::Atomic(XdmAtomicValue::Decimal(dd)) => *dd,
        XdmItem::Atomic(XdmAtomicValue::Integer(i)) => *i as f64,
        _ => panic!("expected numeric"),
    };
    assert!((got - expected).abs() < 1e-9, "avg mismatch got {got} expected {expected}");
}
