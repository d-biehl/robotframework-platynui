use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::model::simple::{elem, ns};
use platynui_xpath::runtime::ErrorCode;
use platynui_xpath::{engine::evaluator::evaluate_expr, model::XdmNode, xdm::XdmAtomicValue as A, xdm::XdmItem as I};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;

fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::new().build()
}

#[rstest]
fn qname_construction_and_from_parts() {
    let c = ctx();
    // QName(namespace, lex)
    let q = evaluate_expr::<N>("QName('urn:x','p:l')", &c).unwrap();
    assert_eq!(
        q,
        vec![I::Atomic(A::QName { ns_uri: Some("urn:x".into()), prefix: Some("p".into()), local: "l".into() })]
    );
    // empty namespace
    let q2 = evaluate_expr::<N>("QName('', 'local')", &c).unwrap();
    assert_eq!(q2, vec![I::Atomic(A::QName { ns_uri: None, prefix: None, local: "local".into() })]);
}

#[rstest]
fn resolve_qname_and_in_scope() {
    // <root xmlns:p="urn:one"><p:c/></root>
    let doc = platynui_xpath::simple_doc().child(elem("root").namespace(ns("p", "urn:one")).child(elem("c"))).build();
    let root = doc.children().next().unwrap();
    let ctx = DynamicContextBuilder::new().with_context_item(root.clone()).build();
    // resolve-QName with element
    let r = evaluate_expr::<N>("resolve-QName('p:l', .)", &ctx).unwrap();
    assert_eq!(
        r,
        vec![I::Atomic(A::QName { ns_uri: Some("urn:one".into()), prefix: Some("p".into()), local: "l".into() })]
    );
    // unknown prefix errors
    let err = evaluate_expr::<N>("resolve-QName('zzz:l', .)", &ctx).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::FORG0001);
    // namespace-uri-for-prefix
    let u = evaluate_expr::<N>("namespace-uri-for-prefix('p', .)", &ctx).unwrap();
    assert_eq!(u, vec![I::Atomic(A::AnyUri("urn:one".into()))]);
    // in-scope-prefixes includes xml
    let v = evaluate_expr::<N>("in-scope-prefixes(.)", &ctx).unwrap();
    let mut prefixes: Vec<String> =
        v.iter().filter_map(|i| if let I::Atomic(A::NCName(s)) = i { Some(s.clone()) } else { None }).collect();
    prefixes.sort();
    assert!(prefixes.contains(&"p".to_string()));
    assert!(prefixes.contains(&"xml".to_string()));
}

#[rstest]
fn qname_accessors() {
    let c = ctx();
    // namespace-uri-from-QName
    let u = evaluate_expr::<N>("namespace-uri-from-QName((QName('urn:a','p:l')))", &c).unwrap();
    assert_eq!(u, vec![I::Atomic(A::AnyUri("urn:a".into()))]);
    // local-name-from-QName
    let l = evaluate_expr::<N>("local-name-from-QName(QName('urn:a','p:l'))", &c).unwrap();
    assert_eq!(l, vec![I::Atomic(A::NCName("l".into()))]);
    // prefix-from-QName
    let p = evaluate_expr::<N>("prefix-from-QName(QName('urn:a','p:l'))", &c).unwrap();
    assert_eq!(p, vec![I::Atomic(A::NCName("p".into()))]);
}
