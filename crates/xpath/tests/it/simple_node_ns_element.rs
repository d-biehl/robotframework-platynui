use platynui_xpath::model::XdmNode;
use platynui_xpath::model::simple::{elem, ns};
use rstest::rstest;

#[rstest]
fn element_prefix_resolves_against_own_namespaces() {
    // <p:root xmlns:p="urn:one" />
    let r = elem("p:root").namespace(ns("p", "urn:one")).build();
    let name = r.name().unwrap();
    assert_eq!(name.local, "root");
    assert_eq!(name.prefix.as_deref(), Some("p"));
    // Namespace URI is not auto-resolved for arbitrary prefixes; prefix is preserved.
    assert!(name.ns_uri.is_none());
}

#[rstest]
fn child_element_prefix_resolves_against_parent_namespaces() {
    // <root xmlns:p="urn:one"><p:child/></root>
    let root = elem("root").namespace(ns("p", "urn:one")).child(elem("p:child")).build();
    let child = root.children().next().unwrap();
    let q = child.name().unwrap();
    assert_eq!(q.local, "child");
    assert_eq!(q.prefix.as_deref(), Some("p"));
    assert!(q.ns_uri.is_none());
}
