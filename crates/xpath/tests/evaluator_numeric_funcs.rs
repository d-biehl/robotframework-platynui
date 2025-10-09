use platynui_xpath::{
    evaluate_expr,
    runtime::DynamicContext,
    xdm::{XdmAtomicValue, XdmItem},
};
use rstest::rstest;

fn ctx() -> DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContext::default()
}

fn double(expr: &str) -> f64 {
    let seq = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx()).unwrap();
    if let Some(XdmItem::Atomic(XdmAtomicValue::Double(d))) = seq.first() { *d } else { panic!("expected double") }
}

#[rstest]
#[case("abs(-3.5)", 3.5)]
#[case("floor(3.9)", 3.0)]
#[case("ceiling(3.1)", 4.0)]
fn abs_floor_ceiling_basic(#[case] expr: &str, #[case] expected: f64) {
    assert_eq!(double(expr), expected);
}

#[rstest]
#[case("round(2.4)", 2.0)]
#[case("round(2.5)", 3.0)] // ties away from zero per f64::round
#[case("round(-2.5)", -3.0)]
fn round_standard(#[case] expr: &str, #[case] expected: f64) {
    assert_eq!(double(expr), expected);
}

#[rstest]
#[case("round-half-to-even(2.5)", 2.0)]
#[case("round-half-to-even(3.5)", 4.0)]
#[case("round-half-to-even(-2.5)", -2.0)]
#[case("round-half-to-even(-3.5)", -4.0)]
fn round_half_to_even_ties(#[case] expr: &str, #[case] expected: f64) {
    assert_eq!(double(expr), expected);
}

#[rstest]
#[case("round(2.345, 2)", 2.35)]
#[case("round(2.344, 2)", 2.34)]
#[case("round(-2.345, 2)", -2.35)]
fn round_precision_positive(#[case] expr: &str, #[case] expected: f64) {
    assert_eq!(double(expr), expected);
}

#[rstest]
#[case("round(1234.0, -2)", 1200.0)]
#[case("round(1499.9, -2)", 1500.0)]
#[case("round(-1499.9, -2)", -1500.0)]
fn round_precision_negative(#[case] expr: &str, #[case] expected: f64) {
    assert_eq!(double(expr), expected);
}

#[rstest]
#[case("round-half-to-even(2.345, 2)", 2.34)]
#[case("round-half-to-even(2.355, 2)", 2.36)]
#[case("round-half-to-even(-2.355, 2)", -2.36)]
fn round_half_to_even_precision_positive(#[case] expr: &str, #[case] expected: f64) {
    assert_eq!(double(expr), expected);
}

#[rstest]
#[case("round-half-to-even(12350, -2)", 12400.0)]
#[case("round-half-to-even(12450, -2)", 12400.0)]
fn round_half_to_even_precision_negative(#[case] expr: &str, #[case] expected: f64) {
    assert_eq!(double(expr), expected);
}

#[rstest]
#[case("round-half-to-even(12340, -2)", 12300.0)]
#[case("round-half-to-even(12360, -2)", 12400.0)]
#[case("round-half-to-even(12550, -2)", 12600.0)]
#[case("round-half-to-even(-12550, -2)", -12600.0)]
fn round_half_to_even_precision_negative_mixed(#[case] expr: &str, #[case] expected: f64) {
    assert_eq!(double(expr), expected);
}
