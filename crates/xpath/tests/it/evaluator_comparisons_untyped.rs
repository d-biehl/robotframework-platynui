use platynui_xpath::engine::runtime::ErrorCode;
use platynui_xpath::{
    SimpleNode, evaluate_expr,
    runtime::DynamicContext,
    xdm::{XdmAtomicValue, XdmItem},
};
use rstest::rstest;

fn ctx(val: &str) -> DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContext::<platynui_xpath::model::simple::SimpleNode> {
        context_item: Some(XdmItem::Node(SimpleNode::text(val))),
        ..Default::default()
    }
}

fn bool_of(expr: &str, val: &str) -> bool {
    let seq = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx(val)).unwrap();
    if let Some(XdmItem::Atomic(XdmAtomicValue::Boolean(b))) = seq.first() { *b } else { panic!("expected boolean") }
}

#[rstest]
#[case(". = .", "abc", true)]
#[case(". = 'def'", "abc", false)]
#[case(". = 5", "5", true)]
#[case("5 = .", "5", true)]
fn untyped_comparison_truth(#[case] expr: &str, #[case] ctx_val: &str, #[case] expected: bool) {
    assert_eq!(bool_of(expr, ctx_val), expected);
}

#[rstest]
fn untyped_vs_numeric_invalid() {
    let c = ctx("abc");
    let err = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(". = 5", &c).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::FORG0001);
}
