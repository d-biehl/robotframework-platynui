use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::runtime::ErrorCode;
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmAtomicValue as A, xdm::XdmItem as I};
use rstest::rstest;
type N = platynui_xpath::model::simple::SimpleNode;
fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

#[rstest]
fn cast_integer() {
    let out = evaluate_expr::<N>("'42' cast as xs:integer", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Integer(42))]);
}

#[rstest]
fn castable_integer_true() {
    let out = evaluate_expr::<N>("'42' castable as xs:integer", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Boolean(true))]);
}

#[rstest]
fn castable_integer_false() {
    let out = evaluate_expr::<N>("'xx' castable as xs:integer", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Boolean(false))]);
}

#[rstest]
fn treat_as_string_sequence() {
    let out = evaluate_expr::<N>("('a','b') treat as xs:string*", &ctx()).unwrap();
    assert_eq!(out.len(), 2); // simplified treat always ok currently
}

#[rstest]
fn instance_of_always_true_placeholder() {
    let out = evaluate_expr::<N>("'abc' instance of xs:string", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Boolean(true))]);
}

#[rstest]
fn instance_of_false_mismatch() {
    let out = evaluate_expr::<N>("123 instance of xs:string", &ctx()).unwrap();
    // After refining atomic_matches_name, xs:string no longer matches integer.
    assert_eq!(out, vec![I::Atomic(A::Boolean(false))]);
}

#[rstest]
fn instance_of_cardinality_zero_or_one() {
    let out = evaluate_expr::<N>("('a') instance of xs:string?", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Boolean(true))]);
}

#[rstest]
fn instance_of_cardinality_fail() {
    let out = evaluate_expr::<N>("('a','b') instance of xs:string?", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Boolean(false))]);
}

#[rstest]
fn treat_cardinality_fail() {
    let err = evaluate_expr::<N>("('a','b') treat as xs:string?", &ctx()).expect_err("expected error");
    assert_eq!(err.code_enum(), ErrorCode::XPTY0004);
}

#[rstest]
fn treat_cardinality_ok() {
    let out = evaluate_expr::<N>("('a') treat as xs:string?", &ctx()).unwrap();
    assert_eq!(out.len(), 1);
}
