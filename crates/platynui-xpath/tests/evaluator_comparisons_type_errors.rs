use platynui_xpath::{
    SimpleNode, evaluate_expr,
    runtime::{DynamicContext, ErrorCode},
};
use rstest::rstest;

fn ctx() -> DynamicContext<SimpleNode> {
    DynamicContext::default()
}

fn err(expr: &str) -> ErrorCode {
    evaluate_expr::<SimpleNode>(expr, &ctx())
        .unwrap_err()
        .code_enum()
}

#[rstest]
#[case("true() lt false()", ErrorCode::XPTY0004)]
#[case("true() gt 1", ErrorCode::XPTY0004)]
#[case("'a' lt 1", ErrorCode::XPTY0004)]
fn comparison_type_errors(#[case] expr: &str, #[case] code: ErrorCode) {
    assert_eq!(err(expr), code);
}
