//! XPath 2.0 §3.2: a leading `/` abbreviates
//! `(fn:root(self::node()) treat as document-node())`, so an absolute path is a
//! dynamic error (XPDY0050) when the context node's tree is not rooted at a
//! document node.

use platynui_xpath::compiler::compile_with_context;
use platynui_xpath::engine::runtime::{DynamicContextBuilder, ErrorCode, StaticContextBuilder};
use platynui_xpath::evaluate;
use platynui_xpath::model::XdmNode;
use platynui_xpath::model::simple::{SimpleNode, doc, elem, text};
use platynui_xpath::xdm::XdmItem;
use rstest::rstest;

/// `<root><child>x</child></root>` without a surrounding document node.
fn free_standing_element() -> SimpleNode {
    elem("root").child(elem("child").child(text("x"))).build()
}

fn in_document() -> SimpleNode {
    doc().child(elem("root").child(elem("child").child(text("x")))).build()
}

fn eval(expr: &str, context: SimpleNode) -> Result<Vec<XdmItem<SimpleNode>>, platynui_xpath::runtime::Error> {
    let compiled = compile_with_context(expr, &StaticContextBuilder::new().build()).expect("compile ok");
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(context)).build();
    evaluate::<SimpleNode>(&compiled, &ctx)
}

#[rstest]
#[case("/")]
#[case("/.")]
#[case("/root")]
#[case("//child")]
#[case("/root/child")]
fn absolute_path_without_document_root_raises_xpdy0050(#[case] expr: &str) {
    let err = eval(expr, free_standing_element()).expect_err("expected XPDY0050");
    assert_eq!(err.code_enum(), ErrorCode::XPDY0050, "unexpected error for {expr}: {err}");
}

#[rstest]
#[case("/", 1)]
#[case("/.", 1)]
#[case("/root", 1)]
#[case("//child", 1)]
#[case("/root/child", 1)]
fn absolute_path_with_document_root_still_works(#[case] expr: &str, #[case] expected: usize) {
    let result = eval(expr, in_document()).expect("eval ok");
    assert_eq!(result.len(), expected, "unexpected result for {expr}");
}

/// The context node may sit anywhere in the tree — only the root matters.
#[rstest]
fn absolute_path_from_inner_node_uses_document_root() {
    let document = in_document();
    let root_elem = document.children().next().expect("root element");
    let result = eval("/root/child", root_elem).expect("eval ok");
    assert_eq!(result.len(), 1);
}

/// `fn:root()` has no `treat as`, so it keeps working on document-less trees.
#[rstest]
fn fn_root_is_unaffected() {
    let free = free_standing_element();
    let result = eval("root()", free.clone()).expect("eval ok");
    assert_eq!(result, vec![XdmItem::Node(free)]);
}

/// A relative path never triggers the document-node requirement.
#[rstest]
#[case("child", 1)]
#[case(".", 1)]
#[case("descendant::child", 1)]
fn relative_path_without_document_root_is_fine(#[case] expr: &str, #[case] expected: usize) {
    let result = eval(expr, free_standing_element()).expect("eval ok");
    assert_eq!(result.len(), expected, "unexpected result for {expr}");
}
