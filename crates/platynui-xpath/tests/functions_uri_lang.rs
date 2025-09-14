use platynui_xpath::engine::runtime::{DynamicContextBuilder, StaticContextBuilder};
use platynui_xpath::{
    XdmItem, XdmNode, compiler::compile_xpath_with_context, evaluate, evaluate_expr,
};

type N = platynui_xpath::model::simple::SimpleNode;

#[test]
fn static_base_uri_reports_from_static_ctx() {
    let sc = StaticContextBuilder::new()
        .with_base_uri("http://example.com/base/")
        .build();
    let compiled = compile_xpath_with_context("static-base-uri()", &sc).unwrap();
    let ctx = DynamicContextBuilder::<N>::default().build();
    let out = evaluate(&compiled, &ctx).unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::AnyUri(u)) => {
            assert_eq!(u, "http://example.com/base/")
        }
        _ => panic!("expected anyURI"),
    }
}

#[test]
fn resolve_uri_relative_join() {
    let sc = StaticContextBuilder::new()
        .with_base_uri("http://ex/x/")
        .build();
    let compiled = compile_xpath_with_context("resolve-uri('a/b')", &sc).unwrap();
    let ctx = DynamicContextBuilder::<N>::default().build();
    let out = evaluate(&compiled, &ctx).unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::AnyUri(u)) => {
            assert_eq!(u, "http://ex/x/a/b")
        }
        _ => panic!("expected anyURI"),
    }
}

#[test]
fn encode_and_iri_to_uri() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let enc = evaluate_expr::<N>("encode-for-uri('a b/β')", &ctx).unwrap();
    match &enc[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::String(s)) => {
            assert_eq!(s, "a%20b/%CE%B2")
        }
        _ => panic!("expected string"),
    }
    let iri = evaluate_expr::<N>("iri-to-uri('http://ex/ä')", &ctx).unwrap();
    match &iri[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::String(s)) => {
            assert!(s.ends_with("/%C3%A4"))
        }
        _ => panic!("expected string"),
    }
}

#[test]
fn escape_html_uri_spaces() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let esc = evaluate_expr::<N>("escape-html-uri('http://ex/a b?c=d')", &ctx).unwrap();
    match &esc[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::String(s)) => {
            assert_eq!(s, "http://ex/a%20b?c=d")
        }
        _ => panic!("expected string"),
    }
}

#[test]
fn lang_matches_ancestor_xml_lang() {
    use platynui_xpath::model::simple::{doc, elem};
    let root = elem("root")
        .attr(platynui_xpath::model::simple::SimpleNode::attribute(
            "xml:lang", "en-US",
        ))
        .child(elem("child"))
        .build();
    let d = doc().child(root.clone()).build();
    let ctx = DynamicContextBuilder::<N>::default()
        .with_context_item(d.children()[0].children()[0].clone())
        .build();
    let out = evaluate_expr::<N>("lang('en')", &ctx).unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("expected boolean"),
    }
}
