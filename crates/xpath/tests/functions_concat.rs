use platynui_xpath::engine::evaluator::evaluate_expr;
use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use rstest::rstest;

#[rstest]
fn concat_variadic_many_args() {
    let ctx = DynamicContextBuilder::<platynui_xpath::model::simple::SimpleNode>::new().build();
    let expr = r#"concat('a', '-', 'b', '-', 'c', '-', 'd', '-', 'e', '-', 'f', '-', 'g')"#;
    let result = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx)
        .expect("concat should succeed");
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert_eq!(s, "a-b-c-d-e-f-g"),
        other => panic!("expected atomic String, got {other:?}"),
    }
}

#[rstest]
fn concat_too_few_args_reports_wrong_arity() {
    let ctx = DynamicContextBuilder::<platynui_xpath::model::simple::SimpleNode>::new().build();
    // Note: call with one argument should be a static error XPST0017 (wrong arity)
    let expr = r#"concat('only-one')"#;
    let err = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx).unwrap_err();
    // Message should mention function cannot be called with one argument
    let msg = format!("{}", err);
    assert!(msg.contains("cannot be called with one argument") || msg.contains("one argument"));
}
