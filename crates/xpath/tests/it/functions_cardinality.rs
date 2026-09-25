use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::{
    evaluate_expr,
    simple_node::{doc, elem, text},
};
use rstest::rstest;

fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    let root = doc().child(elem("r").child(text("x"))).build();
    DynamicContextBuilder::default().with_context_item(root).build()
}

#[rstest]
fn exactly_one_happy() {
    let c = ctx();
    let out = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("exactly-one((42))", &c).unwrap();
    assert_eq!(out.len(), 1);
}

#[rstest]
fn exactly_one_error() {
    let c = ctx();
    let err = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("exactly-one((1,2))", &c).unwrap_err();
    assert_eq!(err.code_enum(), platynui_xpath::engine::runtime::ErrorCode::FORG0005);
}

#[rstest]
fn one_or_more_happy() {
    let c = ctx();
    let out = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("one-or-more((1,2))", &c).unwrap();
    assert_eq!(out.len(), 2);
}

#[rstest]
fn one_or_more_error() {
    let c = ctx();
    let err = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("one-or-more(())", &c).unwrap_err();
    assert_eq!(err.code_enum(), platynui_xpath::engine::runtime::ErrorCode::FORG0004);
}

#[rstest]
fn zero_or_one_happy() {
    let c = ctx();
    let out = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("zero-or-one((1))", &c).unwrap();
    assert_eq!(out.len(), 1);
}

#[rstest]
fn zero_or_one_error() {
    let c = ctx();
    let err = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("zero-or-one((1,2))", &c).unwrap_err();
    assert_eq!(err.code_enum(), platynui_xpath::engine::runtime::ErrorCode::FORG0004);
}
