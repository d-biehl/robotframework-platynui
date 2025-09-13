use platynui_xpath::runtime::DynamicContextBuilder;
use platynui_xpath::{
    SimpleNode, evaluate_expr,
    xdm::{XdmAtomicValue as A, XdmItem as I},
};
use proptest::prelude::*;

type N = SimpleNode;
fn ctx() -> platynui_xpath::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

fn eval_bool(expr: &str) -> bool {
    let out = evaluate_expr::<N>(expr, &ctx()).unwrap();
    match out.as_slice() {
        [I::Atomic(A::Boolean(b))] => *b,
        _ => panic!("expected boolean for {expr}"),
    }
}

// Property: For integers a,b: if a < b then not(a >= b) and if a > b then not(a <= b). Equality symmetry also.
proptest! {
    #[test]
    fn prop_integer_ordering_consistency(a in -1_000i64..1_000, b in -1_000i64..1_000) {
        if a < b {
            let lt = eval_bool(&format!("{a} lt {b}"));
            let ge = eval_bool(&format!("{a} ge {b}"));
            prop_assert!(lt, "lt should be true for {a} < {b}");
            prop_assert!(!ge, "ge should be false when {a} < {b}");
        } else if a > b {
            let gt = eval_bool(&format!("{a} gt {b}"));
            let le = eval_bool(&format!("{a} le {b}"));
            prop_assert!(gt, "gt should be true for {a} > {b}");
            prop_assert!(!le, "le should be false when {a} > {b}");
        } else { // a == b
            let eq = eval_bool(&format!("{a} eq {b}"));
            let ne = eval_bool(&format!("{a} ne {b}"));
            prop_assert!(eq, "eq true for equality {a} == {b}");
            prop_assert!(!ne, "ne false for equality {a} == {b}");
        }
    }
}
