use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::runtime::ErrorCode;
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmAtomicValue as A, xdm::XdmItem as I};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;
fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

fn bool_expr(e: &str) -> bool {
    let c = ctx();
    let r = evaluate_expr::<N>(e, &c).unwrap();
    if let I::Atomic(A::Boolean(b)) = &r[0] { *b } else { panic!("expected boolean") }
}

#[rstest]
fn string_numeric_general_comparison() {
    // General comparison should atomize and attempt numeric; incompatible leads to try string comparison then false
    assert!(!bool_expr("'abc' = 10"));
}

#[rstest]
fn string_numeric_value_comparison_type_error() {
    // Value comparison with disjoint types should raise a type error (XPTY0004 or similar)
    let c = ctx();
    let err = evaluate_expr::<N>("'abc' eq 10", &c).unwrap_err();
    // The suite expects a type error. Current engine uses XPTY0004.
    assert_eq!(err.code_enum(), ErrorCode::XPTY0004);
}
