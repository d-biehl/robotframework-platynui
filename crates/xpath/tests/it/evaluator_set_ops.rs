use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::{
    XdmNode, evaluate_expr,
    simple_node::{doc, elem},
    xdm::XdmItem as I,
};
use rstest::rstest;
type N = platynui_xpath::model::simple::SimpleNode;
fn build_tree() -> N {
    // <root><a/><b/><c/></root>
    let d = doc().child(elem("root").child(elem("a")).child(elem("b")).child(elem("c"))).build();
    let root: Vec<_> = d.children().collect();
    assert_eq!(root.len(), 1);
    root[0].clone()
}
fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    let root = build_tree();
    let b = DynamicContextBuilder::default();
    b.with_context_item(I::Node(root)).build()
}

#[rstest]
fn union_nodes() {
    let out = evaluate_expr::<N>("child::a union child::b", &ctx()).unwrap();
    assert_eq!(out.len(), 2);
}

#[rstest]
fn intersect_nodes() {
    let out = evaluate_expr::<N>("(child::a, child::b) intersect (child::b, child::c)", &ctx()).unwrap();
    assert_eq!(out.len(), 1);
}

#[rstest]
fn except_nodes() {
    let out = evaluate_expr::<N>("(child::a, child::b, child::c) except (child::b)", &ctx()).unwrap();
    assert_eq!(out.len(), 2);
}

#[rstest]
fn set_ops_on_atomics_error() {
    let err = evaluate_expr::<N>("(1,2) union (2,3)", &ctx()).expect_err("should error");
    assert_eq!(err.code_enum(), platynui_xpath::engine::runtime::ErrorCode::XPTY0004);
}
