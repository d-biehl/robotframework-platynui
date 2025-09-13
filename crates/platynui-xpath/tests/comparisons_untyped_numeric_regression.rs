use platynui_xpath::runtime::DynamicContextBuilder;
use platynui_xpath::{SimpleNode, XdmItem as I, evaluate_expr, xdm::XdmAtomicValue as A};
use rstest::rstest;

type N = SimpleNode;
fn ctx() -> platynui_xpath::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

fn bool_expr(e: &str) -> bool {
    let c = ctx();
    let r = evaluate_expr::<N>(e, &c).unwrap();
    if let I::Atomic(A::Boolean(b)) = &r[0] {
        *b
    } else {
        panic!("expected boolean")
    }
}

fn expect_err(e: &str, code_frag: &str) {
    let c = ctx();
    let err = evaluate_expr::<N>(e, &c).unwrap_err();
    assert!(
        err.code.contains(code_frag),
        "expected code fragment {code_frag} in {} got {}",
        e,
        err.code
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
