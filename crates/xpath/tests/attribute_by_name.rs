//! Tests for the `XdmNode::attribute_by_name` method.

use platynui_xpath::model::simple::{attr, doc, elem, SimpleNode};
use platynui_xpath::model::{QName, XdmNode};

fn make_tree() -> SimpleNode {
    doc().child(
        elem("root")
            .attr(attr("id", "42"))
            .attr(attr("class", "main"))
            .child(elem("child").attr(attr("name", "inner"))),
    )
    .build()
}

#[test]
fn lookup_existing_attribute() {
    let tree = make_tree();
    // Navigate to first child element (root)
    let root = tree.children().next().expect("root element");

    let qn = QName { prefix: None, local: "id".into(), ns_uri: None };
    let found = root.attribute_by_name(&qn);
    assert!(found.is_some());
    assert_eq!(found.unwrap().string_value(), "42");
}

#[test]
fn lookup_missing_attribute_returns_none() {
    let tree = make_tree();
    let root = tree.children().next().expect("root element");

    let qn = QName { prefix: None, local: "nonexistent".into(), ns_uri: None };
    assert!(root.attribute_by_name(&qn).is_none());
}

#[test]
fn lookup_attribute_ns_mismatch() {
    let tree = make_tree();
    let root = tree.children().next().expect("root element");

    // Attribute "id" exists with no namespace; searching with a namespace must miss.
    let qn = QName { prefix: None, local: "id".into(), ns_uri: Some("urn:other".into()) };
    assert!(root.attribute_by_name(&qn).is_none());
}

#[test]
fn document_node_has_no_attributes() {
    let tree = make_tree();
    let qn = QName { prefix: None, local: "id".into(), ns_uri: None };
    // Document node itself should never match attributes.
    assert!(tree.attribute_by_name(&qn).is_none());
}

#[test]
fn attribute_node_has_no_sub_attributes() {
    let tree = make_tree();
    let root = tree.children().next().expect("root element");
    let id_attr = root.attribute_by_name(&QName { prefix: None, local: "id".into(), ns_uri: None }).unwrap();

    // An attribute node should not expose sub-attributes.
    let qn = QName { prefix: None, local: "id".into(), ns_uri: None };
    assert!(id_attr.attribute_by_name(&qn).is_none());
}
