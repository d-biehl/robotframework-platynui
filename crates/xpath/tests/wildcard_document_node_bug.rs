//! Tests ensuring XPath wildcards (*) don't incorrectly select document nodes.
//!
//! The XPath standard requires wildcards to only match element nodes, but the
//! original implementation also selected document nodes.

use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder};
use platynui_xpath::model::simple::{attr, doc, elem};
use platynui_xpath::{XdmNode, evaluate_expr, xdm::XdmItem as I};
use rstest::{fixture, rstest};

type N = platynui_xpath::model::simple::SimpleNode;

#[fixture]
fn document_context() -> DynamicContext<N> {
    let d = doc().child(elem("root").child(elem("child"))).build();

    DynamicContextBuilder::default().with_context_item(I::Node(d)).build()
}

#[fixture]
fn complex_nested_context() -> DynamicContext<N> {
    let d = doc()
        .child(
            elem("Application")
                .attr(attr("app:Name", "olk"))
                .child(elem("Window").child(elem("Panel").child(elem("Control").attr(attr("Name", "Daniel User"))))),
        )
        .build();

    DynamicContextBuilder::default().with_context_item(I::Node(d)).build()
}

#[rstest]
fn wildcard_should_not_match_document_node(document_context: DynamicContext<N>) {
    let ctx = document_context;

    let result = evaluate_expr::<N>("*", &ctx).unwrap();

    for item in &result {
        if let I::Node(node) = item {
            assert_eq!(
                node.kind(),
                platynui_xpath::model::NodeKind::Element,
                "Wildcard * should only match element nodes, but found: {:?}",
                node.kind()
            );
        }
    }

    assert_eq!(result.len(), 1, "Expected exactly 1 element from wildcard");

    let self_result = evaluate_expr::<N>("self::*", &ctx).unwrap();
    assert!(self_result.is_empty(), "self::* from document node should be empty, but got {} items", self_result.len());

    let desc_result = evaluate_expr::<N>("descendant-or-self::*", &ctx).unwrap();
    for item in &desc_result {
        if let I::Node(node) = item {
            assert_ne!(
                node.kind(),
                platynui_xpath::model::NodeKind::Document,
                "descendant-or-self::* should not include document nodes"
            );
        }
    }
}

#[rstest]
fn ancestor_wildcard_should_not_match_document_node(complex_nested_context: DynamicContext<N>) {
    let ctx = complex_nested_context;

    let result = evaluate_expr::<N>("//Control[@Name='Daniel User']/ancestor::*", &ctx).unwrap();

    for item in &result {
        if let I::Node(node) = item {
            assert_eq!(
                node.kind(),
                platynui_xpath::model::NodeKind::Element,
                "ancestor::* should only match element nodes, but found: {:?}",
                node.kind()
            );
        }
    }

    assert_eq!(result.len(), 3, "Expected exactly 3 ancestors: Panel, Window, Application");

    let names: Vec<String> = result
        .iter()
        .filter_map(|i| match i {
            I::Node(n) => n.name().map(|q| q.local.to_string()),
            _ => None,
        })
        .collect();

    assert!(names.contains(&"Panel".to_string()), "Should contain 'Panel'");
    assert!(names.contains(&"Window".to_string()), "Should contain 'Window'");
    assert!(names.contains(&"Application".to_string()), "Should contain 'Application'");
}

#[rstest]
fn document_node_test_explicit(document_context: DynamicContext<N>) {
    let ctx = document_context;

    let doc_result = evaluate_expr::<N>("self::document-node()", &ctx).unwrap();
    assert_eq!(doc_result.len(), 1, "Expected exactly 1 document node");

    if let I::Node(node) = &doc_result[0] {
        assert_eq!(
            node.kind(),
            platynui_xpath::model::NodeKind::Document,
            "document-node() should match document nodes"
        );
    } else {
        panic!("Expected node item");
    }
}
