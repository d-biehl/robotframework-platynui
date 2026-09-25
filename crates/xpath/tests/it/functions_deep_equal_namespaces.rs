use platynui_xpath::functions::deep_equal_with_collation;
use platynui_xpath::model::simple::SimpleNode;
use platynui_xpath::model::simple::{elem, ns, text};
use platynui_xpath::xdm::XdmItem;
use rstest::rstest;

fn wrap(root: SimpleNode) -> Vec<XdmItem<platynui_xpath::model::simple::SimpleNode>> {
    vec![XdmItem::Node(root)]
}

#[rstest]
fn deep_equal_same_namespaces_different_prefix_order() {
    // <r p1:one="" p2:two=""> (namespaces only)
    let a = elem("r").namespace(ns("p1", "urn:one")).namespace(ns("p2", "urn:two")).child(text("x")).build();
    let b = elem("r").namespace(ns("p2", "urn:two")).namespace(ns("p1", "urn:one")).child(text("x")).build();
    let result = deep_equal_with_collation(&wrap(a), &wrap(b), None).unwrap();
    assert!(result, "Namespace set order should not matter");
}

#[rstest]
fn deep_equal_namespace_missing() {
    let a = elem("r").namespace(ns("p1", "urn:one")).child(text("x")).build();
    let b = elem("r").namespace(ns("p1", "urn:one")).namespace(ns("p2", "urn:two")).child(text("x")).build();
    let result = deep_equal_with_collation(&wrap(a), &wrap(b), None).unwrap();
    assert!(!result, "Different namespace sets must be unequal");
}

#[rstest]
fn deep_equal_namespace_uri_diff_same_prefix() {
    let a = elem("r").namespace(ns("p", "urn:one")).child(text("x")).build();
    let b = elem("r").namespace(ns("p", "urn:two")).child(text("x")).build();
    let result = deep_equal_with_collation(&wrap(a), &wrap(b), None).unwrap();
    assert!(!result, "Same prefix different URI should be unequal");
}
