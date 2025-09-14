use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::{xdm::XdmItem, engine::evaluator::evaluate_expr};

type N = platynui_xpath::model::simple::SimpleNode;

fn dyn_ctx_with_collation(uri: &str) -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::default()
        .with_default_collation(uri.to_string())
        .build()
}

#[test]
fn case_insensitive_equality_and_order() {
    let ctx = dyn_ctx_with_collation(platynui_xpath::engine::collation::SIMPLE_CASE_URI);
    // Equality with default collation (simple case-insensitive)
    let out = evaluate_expr::<N>("'Ab' = 'ab'", &ctx).unwrap();
    assert_eq!(out.len(), 1);
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("expected boolean"),
    }
    // General comparison also respects collation
    let out2 = evaluate_expr::<N>("('Z','Ab') = ('ab')", &ctx).unwrap();
    match &out2[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("expected boolean"),
    }
    // Ordering with collation
    let lt_out = evaluate_expr::<N>("'aa' lt 'Ab'", &ctx).unwrap();
    match &lt_out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("expected boolean"),
    }
}
