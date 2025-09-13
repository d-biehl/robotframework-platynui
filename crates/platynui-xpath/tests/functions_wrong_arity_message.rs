use platynui_xpath::evaluator::evaluate_expr;
use platynui_xpath::runtime::{DynamicContextBuilder, ErrorCode};
use platynui_xpath::simple_node::SimpleNode;
use rstest::rstest;

#[rstest]
fn contains_wrong_arity_message_is_humanized() {
    let ctx = DynamicContextBuilder::new().build();
    let err = evaluate_expr::<SimpleNode>("contains('abc')", &ctx).unwrap_err();
    // Code must be XPST0017 and message should be the humanized variant
    assert_eq!(err.code_enum(), ErrorCode::XPST0017);
    let msg = format!("{}", err);
    assert!(
        msg.contains("function contains() cannot be called with one argument"),
        "unexpected error message: {msg}"
    );
}
