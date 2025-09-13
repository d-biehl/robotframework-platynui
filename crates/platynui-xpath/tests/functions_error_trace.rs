use platynui_xpath::{
    SimpleNode, evaluate_expr,
    runtime::{DynamicContext, ErrorCode},
};
use rstest::rstest;

fn ctx() -> DynamicContext<SimpleNode> {
    DynamicContext::default()
}

#[rstest]
fn error_zero_arity_default() {
    let c = ctx();
    let err = evaluate_expr::<SimpleNode>("error()", &c).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::FOER0000);
}

#[rstest]
fn error_with_custom_code() {
    let c = ctx();
    let err = evaluate_expr::<SimpleNode>("error('err:XPST0003')", &c).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::XPST0003);
}

#[rstest]
fn error_with_desc_and_data() {
    let c = ctx();
    let err =
        evaluate_expr::<SimpleNode>("error('err:FORG0001', 'bad cast', 123)", &c).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::FORG0001);
}

#[rstest]
fn trace_passthrough_empty_label() {
    let c = ctx();
    let out = evaluate_expr::<SimpleNode>("trace((1,2,3), '')", &c).unwrap();
    assert_eq!(out.len(), 3);
}

#[rstest]
fn trace_passthrough_string() {
    let c = ctx();
    let out = evaluate_expr::<SimpleNode>("string(trace('ab', 'lbl'))", &c).unwrap();
    assert_eq!(out.len(), 1);
}
