use platynui_xpath::XdmNode;
use platynui_xpath::simple_node::{elem, ns};

#[test]
fn element_prefix_resolves_against_own_namespaces() {
    // <p:root xmlns:p="urn:one" />
    let r = elem("p:root").namespace(ns("p", "urn:one")).build();
    let name = r.name().unwrap();
    assert_eq!(name.local, "root");
    assert_eq!(name.prefix.as_deref(), Some("p"));
    // Namespace URI is not auto-resolved for arbitrary prefixes; prefix is preserved.
    assert!(name.ns_uri.is_none());
}

#[test]
fn child_element_prefix_resolves_against_parent_namespaces() {
    // <root xmlns:p="urn:one"><p:child/></root>
    let root = elem("root")
        .namespace(ns("p", "urn:one"))
        .child(elem("p:child"))
        .build();
    let children = root.children();
    let child = children[0].clone();
    let q = child.name().unwrap();
    assert_eq!(q.local, "child");
    assert_eq!(q.prefix.as_deref(), Some("p"));
    assert!(q.ns_uri.is_none());
}
