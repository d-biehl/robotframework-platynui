use platynui_xpath::{
    evaluate_expr,
    runtime::DynamicContext,
    xdm::{XdmAtomicValue, XdmItem},
};
use rstest::rstest;

type C = DynamicContext<platynui_xpath::model::simple::SimpleNode>;
fn ctx() -> C {
    DynamicContext::default()
}

fn bool_of(expr: &str) -> bool {
    let seq = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx()).unwrap();
    if let Some(XdmItem::Atomic(XdmAtomicValue::Boolean(b))) = seq.first() { *b } else { panic!("expected boolean") }
}

fn assert_xpath_true(expr: &str) {
    assert!(bool_of(expr), "expected true for: {expr}");
}
fn assert_xpath_false(expr: &str) {
    assert!(!bool_of(expr), "expected false for: {expr}");
}

// Value comparisons across numeric types (eq)
#[rstest]
#[case::int_dec("1 eq 1.0", true)]
#[case::int_float("1 eq 1e0", true)]
#[case::dec_double("1.0 eq 1e0", true)]
#[case::float_double("1e0 eq 1E0", true)]
fn value_numeric_cross_type_eq(#[case] expr: &str, #[case] expected: bool) {
    if expected { assert_xpath_true(expr) } else { assert_xpath_false(expr) }
}

// Value comparisons lt/gt across cross types
#[rstest]
#[case::int_lt_dec("1 lt 1.1", true)]
#[case::dec_gt_int("1.1 gt 1", true)]
#[case::int_vs_double_arith("1 lt 1e0 + 0.1", true)]
#[case::double_vs_double_plus_dec("1e0 lt 1E0 + 1.0", true)]
fn value_numeric_cross_type_lt_gt(#[case] expr: &str, #[case] expected: bool) {
    if expected { assert_xpath_true(expr) } else { assert_xpath_false(expr) }
}

// General comparisons (=) across sequences
#[rstest]
#[case::any_match_1("(1,2) = (2.0,3.0)", true)]
#[case::any_match_2("(1,2) = (2e0,3)", true)]
#[case::no_match("(1,2) = (3.1,4.2)", false)]
fn general_numeric_sequence_any_match(#[case] expr: &str, #[case] expected: bool) {
    if expected { assert_xpath_true(expr) } else { assert_xpath_false(expr) }
}

// ne and promotion sanity
#[rstest]
#[case::ne_1("1 ne 2.0", true)]
#[case::ne_2("1.0 ne 2e0", true)]
#[case::ne_equal("1 ne 1.0", false)]
fn numeric_ne_and_promotion(#[case] expr: &str, #[case] expected: bool) {
    if expected { assert_xpath_true(expr) } else { assert_xpath_false(expr) }
}

// Edge cases: NaN, ±INF, signed zero
#[rstest]
#[case::nan_eq_nan("number('foo') eq number('bar')", false)]
#[case::nan_ne_nan("number('foo') ne number('bar')", true)]
#[case::nan_order_lt("number('foo') lt 0", false)]
#[case::nan_general_pair("(number('foo'), 1) = (1)", true)]
#[case::nan_general_nan("(number('foo')) = (number('foo'))", false)]
#[case::pos_inf_gt("1e0 div 0e0 gt 1e0", true)]
#[case::neg_inf_lt("(-1e0) div 0e0 lt -1e0", true)]
#[case::inf_eq_inf("1e0 div 0e0 eq 1e0 div 0e0", true)]
#[case::neg_inf_ne_inf("(-1e0) div 0e0 ne 1e0 div 0e0", true)]
#[case::signed_zero_eq("0e0 eq -0e0", true)]
#[case::signed_zero_lt("0e0 lt -0e0", false)]
#[case::signed_zero_gt("0e0 gt -0e0", false)]
fn numeric_edge_cases(#[case] expr: &str, #[case] expected: bool) {
    if expected { assert_xpath_true(expr) } else { assert_xpath_false(expr) }
}

// Value comparisons must error on non-singleton sequences (XPTY0004)
#[rstest]
#[case::lhs_multi("(1,2) eq 1")]
#[case::rhs_multi("1 eq (1,2)")]
#[case::both_multi("(1,2) le (1,2)")]
fn value_comparison_cardinality_errors(#[case] expr: &str) {
    let err = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx()).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("FORG0006"), "expected FORG0006, got: {msg}");
}
