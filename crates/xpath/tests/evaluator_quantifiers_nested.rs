use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmAtomicValue as A, xdm::XdmItem as I};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;
fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

// Nested quantifier tests verifying no stack overflow and correct semantics.
// XPath examples:
// 1. some $x in (1,2) satisfies every $y in (1,2) satisfies $x <= $y * 2  -> true (1 <= 2*2 and 2 <= 1*2? second inner for x=2: need y sequence (1,2): 2 <= 1*2 (2) true; 2 <= 2*2 (4) true)
// 2. every $x in (1,2) satisfies some $y in (1) satisfies $y = $x         -> false (x=2 fails)
// 3. some $x in (1,2,3) satisfies some $y in (10,20) satisfies $x*10 = $y -> true (x=1,y=10)

#[rstest]
fn nested_some_every_true() {
    let out =
        evaluate_expr::<N>("some $x in (1,2) satisfies every $y in (1,2) satisfies $x <= $y * 2", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Boolean(true))]);
}

#[rstest]
fn nested_every_some_false() {
    let out = evaluate_expr::<N>("every $x in (1,2) satisfies some $y in (1) satisfies $y = $x", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Boolean(false))]);
}

#[rstest]
fn nested_some_some_true() {
    let out =
        evaluate_expr::<N>("some $x in (1,2,3) satisfies some $y in (10,20) satisfies $x * 10 = $y", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Boolean(true))]);
}
