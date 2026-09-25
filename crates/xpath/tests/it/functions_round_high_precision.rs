use platynui_xpath::{
    evaluator::evaluate_expr,
    runtime::DynamicContextBuilder,
    xdm::{XdmAtomicValue, XdmItem},
};
use rstest::rstest;

fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContextBuilder::new().build()
}

fn dbl(expr: &str) -> f64 {
    let seq = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx()).unwrap();
    if let Some(XdmItem::Atomic(XdmAtomicValue::Double(d))) = seq.first() { *d } else { f64::NAN }
}

// Exact equality intended: the rounded result must match the literal bit for bit.
#[allow(clippy::float_cmp)]
#[track_caller]
fn assert_exact_f64(actual: f64, expected: f64) {
    assert_eq!(actual, expected);
}

#[rstest]
#[case("round(1.234567890123456, 16)", 1.234_567_890_123_456_f64)] // precision > 15 -> unchanged
#[case("round-half-to-even(1.234567890123456, 16)", 1.234_567_890_123_456_f64)]
#[case("round-half-to-even(2.3449999999, 2)", 2.34)] // banker epsilon boundary test
fn round_high_precision_cases(#[case] expr: &str, #[case] expected: f64) {
    let v = dbl(expr);
    assert_exact_f64(v, expected);
}
