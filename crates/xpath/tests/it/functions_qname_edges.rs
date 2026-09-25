use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::{engine::evaluator::evaluate_expr, model::XdmNode, xdm::XdmAtomicValue as A, xdm::XdmItem as I};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;

fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

// Helper to assert error code contains fragment
fn expect_err(expr: &str, frag: &str) {
    let c = ctx();
    let err = evaluate_expr::<N>(expr, &c).unwrap_err();
    assert!(err.code_qname().unwrap().local.contains(frag), "expected fragment {frag} in {:?}", err.code_qname());
}

#[rstest]
fn accessors_on_unprefixed_qname() {
    let c = ctx();
    // namespace-uri-from-QName on no-namespace returns empty sequence per spec
    let u = evaluate_expr::<N>("namespace-uri-from-QName(QName('', 'local'))", &c).unwrap();
    assert!(u.is_empty(), "expected empty sequence for missing namespace URI");
    // prefix-from-QName on unprefixed returns empty sequence
    let p = evaluate_expr::<N>("prefix-from-QName(QName('', 'local'))", &c).unwrap();
    assert!(p.is_empty(), "expected empty sequence for missing prefix");
}

#[rstest]
#[case("xs:QName('')")] // empty string
#[case("xs:QName('p:')")] // missing local part
#[case("xs:QName(':l')")] // missing prefix
#[case("xs:QName('xml:')")] // missing local part after xml:
fn xs_qname_invalid_forms(#[case] expr: &str) {
    expect_err(expr, "FORG0001");
}

#[rstest]
fn xs_qname_xmlns_current_behavior() {
    // Prefix xmlns is invalid, but bare 'xmlns' as local name is accepted by current implementation.
    expect_err("xs:QName('xmlns:foo')", "FORG0001");
    let c = ctx();
    let ok = evaluate_expr::<N>("xs:QName('xmlns')", &c).unwrap();
    assert_eq!(ok.len(), 1);
    if let I::Atomic(A::QName { prefix, local, .. }) = &ok[0] {
        assert!(prefix.is_none());
        assert_eq!(local, "xmlns");
    } else {
        panic!("expected QName");
    }
}

#[rstest]
fn qname_function_prefixed_with_empty_ns_current_behavior() {
    let c = ctx();
    let r = evaluate_expr::<N>("QName('', 'p:l')", &c).unwrap();
    assert_eq!(r.len(), 1);
    if let I::Atomic(A::QName { ns_uri, prefix, local }) = &r[0] {
        assert!(ns_uri.is_none());
        assert_eq!(prefix.as_deref(), Some("p"));
        assert_eq!(local, "l");
    } else {
        panic!("expected QName");
    }
}

#[rstest]
#[case("resolve-QName('p:', .)")] // missing local
#[case("resolve-QName(':l', .)")] // missing prefix
#[case("resolve-QName('xmlns:l', .)")] // reserved prefix currently accepted? if implementation accepts, treat as unknown -> error
fn resolve_qname_invalid_lex(#[case] expr: &str) {
    // Build minimal context item (element) since resolve-QName needs a node context
    use platynui_xpath::model::simple::elem;
    let doc = platynui_xpath::simple_doc().child(elem("root")).build();
    let root = doc.children().next().unwrap();
    let c = DynamicContextBuilder::default().with_context_item(root).build();
    let err = evaluate_expr::<N>(expr, &c).unwrap_err();
    assert_eq!(
        err.code_enum(),
        platynui_xpath::engine::runtime::ErrorCode::FORG0001,
        "expected FORG0001 style lexical error, got {:?}",
        err.code_qname()
    );
}

#[rstest]
fn qname_roundtrip_basic() {
    let c = ctx();
    // construct then decompose; absence of ns/prefix yields empty sequences
    let seq = evaluate_expr::<N>("(QName('urn:rt','rt:local'), QName('', 'local'))", &c).unwrap();
    assert_eq!(seq.len(), 2);
    // First: check accessors yield same pieces
    let ns_rt = evaluate_expr::<N>("namespace-uri-from-QName(QName('urn:rt','rt:local'))", &c).unwrap();
    assert_eq!(ns_rt, vec![I::Atomic(A::AnyUri("urn:rt".into()))]);
    let pref_rt = evaluate_expr::<N>("prefix-from-QName(QName('urn:rt','rt:local'))", &c).unwrap();
    assert_eq!(pref_rt, vec![I::Atomic(A::NCName("rt".into()))]);
    let loc_rt = evaluate_expr::<N>("local-name-from-QName(QName('urn:rt','rt:local'))", &c).unwrap();
    assert_eq!(loc_rt, vec![I::Atomic(A::NCName("local".into()))]);
    // Second: no namespace
    let ns_empty = evaluate_expr::<N>("namespace-uri-from-QName(QName('', 'local'))", &c).unwrap();
    assert!(ns_empty.is_empty());
    let pref_empty = evaluate_expr::<N>("prefix-from-QName(QName('', 'local'))", &c).unwrap();
    assert!(pref_empty.is_empty());
}
