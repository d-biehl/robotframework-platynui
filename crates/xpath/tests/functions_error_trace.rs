use platynui_xpath::{
    evaluate_expr,
    runtime::{DynamicContext, ErrorCode},
};
use rstest::rstest;

fn ctx() -> DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContext::default()
}

#[rstest]
fn error_zero_arity_default() {
    let c = ctx();
    let err = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("error()", &c).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::FOER0000);
}

#[rstest]
fn error_with_custom_code() {
    let c = ctx();
    let err = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(
        "error(QName('http://www.w3.org/2005/xqt-errors', 'XPST0003'))",
        &c,
    )
    .unwrap_err();
    assert_eq!(err.code.local, "XPST0003");
}

#[rstest]
fn error_with_desc_and_data() {
    let c = ctx();
    let err = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(
        "error(QName('http://www.w3.org/2005/xqt-errors', 'FORG0001'), 'bad cast', 123)",
        &c,
    )
    .unwrap_err();
    assert_eq!(err.code.local, "FORG0001");
}

#[rstest]
fn trace_passthrough_empty_label() {
    let c = ctx();
    let out = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("trace((1,2,3), '')", &c).unwrap();
    assert_eq!(out.len(), 3);
}

#[rstest]
fn trace_passthrough_string() {
    let c = ctx();
    let out = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("string(trace('ab', 'lbl'))", &c).unwrap();
    assert_eq!(out.len(), 1);
}
