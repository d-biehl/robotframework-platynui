use platynui_xpath::runtime::DynamicContextBuilder;
use platynui_xpath::{
    SimpleNode, evaluate_expr,
    simple_node::{doc, elem, text},
};
use rstest::rstest;

fn ctx() -> platynui_xpath::runtime::DynamicContext<SimpleNode> {
    let root = doc().child(elem("r").child(text("x"))).build();
    DynamicContextBuilder::default()
        .with_context_item(root)
        .build()
}

#[rstest]
fn exactly_one_happy() {
    let c = ctx();
    let out = evaluate_expr::<SimpleNode>("exactly-one((42))", &c).unwrap();
    assert_eq!(out.len(), 1);
}

#[rstest]
fn exactly_one_error() {
    let c = ctx();
    let err = evaluate_expr::<SimpleNode>("exactly-one((1,2))", &c).unwrap_err();
    assert_eq!(err.code_enum().as_str(), "err:FORG0005");
}

#[rstest]
fn one_or_more_happy() {
    let c = ctx();
    let out = evaluate_expr::<SimpleNode>("one-or-more((1,2))", &c).unwrap();
    assert_eq!(out.len(), 2);
}

#[rstest]
fn one_or_more_error() {
    let c = ctx();
    let err = evaluate_expr::<SimpleNode>("one-or-more(())", &c).unwrap_err();
    assert_eq!(err.code_enum().as_str(), "err:FORG0004");
}

#[rstest]
fn zero_or_one_happy() {
    let c = ctx();
    let out = evaluate_expr::<SimpleNode>("zero-or-one((1))", &c).unwrap();
    assert_eq!(out.len(), 1);
}

#[rstest]
fn zero_or_one_error() {
    let c = ctx();
    let err = evaluate_expr::<SimpleNode>("zero-or-one((1,2))", &c).unwrap_err();
    assert_eq!(err.code_enum().as_str(), "err:FORG0004");
}
