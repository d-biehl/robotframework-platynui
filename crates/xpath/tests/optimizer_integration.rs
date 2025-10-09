/// Integration tests for the optimizer module.
///
/// These tests verify that the predicate pushdown optimizer works correctly
/// in end-to-end scenarios with the complete compilation and evaluation pipeline.
use platynui_xpath::simple_node::{attr, doc, elem, text};
use platynui_xpath::xdm::XdmItem;
use platynui_xpath::{DynamicContextBuilder, XdmNode, evaluate_expr, evaluate_stream_expr};

type N = platynui_xpath::model::simple::SimpleNode;

/// Helper to create a simple test tree.
fn make_test_tree() -> N {
    doc()
        .child(
            elem("root")
                .child(elem("item").attr(attr("id", "foo")).child(text("First")))
                .child(elem("item").attr(attr("id", "bar")).child(text("Second")))
                .child(elem("item").attr(attr("id", "baz")).child(text("Third"))),
        )
        .build()
}

#[test]
fn test_predicate_pushdown_simple() {
    let root = make_test_tree();
    let root_elem = root.children().next().unwrap();
    let ctx = DynamicContextBuilder::new().with_context_item(XdmItem::Node(root_elem)).build();

    // This expression should have predicates pushed down:
    // Before: (//item)[@id='foo']
    // After: //item[@id='foo']
    let nodes = evaluate_expr::<N>("(//item)[@id='foo']", &ctx).unwrap();

    assert_eq!(nodes.len(), 1, "Should find exactly one item");
    assert!(matches!(nodes[0], XdmItem::Node(_)), "Should be a node");
}

#[test]
fn test_predicate_pushdown_multiple() {
    let root = make_test_tree();
    let root_elem = root.children().next().unwrap();
    let ctx = DynamicContextBuilder::new().with_context_item(XdmItem::Node(root_elem)).build();

    // Multiple predicates should all be pushed down
    let nodes = evaluate_expr::<N>("(//item)[@id='foo'][text()='First']", &ctx).unwrap();

    assert_eq!(nodes.len(), 1, "Should find exactly one item");
    assert!(matches!(nodes[0], XdmItem::Node(_)), "Should be a node");
}

#[test]
fn test_predicate_pushdown_positional() {
    let root = make_test_tree();
    let root_elem = root.children().next().unwrap();
    let ctx = DynamicContextBuilder::new().with_context_item(XdmItem::Node(root_elem)).build();

    // Positional predicate - should still work correctly after optimization
    let nodes = evaluate_expr::<N>("(//item)[1]", &ctx).unwrap();

    assert_eq!(nodes.len(), 1, "Should find exactly one item");
    assert!(matches!(nodes[0], XdmItem::Node(_)), "Should be a node");
}

#[test]
fn test_optimizer_streaming_benefit() {
    let root = make_test_tree();
    let root_elem = root.children().next().unwrap();
    let ctx = DynamicContextBuilder::new().with_context_item(XdmItem::Node(root_elem)).build();

    // With optimization, this should be able to stop after finding the first match
    let stream = evaluate_stream_expr::<N>("(//item[@id='foo'])[1]", &ctx).unwrap();
    let mut iter = stream.iter();

    // Take only first item - with streaming this should be efficient
    assert!(iter.next().is_some(), "Should have at least one result");
}

#[test]
fn test_nested_predicates() {
    let root = doc()
        .child(
            elem("root").child(
                elem("container")
                    .attr(attr("type", "main"))
                    .child(elem("item").attr(attr("id", "nested")).child(text("Nested"))),
            ),
        )
        .build();

    let root_elem = root.children().next().unwrap();
    let ctx = DynamicContextBuilder::new().with_context_item(XdmItem::Node(root_elem)).build();

    // Nested path with predicates at multiple levels
    let nodes = evaluate_expr::<N>("(//container[@type='main']/item)[@id='nested']", &ctx).unwrap();

    assert_eq!(nodes.len(), 1, "Should find the nested item");
    assert!(matches!(nodes[0], XdmItem::Node(_)), "Should be a node");
}
