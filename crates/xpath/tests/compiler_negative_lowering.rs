use platynui_xpath::compiler::compile_with_context;
use platynui_xpath::engine::runtime::{ErrorCode, StaticContext};
use rstest::{fixture, rstest};

#[fixture]
fn ctx() -> StaticContext {
    return StaticContext::default();
}

// Negative lowering: invalid constructs should yield XPST0003
#[rstest]
fn invalid_axis(ctx: StaticContext) {
    let err = compile_with_context("/nonsense-axis::node()", &ctx).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::XPST0003);
}

#[rstest]
fn invalid_document_node_arg(ctx: StaticContext) {
    let err = compile_with_context("self::document-node(text())", &ctx).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::XPST0003);
}

#[rstest]
fn invalid_qname(ctx: StaticContext) {
    let err = compile_with_context("//1abc", &ctx).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::XPST0003);
}

#[rstest]
fn unmatched_bracket(ctx: StaticContext) {
    let err = compile_with_context("(1,2,3[", &ctx).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::XPST0003);
}

#[rstest]
fn stray_operator(ctx: StaticContext) {
    let err = compile_with_context("1 +", &ctx).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::XPST0003);
}
