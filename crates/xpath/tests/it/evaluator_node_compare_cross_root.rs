use platynui_xpath::engine::runtime::{DynamicContextBuilder, ErrorCode, StaticContextBuilder};
use platynui_xpath::{
    ExpandedName, XdmNode, compile_with_context, evaluate,
    simple_node::{doc, elem},
    xdm::XdmItem as I,
};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;

fn doc_root(name: &str) -> N {
    let d = doc().child(elem(name)).build();
    let roots: Vec<_> = d.children().collect();
    assert_eq!(roots.len(), 1);
    roots[0].clone()
}

#[rstest]
fn node_before_cross_root_errors() {
    let left = doc_root("l");
    let right = doc_root("r");
    let var_x = ExpandedName { ns_uri: None, local: "x".to_string() };
    let ctx = DynamicContextBuilder::default()
        .with_context_item(I::Node(left))
        .with_variable(var_x.clone(), vec![I::Node(right)])
        .build();
    let static_ctx = StaticContextBuilder::new().with_variable(var_x.clone()).build();
    let compiled = compile_with_context(". << $x", &static_ctx).unwrap();
    let err = evaluate::<N>(&compiled, &ctx).expect_err("should error for cross-root");
    assert_eq!(err.code_enum(), ErrorCode::FOER0000);
}

#[rstest]
fn node_after_cross_root_errors() {
    let left = doc_root("l");
    let right = doc_root("r");
    let var_x = ExpandedName { ns_uri: None, local: "x".to_string() };
    let ctx = DynamicContextBuilder::default()
        .with_context_item(I::Node(left))
        .with_variable(var_x.clone(), vec![I::Node(right)])
        .build();
    let static_ctx = StaticContextBuilder::new().with_variable(var_x.clone()).build();
    let compiled = compile_with_context(". >> $x", &static_ctx).unwrap();
    let err = evaluate::<N>(&compiled, &ctx).expect_err("should error for cross-root");
    assert_eq!(err.code_enum(), ErrorCode::FOER0000);
}
