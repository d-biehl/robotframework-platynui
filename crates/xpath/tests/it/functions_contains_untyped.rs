use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use platynui_xpath::{evaluator::evaluate_expr, runtime::DynamicContextBuilder};
use rstest::rstest;

fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContextBuilder::new().build()
}

fn eval_bool(expr: &str) -> bool {
    let c = ctx();
    let seq = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &c).unwrap();
    assert_eq!(seq.len(), 1, "expected single result, got {}", seq.len());
    match &seq[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => *b,
        other => panic!("expected Boolean, got {other:?}"),
    }
}

#[rstest]
#[case("fn:contains(untypedAtomic('abc'),'b')", true)]
#[case("fn:contains(untypedAtomic('abc'),'z')", false)]
fn contains_with_untyped_atomic(#[case] expr: &str, #[case] expected: bool) {
    assert_eq!(eval_bool(expr), expected);
}
