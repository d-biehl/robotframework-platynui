use platynui_xpath::SimpleNode;
use platynui_xpath::engine::evaluator::evaluate_expr;
use platynui_xpath::engine::runtime::DynamicContext;
use rstest::rstest;

fn eval_bool(expr: &str) -> bool {
    let ctx: DynamicContext<SimpleNode> = DynamicContext::default();
    let seq = evaluate_expr(expr, &ctx).expect("eval");
    match seq.first() {
        Some(platynui_xpath::xdm::XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b))) => *b,
        _ => false,
    }
}

#[rstest]
fn incomparable_boolean_general_eq_false() {
    // General comparison: boolean vs numeric values are incomparable; overall result false.
    // Using sequence to force general comparison semantics.
    assert!(!eval_bool("(true(), false()) = (1, 2)"));
}

#[rstest]
fn incomparable_string_vs_boolean_eq_false() {
    assert!(!eval_bool("'a' = true()"));
}

#[rstest]
fn incomparable_numeric_vs_string_lt_false() {
    assert!(!eval_bool("1 < 'x'"));
}
