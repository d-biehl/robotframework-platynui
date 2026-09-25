use platynui_xpath::{attr, elem, model::XdmNode, model::simple::SimpleNode, ns, simple_doc, text};
use rstest::rstest;

fn cmp(a: &SimpleNode, b: &SimpleNode) -> &'static str {
    match a.compare_document_order(b) {
        Ok(core::cmp::Ordering::Less) => "<",
        Ok(core::cmp::Ordering::Greater) => ">",
        Ok(core::cmp::Ordering::Equal) => "=",
        Err(_) => "!",
    }
}

#[rstest]
fn ordering_attributes_before_children() {
    let root = elem("root").attr(attr("id", "1")).attr(attr("b", "x")).child(elem("a")).child(elem("b")).build();
    let attrs: Vec<_> = root.attributes().collect();
    let kids: Vec<_> = root.children().collect();
    assert_eq!(cmp(&attrs[0], &attrs[1]), "<");
    assert_eq!(cmp(&attrs[1], &kids[0]), "<");
    assert_eq!(cmp(&kids[0], &kids[1]), "<");
}

#[rstest]
fn ordering_ancestor_before_descendant() {
    let tree = elem("r").child(elem("a").child(elem("b"))).build();
    let a = tree.children().next().unwrap();
    let b = a.children().next().unwrap();
    assert_eq!(cmp(&tree, &a), "<");
    assert_eq!(cmp(&a, &b), "<");
}

#[rstest]
fn namespaces_nested_lookup() {
    let t = elem("root")
        .namespace(ns("p", "urn:one"))
        .child(elem("mid").namespace(ns("q", "urn:two")).child(elem("leaf")))
        .build();
    let mid = t.children().next().unwrap();
    let leaf = mid.children().next().unwrap();
    assert_eq!(mid.lookup_namespace_uri("p").as_deref(), Some("urn:one"));
    assert_eq!(leaf.lookup_namespace_uri("p").as_deref(), Some("urn:one"));
    assert_eq!(leaf.lookup_namespace_uri("q").as_deref(), Some("urn:two"));
    assert!(leaf.lookup_namespace_uri("zzz").is_none());
}

#[rstest]
fn document_builder_example() {
    let doc = simple_doc()
        .child(elem("root").attr(attr("id", "r")).child(text("Hello")).child(elem("inner").child(text("!"))))
        .build();
    let root = doc.children().next().unwrap();
    assert_eq!(root.string_value(), "Hello!");
}

#[rstest]
fn compare_different_roots_error() {
    let a = elem("x").build();
    let b = elem("y").build();
    assert_eq!(cmp(&a, &b), "!");
}

#[rstest]
fn build_simple() {
    let n = elem("root").attr(attr("id", "1")).child(elem("a").child(text("hi"))).child(elem("b")).build();
    assert_eq!(n.children().count(), 2);
    assert_eq!(n.attributes().count(), 1);
    let children: Vec<_> = n.children().collect();
    let a = &children[0];
    assert_eq!(a.string_value(), "hi");
    assert!(a.compare_document_order(&children[1]).is_ok());
}

#[rstest]
fn memoized_string_value_and_ns() {
    let root = simple_doc()
        .child(elem("root").namespace(ns("p", "urn:x")).child(elem("a").child(text("hi"))).child(text("!")))
        .build();
    let doc_child = root.children().next().unwrap();
    assert_eq!(doc_child.string_value(), "hi!");
    assert_eq!(doc_child.string_value(), "hi!");
    assert_eq!(doc_child.lookup_namespace_uri("p").as_deref(), Some("urn:x"));
}
