use platynui_xpath::{evaluate_expr, runtime::DynamicContextBuilder, xdm::XdmAtomicValue as A, xdm::XdmItem as I};
use rstest::rstest;
type N = platynui_xpath::model::simple::SimpleNode;
fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

#[rstest]
#[case("1+2", 3)]
#[case("5-2", 3)]
#[case("2*3", 6)]
#[case("6 idiv 2", 3)]
#[case("10 mod 7", 3)]
fn arith_ints(#[case] expr: &str, #[case] expect: i64) {
    let out = evaluate_expr::<N>(expr, &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Integer(expect))]);
}

#[rstest]
#[case("true() and false()", false)]
#[case("true() or false()", true)]
#[case("not(false())", true)]
fn logic_and_or_not(#[case] expr: &str, #[case] expect: bool) {
    let out = evaluate_expr::<N>(expr, &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Boolean(expect))]);
}

#[rstest]
fn range_to() {
    let out = evaluate_expr::<N>("1 to 3", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Integer(1)), I::Atomic(A::Integer(2)), I::Atomic(A::Integer(3))]);
}
