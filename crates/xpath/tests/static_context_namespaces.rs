use platynui_xpath::simple_node::{doc, elem, ns, text};
use platynui_xpath::{
    XdmItem, XdmNode,
    compiler::compile_with_context,
    evaluate,
    runtime::{DynamicContextBuilder, StaticContextBuilder},
};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;

fn dyn_ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

#[rstest]
fn register_and_use_custom_prefix_in_qname_constructor() {
    let static_ctx = StaticContextBuilder::new().with_namespace("ex", "urn:example").build();
    let ctx = dyn_ctx();
    let compiled = compile_with_context("xs:QName('ex:local')", &static_ctx).unwrap();
    let seq = evaluate(&compiled, &ctx).unwrap();
    assert_eq!(seq.len(), 1);
    let atom = match &seq[0] {
        XdmItem::Atomic(a) => a,
        _ => panic!("expected atomic"),
    };
    if let platynui_xpath::xdm::XdmAtomicValue::QName { ns_uri, local, .. } = atom {
        assert_eq!(ns_uri.as_deref(), Some("urn:example"));
        assert_eq!(local, "local");
    } else {
        panic!("not qname");
    }
}

#[rstest]
fn resolve_qname_uses_static_namespace() {
    let static_ctx = StaticContextBuilder::new().with_namespace("p", "urn:ns").build();
    let ctx = dyn_ctx();
    let compiled = compile_with_context("namespace-uri-from-QName(xs:QName('p:thing'))", &static_ctx).unwrap();
    let seq = evaluate(&compiled, &ctx).unwrap();
    let uri = match &seq[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::AnyUri(s)) => s,
        _ => panic!("expected anyURI"),
    };
    assert_eq!(uri, "urn:ns");
}

#[rstest]
fn implicit_xml_binding_present_and_not_overridden() {
    let static_ctx = StaticContextBuilder::new().with_namespace("xml", "urn:override").build();
    let ctx = dyn_ctx();
    let compiled = compile_with_context("namespace-uri-from-QName(xs:QName('xml:lang'))", &static_ctx).unwrap();
    let seq = evaluate(&compiled, &ctx).unwrap();
    let uri = match &seq[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::AnyUri(s)) => s,
        _ => panic!("expected anyURI"),
    };
    assert_eq!(uri, "http://www.w3.org/XML/1998/namespace");
}

#[rstest]
fn unknown_prefix_still_errors_without_registration() {
    let static_ctx = StaticContextBuilder::new().build();
    let ctx = dyn_ctx();
    let compiled = compile_with_context("xs:QName('u:local')", &static_ctx).unwrap();
    let err = evaluate(&compiled, &ctx).expect_err("expected error");
    assert_eq!(err.code_enum(), platynui_xpath::engine::runtime::ErrorCode::FORG0001);
}

#[rstest]
fn resolve_qname_uses_element_inscope_not_static() {
    // Static context defines prefix 'a' to urn:static, but element defines xmlns:a="urn:elem".
    // According to spec, resolve-QName should use the element's in-scope namespaces (not static context).
    let static_ctx = StaticContextBuilder::new().with_namespace("a", "urn:static").build();
    // Build document/root with namespace declaration a -> urn:elem and a child element
    let document =
        doc().child(elem("root").namespace(ns("a", "urn:elem")).child(elem("child").child(text("content")))).build();
    let dyn_ctx = DynamicContextBuilder::default().with_context_item(document.clone()).build();
    // Expression: resolve-QName('a:child', root/child) then namespace-uri-from-QName
    let expr = "namespace-uri-from-QName(resolve-QName('a:child', /root/child))";
    let compiled = compile_with_context(expr, &static_ctx).unwrap();
    let seq = evaluate(&compiled, &dyn_ctx).unwrap();
    let uri = match &seq[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::AnyUri(s)) => s,
        _ => panic!("expected anyURI"),
    };
    assert_eq!(uri, "urn:elem"); // proves element in-scope took precedence
}

#[rstest]
fn default_element_namespace_applies_to_unprefixed_steps() {
    let static_ctx = StaticContextBuilder::new().with_default_element_namespace("urn:def").build();

    let document = doc()
        .child(
            elem("root")
                .namespace(ns("", "urn:def"))
                .namespace(ns("p", "urn:def"))
                .child(elem("p:foo").child(text("hit")))
                .child(elem("other").child(text("skip"))),
        )
        .build();

    let dyn_ctx = DynamicContextBuilder::default().with_context_item(document.clone()).build();

    let compiled = compile_with_context("//foo", &static_ctx).unwrap();
    let seq = evaluate(&compiled, &dyn_ctx).unwrap();
    assert_eq!(seq.len(), 1);
    let node = match &seq[0] {
        XdmItem::Node(n) => n.clone(),
        _ => panic!("expected node"),
    };
    assert_eq!(node.name().unwrap().local, "foo");
    assert_eq!(node.lookup_namespace_uri(""), Some("urn:def".to_string()));
}
