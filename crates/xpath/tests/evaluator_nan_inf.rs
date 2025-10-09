use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::runtime::ErrorCode;
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmAtomicValue as A, xdm::XdmItem as I};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;
fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

fn bool(expr: &str) -> bool {
    let out = evaluate_expr::<N>(expr, &ctx()).unwrap();
    match out.as_slice() {
        [I::Atomic(A::Boolean(b))] => *b,
        _ => panic!("expected boolean: {expr}"),
    }
}

// Produce NaN/INF values using division by zero in floating point domain (double per exponent notation)
// XPath F&O: 0e0 div 0e0 => NaN, 1e0 div 0e0 => +INF, -1e0 div 0e0 => -INF
const NAN_EXPR: &str = "0e0 div 0e0"; // NaN
const INF_EXPR: &str = "1e0 div 0e0"; // +INF
const NEG_INF_EXPR: &str = "-1e0 div 0e0"; // -INF

#[rstest]
fn nan_eq_nan_false() {
    assert!(!bool(&format!("{NAN_EXPR} eq {NAN_EXPR}")));
}

#[rstest]
fn nan_ne_nan_true() {
    assert!(bool(&format!("{NAN_EXPR} ne {NAN_EXPR}")));
}

#[rstest]
fn nan_lt_number_false() {
    assert!(!bool(&format!("{NAN_EXPR} lt 1")));
}

#[rstest]
fn number_lt_nan_false() {
    assert!(!bool(&format!("1 lt {NAN_EXPR}")));
}

#[rstest]
fn nan_gt_number_false() {
    assert!(!bool(&format!("{NAN_EXPR} gt 1")));
}

#[rstest]
fn number_gt_nan_false() {
    assert!(!bool(&format!("1 gt {NAN_EXPR}")));
}

#[rstest]
fn inf_gt_number_true() {
    assert!(bool(&format!("{INF_EXPR} gt 1e0")));
}

#[rstest]
fn neg_inf_lt_number_true() {
    assert!(bool(&format!("{NEG_INF_EXPR} lt 1e0")));
}

#[rstest]
fn inf_eq_inf_true() {
    assert!(bool(&format!("{INF_EXPR} eq {INF_EXPR}")));
}

#[rstest]
fn neg_inf_eq_neg_inf_true() {
    assert!(bool(&format!("{NEG_INF_EXPR} eq {NEG_INF_EXPR}")));
}

#[rstest]
fn inf_ne_neg_inf_true() {
    assert!(bool(&format!("{INF_EXPR} ne {NEG_INF_EXPR}")));
}

#[rstest]
fn div_by_zero_error_integer() {
    // Integer division by zero should raise FOAR0001 (arithmetic error) not produce INF.
    let err = evaluate_expr::<N>("1 idiv 0", &ctx()).expect_err("expected error");
    assert!(err.code_enum() == ErrorCode::FOAR0001, "expected FOAR0001 got {err:?}");
}

#[rstest]
fn number_function_nan() {
    // number('abc') => NaN; test via not(number('abc') = number('abc'))
    assert!(bool("not(number('abc') = number('abc'))"));
}

#[rstest]
fn number_function_string_numeric() {
    assert!(bool("number('42') eq 42"));
}

#[rstest]
fn nan_general_comparison_false() {
    // General comparisons treat NaN pairs as incomparable => false unless some other comparable pair yields true.
    assert!(!bool(&format!("({NAN_EXPR}) = (1,2,3)")));
}
