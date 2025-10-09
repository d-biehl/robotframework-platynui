use platynui_xpath::{
    evaluator::evaluate_expr,
    runtime::DynamicContextBuilder,
    xdm::{XdmAtomicValue as A, XdmItem as I},
};
use rstest::rstest;

fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContextBuilder::new().build()
}

fn eval(expr: &str) -> Vec<I<platynui_xpath::model::simple::SimpleNode>> {
    evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx()).unwrap()
}

#[rstest]
fn index_of_mixed_numeric_equality() {
    let r = eval("index-of((1, 1.0, 2, 2.0, 3.00), 2)");
    // Expect both positions of 2 (integer 3rd and double 4th) to match
    assert_eq!(r.len(), 2);
    match &r[0] {
        I::Atomic(A::Integer(3)) => {}
        _ => panic!("expected position 3"),
    }
    match &r[1] {
        I::Atomic(A::Integer(4)) => {}
        _ => panic!("expected position 4"),
    }
}

#[rstest]
fn index_of_nan_never_matches() {
    let r = eval("index-of((number('NaN'), number('NaN')), number('NaN'))");
    assert!(r.is_empty(), "NaN must not equal anything, even itself");
}
