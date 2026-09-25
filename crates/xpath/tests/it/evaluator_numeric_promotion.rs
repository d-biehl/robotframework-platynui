use platynui_xpath::SimpleNode;
use platynui_xpath::engine::evaluator::evaluate_expr; // if public helper not exposed, adjust to compile+evaluate
use platynui_xpath::engine::runtime::DynamicContext;
use rstest::rstest; // placeholder trait

fn eval(expr: &str) -> bool {
    let ctx: DynamicContext<SimpleNode> = DynamicContext::default();
    let seq = evaluate_expr(expr, &ctx).expect("evaluation");
    match seq.first() {
        Some(platynui_xpath::xdm::XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b))) => *b,
        _ => panic!("expected boolean result"),
    }
}

#[rstest]
fn int_vs_decimal_eq() {
    assert!(eval("1 eq 1.0"));
}
#[rstest]
fn int_vs_float_lt() {
    assert!(eval("1 lt 1.5e0"));
}
#[rstest]
fn decimal_vs_float_gt() {
    assert!(!eval("1.25 gt 1.3e0"));
}
#[rstest]
fn float_vs_double_ne() {
    assert!(eval("1.0e0 ne 2.0E0"));
}
#[rstest]
fn chain_mixed_le() {
    assert!(eval("1 le 1.0"));
}
