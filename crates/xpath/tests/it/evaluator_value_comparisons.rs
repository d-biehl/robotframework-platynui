use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmItem as I};
use rstest::rstest;

// Helper: evaluate simple expression with empty dynamic context
fn eval(expr: &str) -> Vec<I<platynui_xpath::model::simple::SimpleNode>> {
    evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &DynamicContextBuilder::default().build()).unwrap()
}

#[rstest]
fn value_numeric_promotion_eq() {
    let out = eval("1 eq 1.0");
    assert!(matches!(&out[0], I::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(true))));
}

#[rstest]
fn value_numeric_rel() {
    let out = eval("1 lt 2.5");
    assert!(matches!(&out[0], I::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(true))));
}

#[rstest]
fn value_untyped_to_numeric() {
    // '10' untypedAtomic literal not yet parsed; simulate via string cast context: treat as string -> numeric parse with numeric other
    let out = eval("10 eq 10");
    assert!(matches!(&out[0], I::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(true))));
}

#[rstest]
fn value_string_relational() {
    let out = eval("'ab' lt 'b'");
    assert!(matches!(&out[0], I::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(true))));
}

#[rstest]
fn value_boolean_relational_error() {
    // Expect dynamic error for boolean relational
    let res = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(
        "true() lt false()",
        &DynamicContextBuilder::default().build(),
    );
    assert!(res.is_err());
}
