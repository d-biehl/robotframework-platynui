use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::{
    evaluate_expr,
    xdm::{XdmAtomicValue as A, XdmItem as I},
};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;
fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

fn eval_bool(expr: &str) -> bool {
    let out = evaluate_expr::<N>(expr, &ctx()).unwrap();
    match out.as_slice() {
        [I::Atomic(A::Boolean(b))] => *b,
        _ => panic!("expected boolean for {expr}"),
    }
}

// Deterministic coverage: boundary and representative pairs
#[rstest]
#[case(-1000, -1000)]
#[case(-1000, -999)]
#[case(-1, 0)]
#[case(0, 0)]
#[case(0, 1)]
#[case(42, 42)]
#[case(123, -123)]
#[case(999, 1000)]
fn integer_ordering_consistency(#[case] a: i64, #[case] b: i64) {
    if a < b {
        let lt = eval_bool(&format!("{a} lt {b}"));
        let ge = eval_bool(&format!("{a} ge {b}"));
        assert!(lt, "lt should be true for {a} < {b}");
        assert!(!ge, "ge should be false when {a} < {b}");
    } else if a > b {
        let gt = eval_bool(&format!("{a} gt {b}"));
        let le = eval_bool(&format!("{a} le {b}"));
        assert!(gt, "gt should be true for {a} > {b}");
        assert!(!le, "le should be false when {a} > {b}");
    } else {
        let eq = eval_bool(&format!("{a} eq {b}"));
        let ne = eval_bool(&format!("{a} ne {b}"));
        assert!(eq, "eq true for equality {a} == {b}");
        assert!(!ne, "ne false for equality {a} == {b}");
    }
}
