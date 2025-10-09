use platynui_xpath::{
    XdmItem,
    functions::deep_equal_with_collation,
    simple_node::{SimpleNode, attr, elem, text},
};
use rstest::rstest;

// Helper to wrap a single node into a sequence for deep-equal($a,$b)
fn deep_equal_nodes(a: SimpleNode, b: SimpleNode) -> bool {
    let left = vec![XdmItem::Node(a)];
    let right = vec![XdmItem::Node(b)];
    deep_equal_with_collation(&left, &right, None).unwrap()
}

#[rstest]
fn elements_with_same_structure_and_attr_order_irrelevant() {
    let a =
        elem("root").attr(attr("id", "1")).attr(attr("class", "x")).child(elem("child").child(text("hello"))).build();
    let b = elem("root")
        .attr(attr("class", "x"))
        .attr(attr("id", "1")) // different order
        .child(elem("child").child(text("hello")))
        .build();
    assert!(deep_equal_nodes(a, b));
}

#[rstest]
fn elements_different_attribute_value() {
    let a = elem("root").attr(attr("id", "1")).build();
    let b = elem("root").attr(attr("id", "2")).build();
    assert!(!deep_equal_nodes(a, b));
}

#[rstest]
fn elements_child_order_matters() {
    let a = elem("root").child(elem("a")).child(elem("b")).build();
    let b = elem("root").child(elem("b")).child(elem("a")).build();
    assert!(!deep_equal_nodes(a, b));
}

#[rstest]
fn text_nodes_case_sensitive_default() {
    let a = text("Hello");
    let b = text("hello");
    assert!(!deep_equal_nodes(a, b));
}
