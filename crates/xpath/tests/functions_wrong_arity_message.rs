use platynui_xpath::engine::evaluator::evaluate_expr;
use platynui_xpath::engine::runtime::{DynamicContextBuilder, ErrorCode};
use rstest::rstest;

#[rstest]
fn contains_wrong_arity_message_is_humanized() {
    let ctx = DynamicContextBuilder::new().build();
    let err = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("contains('abc')", &ctx).unwrap_err();
    // Code must be XPST0017 and message should be the humanized variant
    assert_eq!(err.code_enum(), ErrorCode::XPST0017);
    let msg = format!("{}", err);
    assert!(msg.contains("function contains() cannot be called with one argument"), "unexpected error message: {msg}");
}
