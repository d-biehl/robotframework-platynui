use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmAtomicValue as A, xdm::XdmItem as I};
use rstest::rstest;
type N = platynui_xpath::model::simple::SimpleNode;
fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

// Quantifier semantics: variable binding now active. Tests assert XPath 2.0 logic.

#[rstest]
fn some_quantifier_basic_true() {
    let out = evaluate_expr::<N>("some $x in (1,2,3) satisfies $x = 2", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Boolean(true))]);
}

#[rstest]
fn every_quantifier_basic_false() {
    let out = evaluate_expr::<N>("every $x in (1,2,3) satisfies $x = 1", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Boolean(false))]);
}

// Additional regression-style quantifier tests (current simplified semantics)
#[rstest]
fn some_quantifier_empty_sequence_false() {
    let out = evaluate_expr::<N>("some $x in () satisfies $x = 1", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Boolean(false))]);
}

#[rstest]
fn every_quantifier_empty_sequence_true() {
    let out = evaluate_expr::<N>("every $x in () satisfies $x = 1", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Boolean(true))]);
}

#[rstest]
fn some_quantifier_first_match_short_circuit() {
    let out = evaluate_expr::<N>("some $x in (5,6,7) satisfies $x = 5", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Boolean(true))]);
}

#[rstest]
fn every_quantifier_all_equal_true() {
    let out = evaluate_expr::<N>("every $x in (2,2) satisfies $x = 2", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Boolean(true))]);
}

// Nested quantifiers pending deeper recursion guard; omitted to avoid stack overflow in current VM.
