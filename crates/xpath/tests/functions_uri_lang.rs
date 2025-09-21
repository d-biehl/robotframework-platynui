use platynui_xpath::engine::runtime::{DynamicContextBuilder, StaticContextBuilder};
use platynui_xpath::{XdmItem, XdmNode, compiler::compile_with_context, evaluate, evaluate_expr};
use rstest::{fixture, rstest};

type N = platynui_xpath::model::simple::SimpleNode;

#[fixture]
fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::<N>::default().build()
}

#[fixture]
fn sc_base() -> platynui_xpath::engine::runtime::StaticContext {
    StaticContextBuilder::new().with_base_uri("http://example.com/base/").build()
}

#[rstest]
fn static_base_uri_reports_from_static_ctx(
    sc_base: platynui_xpath::engine::runtime::StaticContext,
    ctx: platynui_xpath::engine::runtime::DynamicContext<N>,
) {
    let compiled = compile_with_context("static-base-uri()", &sc_base).unwrap();
    let out = evaluate(&compiled, &ctx).unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::AnyUri(u)) => {
            assert_eq!(u, "http://example.com/base/")
        }
        _ => panic!("expected anyURI"),
    }
}

#[fixture]
fn sc_ex_x() -> platynui_xpath::engine::runtime::StaticContext {
    StaticContextBuilder::new().with_base_uri("http://ex/x/").build()
}

#[rstest]
fn resolve_uri_relative_join(
    sc_ex_x: platynui_xpath::engine::runtime::StaticContext,
    ctx: platynui_xpath::engine::runtime::DynamicContext<N>,
) {
    let compiled = compile_with_context("resolve-uri('a/b')", &sc_ex_x).unwrap();
    let out = evaluate(&compiled, &ctx).unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::AnyUri(u)) => {
            assert_eq!(u, "http://ex/x/a/b")
        }
        _ => panic!("expected anyURI"),
    }
}

#[rstest]
fn encode_for_uri(ctx: platynui_xpath::engine::runtime::DynamicContext<N>) {
    let enc = evaluate_expr::<N>("encode-for-uri('a b/β')", &ctx).unwrap();
    match &enc[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::String(s)) => {
            assert_eq!(s, "a%20b%2F%CE%B2")
        }
        _ => panic!("expected string"),
    }
}

#[rstest]
fn iri_to_uri(ctx: platynui_xpath::engine::runtime::DynamicContext<N>) {
    let iri = evaluate_expr::<N>("iri-to-uri('http://ex/ä')", &ctx).unwrap();
    match &iri[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::String(s)) => {
            assert!(s.ends_with("/%C3%A4"))
        }
        _ => panic!("expected string"),
    }
}

#[rstest]
fn escape_html_uri_spaces(ctx: platynui_xpath::engine::runtime::DynamicContext<N>) {
    let esc = evaluate_expr::<N>("escape-html-uri('http://ex/a b?c=d')", &ctx).unwrap();
    match &esc[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::String(s)) => {
            assert_eq!(s, "http://ex/a%20b?c=d")
        }
        _ => panic!("expected string"),
    }
}

#[rstest]
fn lang_matches_ancestor_xml_lang() {
    use platynui_xpath::model::simple::{doc, elem};
    let root = elem("root")
        .attr(platynui_xpath::model::simple::SimpleNode::attribute("xml:lang", "en-US"))
        .child(elem("child"))
        .build();
    let d = doc().child(root.clone()).build();
    let root = d.children().next().unwrap();
    let target = root.children().next().unwrap();
    let ctx = DynamicContextBuilder::<N>::default().with_context_item(target).build();
    let out = evaluate_expr::<N>("lang('en')", &ctx).unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("expected boolean"),
    }
}
