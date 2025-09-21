use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmItem};
use rstest::{fixture, rstest};

type N = platynui_xpath::model::simple::SimpleNode;

fn dyn_ctx_with_collation(uri: &str) -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().with_default_collation(uri.to_string()).build()
}

#[fixture]
fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    dyn_ctx_with_collation(platynui_xpath::engine::collation::SIMPLE_CASE_URI)
}

#[rstest]
fn equality_case_insensitive(ctx: platynui_xpath::engine::runtime::DynamicContext<N>) {
    let out = evaluate_expr::<N>("'Ab' = 'ab'", &ctx).unwrap();
    assert_eq!(out.len(), 1);
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("expected boolean"),
    }
}

#[rstest]
fn general_comparison_respects_collation(ctx: platynui_xpath::engine::runtime::DynamicContext<N>) {
    let out2 = evaluate_expr::<N>("('Z','Ab') = ('ab')", &ctx).unwrap();
    match &out2[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("expected boolean"),
    }
}

#[rstest]
fn ordering_with_collation_lt(ctx: platynui_xpath::engine::runtime::DynamicContext<N>) {
    let lt_out = evaluate_expr::<N>("'aa' lt 'Ab'", &ctx).unwrap();
    match &lt_out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("expected boolean"),
    }
}
