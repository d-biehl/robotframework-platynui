use platynui_xpath::simple_node::{attr, elem, ns, text};
use platynui_xpath::xdm::XdmAtomicValue;
use platynui_xpath::{XdmItem as I, XdmNode, evaluate_expr, runtime::DynamicContextBuilder};
use rstest::rstest;

type N = platynui_xpath::simple_node::SimpleNode;

fn ctx_with_tree() -> platynui_xpath::runtime::DynamicContext<N> {
    // <root xmlns:p="urn:one" a="1"><p:child id="c1">Hi</p:child><child/></root>
    let root = elem("root")
        .namespace(ns("p", "urn:one"))
        .attr(attr("a", "1"))
        .child(elem("child").attr(attr("id", "c1")).child(text("Hi")))
        .child(elem("child"))
        .build();
    DynamicContextBuilder::new().with_context_item(root).build()
}

#[rstest]
fn node_name_and_local_namespace() {
    let ctx = ctx_with_tree();
    // node-name on element
    let out = evaluate_expr::<N>("node-name(.)", &ctx).unwrap();
    assert_eq!(out.len(), 1);
    let I::Atomic(XdmAtomicValue::QName {
        ns_uri,
        prefix,
        local,
    }) = &out[0]
    else {
        panic!("QName expected")
    };
    assert_eq!(prefix, &None);
    assert_eq!(local, "root");
    assert_eq!(ns_uri, &None);

    // name/local-name/namespace-uri on attribute node
    let out = evaluate_expr::<N>("name(@a)", &ctx).unwrap();
    assert_eq!(out[0].to_string(), "String(\"a\")");
    let out = evaluate_expr::<N>("local-name(@a)", &ctx).unwrap();
    assert_eq!(out[0].to_string(), "String(\"a\")");
    let out = evaluate_expr::<N>("namespace-uri(@a)", &ctx).unwrap();
    // attributes in no namespace unless explicitly bound -> empty sequence per spec
    assert!(out.is_empty());
}

#[rstest]
fn empty_and_unnamed_nodes() {
    // Empty sequence
    let ctx = ctx_with_tree();
    let out = evaluate_expr::<N>("node-name(())", &ctx).unwrap();
    assert!(out.is_empty());

    // Text node has no name
    let root = elem("r").child(text("t")).build();
    let ctx = DynamicContextBuilder::new()
        .with_context_item(root.children()[0].clone())
        .build();
    let out = evaluate_expr::<N>("name(.)", &ctx).unwrap();
    assert_eq!(out[0].to_string(), "String(\"\")");
    let out = evaluate_expr::<N>("local-name(.)", &ctx).unwrap();
    assert_eq!(out[0].to_string(), "String(\"\")");
    let out = evaluate_expr::<N>("namespace-uri(.)", &ctx).unwrap();
    assert!(out.is_empty());
}

#[rstest]
fn prefixed_and_namespace_nodes() {
    // Create element with namespace node and prefixed child
    let doc = platynui_xpath::simple_doc()
        .child(
            elem("root")
                .namespace(ns("p", "urn:one"))
                .child(elem("child")),
        )
        .build();
    let root = doc.children()[0].clone();
    let ctx = DynamicContextBuilder::new()
        .with_context_item(root.clone())
        .build();

    // namespace-uri for element with no ns is empty
    let out = evaluate_expr::<N>("namespace-uri(.)", &ctx).unwrap();
    assert!(out.is_empty());

    // Namespace axis returns namespace nodes with prefix in name(), but namespace-uri(name()) is empty (name of namespace node is prefix)
    let out = evaluate_expr::<N>("name(namespace::p)", &ctx).unwrap();
    assert_eq!(out[0].to_string(), "String(\"p\")");
    let out = evaluate_expr::<N>("namespace-uri(namespace::p)", &ctx).unwrap();
    // Namespace nodes have no QName -> empty sequence per fn:namespace-uri definition
    assert!(out.is_empty());
}
