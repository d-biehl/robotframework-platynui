use platynui_xpath::xdm::{XdmAtomicValue as A, XdmItem as I};
use platynui_xpath::{evaluator::evaluate_expr, runtime::DynamicContextBuilder};
use rstest::rstest;

fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContextBuilder::new().build()
}

fn eval_bool(expr: &str) -> bool {
    let c = ctx();
    let seq = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &c).unwrap();
    match &seq[0] {
        I::Atomic(A::Boolean(b)) => *b,
        other => panic!("Expected Boolean, got {:?}", other),
    }
}

#[rstest]
fn deep_equal_nodes_identical() {
    // Node construction integration not exercised deeply yet; use atomic sequences to
    // assert ordering significance and equality baseline.
    assert!(eval_bool("fn:deep-equal(('a','b'),('a','b'))"));
    // Different ordering differs
    assert!(!eval_bool("fn:deep-equal(('a','b'),('b','a'))"));
}

#[rstest]
fn deep_equal_mixed_node_atomic_difference() {
    // Spec-conform: numeric 1 and string '1' are NOT deep-equal (no cross-type parse fallback).
    assert!(!eval_bool("fn:deep-equal((1),('1'))"));
}
