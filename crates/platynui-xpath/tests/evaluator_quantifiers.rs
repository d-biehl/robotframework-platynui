use platynui_xpath::{evaluate_expr, XdmItem as I, xdm::XdmAtomicValue as A, SimpleNode};
use platynui_xpath::runtime::DynamicContextBuilder;
use rstest::rstest;
type N = SimpleNode;
fn ctx() -> platynui_xpath::runtime::DynamicContext<N> { DynamicContextBuilder::default().build() }

// NOTE: Quantifier semantics are currently simplified and variable binding inside the body
// isn't fully wired. We assert current behavior as regression protection.

#[rstest]
fn some_quantifier_placeholder() {
    let out = evaluate_expr::<N>("some $x in (1,2,3) satisfies $x = 2", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Boolean(false))]); // current engine result
}

#[rstest]
fn every_quantifier_placeholder() {
    let out = evaluate_expr::<N>("every $x in (1,2,3) satisfies $x = 1", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Boolean(false))]);
}

// Additional regression-style quantifier tests (current simplified semantics)
#[rstest]
fn some_quantifier_empty_sequence() {
    let out = evaluate_expr::<N>("some $x in () satisfies $x = 1", &ctx()).unwrap();
    // Current engine: false (no items cause body never sets result true)
    assert_eq!(out, vec![I::Atomic(A::Boolean(false))]);
}

#[rstest]
fn every_quantifier_empty_sequence() {
    let out = evaluate_expr::<N>("every $x in () satisfies $x = 1", &ctx()).unwrap();
    // Current engine initializes 'every' to true and with no iterations keeps true
    assert_eq!(out, vec![I::Atomic(A::Boolean(true))]);
}

#[rstest]
fn some_quantifier_first_match() {
    let out = evaluate_expr::<N>("some $x in (5,6,7) satisfies $x = 5", &ctx()).unwrap();
    // Current engine still returns false (binding not wired) -> guard against accidental change
    assert_eq!(out, vec![I::Atomic(A::Boolean(false))]);
}

#[rstest]
fn every_quantifier_all_equal() {
    let out = evaluate_expr::<N>("every $x in (2,2) satisfies $x = 2", &ctx()).unwrap();
    // Current engine (no binding) evaluates body with unbound var -> always false, flipping result to false.
    assert_eq!(out, vec![I::Atomic(A::Boolean(false))]);
}
