use platynui_xpath::{evaluate_expr, XdmItem as I, xdm::XdmAtomicValue as A, SimpleNode};
use platynui_xpath::runtime::DynamicContextBuilder;
use rstest::rstest;
type N = SimpleNode;
fn ctx() -> platynui_xpath::runtime::DynamicContext<N> { DynamicContextBuilder::default().build() }

#[rstest]
fn union_basic() {
    let out = evaluate_expr::<N>("(1,2) union (2,3)", &ctx()).unwrap();
    // Simplified set semantics: order is first sequence order then new elements
    assert_eq!(out, vec![I::Atomic(A::Integer(1)), I::Atomic(A::Integer(2)), I::Atomic(A::Integer(3))]);
}

#[rstest]
fn intersect_basic() {
    let out = evaluate_expr::<N>("(1,2,3) intersect (2,4)", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Integer(2))]);
}

#[rstest]
fn except_basic() {
    let out = evaluate_expr::<N>("(1,2,3) except (2)", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Integer(1)), I::Atomic(A::Integer(3))]);
}
