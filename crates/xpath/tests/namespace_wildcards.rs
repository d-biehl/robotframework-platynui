use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder, StaticContextBuilder};
use platynui_xpath::model::simple::{attr as mkattr, doc, elem, ns};
use platynui_xpath::{XdmNode, compiler::compile_with_context, evaluate, xdm::XdmItem as I};

type N = platynui_xpath::model::simple::SimpleNode;

fn ctx_with(root: N) -> DynamicContext<N> {
    DynamicContextBuilder::default().with_context_item(I::Node(root)).build()
}

#[test]
fn element_namespace_wildcards() {
    // <root xmlns:p="urn:one" xmlns:q="urn:two">
    //   <p:a/><q:b/><c/>
    // </root>
    let d = doc()
        .child(
            elem("root")
                .namespace(ns("p", "urn:one"))
                .namespace(ns("q", "urn:two"))
                .child(elem("p:a"))
                .child(elem("q:b"))
                .child(elem("c")),
        )
        .build();
    let root = d.children().next().unwrap();
    let ctx = ctx_with(root.clone());
    // In XPath 2.0, prefixes in NameTests (like p:* or q:*) are resolved using the static context.
    // Provide static prefix bindings here to map p->urn:one and q->urn:two.
    let static_ctx = StaticContextBuilder::new().with_namespace("p", "urn:one").with_namespace("q", "urn:two").build();

    // child::p:* should match only elements in ns urn:one under root (p:a)
    let compiled = compile_with_context("child::p:*", &static_ctx).unwrap();
    let out = evaluate(&compiled, &ctx).unwrap();
    assert_eq!(out.len(), 1);
    match &out[0] {
        I::Node(n) => {
            let q = n.name().unwrap();
            assert_eq!(q.local, "a");
            // Some models may leave ns_uri unset on creation and resolve via in-scope namespaces.
            // Ensure the effective namespace URI resolves to urn:one.
            let effective = q.ns_uri.or_else(|| n.lookup_namespace_uri(q.prefix.as_deref().unwrap_or("")));
            assert_eq!(effective.as_deref(), Some("urn:one"));
        }
        _ => panic!("expected node"),
    }

    // child::*:b should match element with local b regardless of prefix (q:b)
    let compiled = compile_with_context("child::*:b", &static_ctx).unwrap();
    let out = evaluate(&compiled, &ctx).unwrap();
    assert_eq!(out.len(), 1);
    match &out[0] {
        I::Node(n) => {
            let q = n.name().unwrap();
            assert_eq!(q.local, "b");
        }
        _ => panic!("expected node"),
    }
}

#[test]
fn attribute_namespace_wildcards() {
    // <root xmlns:p="urn:one" xmlns:q="urn:two">
    //   <item p:x="1" q:y="2" z="3"/>
    // </root>
    let d = doc()
        .child(
            elem("root")
                .namespace(ns("p", "urn:one"))
                .namespace(ns("q", "urn:two"))
                .child(elem("item").attr(mkattr("p:x", "1")).attr(mkattr("q:y", "2")).attr(mkattr("z", "3"))),
        )
        .build();
    let root = d.children().next().unwrap();
    let ctx = ctx_with(root.children().next().unwrap()); // context: <item>
    // Bind namespace prefixes for attribute wildcards as well
    let static_ctx = StaticContextBuilder::new().with_namespace("p", "urn:one").with_namespace("q", "urn:two").build();

    // @p:* should return attribute in ns urn:one (p:x)
    let compiled = compile_with_context("@p:*", &static_ctx).unwrap();
    let out = evaluate(&compiled, &ctx).unwrap();
    assert_eq!(out.len(), 1);
    match &out[0] {
        I::Node(n) => {
            let q = n.name().unwrap();
            assert_eq!(q.local, "x");
            assert_eq!(q.ns_uri.as_deref(), Some("urn:one"));
        }
        _ => panic!("expected node"),
    }

    // @*:y should return q:y regardless of prefix
    let compiled = compile_with_context("@*:y", &static_ctx).unwrap();
    let out = evaluate(&compiled, &ctx).unwrap();
    assert_eq!(out.len(), 1);
    match &out[0] {
        I::Node(n) => {
            let q = n.name().unwrap();
            assert_eq!(q.local, "y");
        }
        _ => panic!("expected node"),
    }
}
