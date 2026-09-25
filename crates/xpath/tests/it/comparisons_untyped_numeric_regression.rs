use platynui_xpath::engine::runtime::DynamicContextBuilder;
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

fn expect_err(e: &str, code_frag: &str) {
    let c = ctx();
    let err = evaluate_expr::<N>(e, &c).unwrap_err();
    assert!(
        err.code_qname().unwrap().local.contains(code_frag),
        "expected fragment {code_frag}, got {:?}",
        err.code_qname()
    );
}

#[rstest]
fn value_eq_untyped_numeric() {
    assert!(bool_expr("xs:untypedAtomic('10') eq 10"));
}

#[rstest]
fn value_lt_untyped_numeric() {
    assert!(bool_expr("xs:untypedAtomic('2') lt 3"));
}

#[rstest]
fn invalid_untyped_numeric_error() {
    expect_err("xs:untypedAtomic('abc') eq 5", "FORG0001");
}
