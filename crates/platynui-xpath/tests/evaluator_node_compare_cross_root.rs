use platynui_xpath::engine::runtime::{DynamicContextBuilder, ErrorCode};
use platynui_xpath::{
    ExpandedName, xdm::XdmItem as I, XdmNode, evaluate_expr,
    simple_node::{doc, elem},
};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;

fn doc_root(name: &str) -> N {
    let d = doc().child(elem(name)).build();
    let roots = d.children();
    assert_eq!(roots.len(), 1);
    roots[0].clone()
}

#[rstest]
fn node_before_cross_root_errors() {
    let left = doc_root("l");
    let right = doc_root("r");
    let var_x = ExpandedName {
        ns_uri: None,
        local: "x".to_string(),
    };
    let ctx = DynamicContextBuilder::default()
        .with_context_item(I::Node(left))
        .with_variable(var_x.clone(), vec![I::Node(right)])
        .build();
    let err = evaluate_expr::<N>(". << $x", &ctx)
        .expect_err("should error for cross-root");
    assert_eq!(err.code_enum(), ErrorCode::FOER0000);
}

#[rstest]
fn node_after_cross_root_errors() {
    let left = doc_root("l");
    let right = doc_root("r");
    let var_x = ExpandedName {
        ns_uri: None,
        local: "x".to_string(),
    };
    let ctx = DynamicContextBuilder::default()
        .with_context_item(I::Node(left))
        .with_variable(var_x.clone(), vec![I::Node(right)])
        .build();
    let err = evaluate_expr::<N>(". >> $x", &ctx)
        .expect_err("should error for cross-root");
    assert_eq!(err.code_enum(), ErrorCode::FOER0000);
}
