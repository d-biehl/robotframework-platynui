use platynui_xpath::{
    evaluator::evaluate_expr,
    runtime::DynamicContextBuilder,
    simple_node::SimpleNode,
    xdm::{XdmAtomicValue, XdmItem},
};
use rstest::rstest;

fn ctx() -> platynui_xpath::runtime::DynamicContext<SimpleNode> {
    DynamicContextBuilder::new().build()
}

fn double(expr: &str) -> f64 {
    let seq = evaluate_expr::<SimpleNode>(expr, &ctx()).unwrap();
    if let Some(XdmItem::Atomic(XdmAtomicValue::Double(d))) = seq.first() {
        *d
    } else {
        f64::NAN
    }
}

#[rstest]
#[case("sum((number('abc'), 5))")]
#[case("avg((number('abc'), 5))")]
fn nan_propagation_cases(#[case] expr: &str) {
    let v = double(expr);
    assert!(v.is_nan());
}
