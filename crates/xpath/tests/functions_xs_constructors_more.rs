use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder};
use platynui_xpath::runtime::ErrorCode;
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmAtomicValue as A, xdm::XdmItem as I};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;

fn empty_ctx() -> DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

#[rstest]
#[case("xs:decimal('12.5')", I::Atomic(A::Decimal(12.5)))]
#[case("xs:float('INF')", I::Atomic(A::Float(f32::INFINITY)))]
#[case("xs:double('-INF')", I::Atomic(A::Double(f64::NEG_INFINITY)))]
fn xs_numeric_family_basic(#[case] expr: &str, #[case] expected: I<N>) {
    let c = empty_ctx();
    let out = evaluate_expr::<N>(expr, &c).unwrap();
    match (expr, &out[0], &expected) {
        (e, I::Atomic(A::Float(v)), I::Atomic(A::Float(ev))) if e.contains("float") => {
            assert!(v.is_infinite() && v.is_sign_positive() && ev.is_infinite())
        }
        (e, I::Atomic(A::Double(v)), I::Atomic(A::Double(ev))) if e.contains("double") => {
            assert!(v.is_infinite() && v.is_sign_negative() && ev.is_infinite())
        }
        _ => assert_eq!(out[0].to_string(), expected.to_string()),
    }
}

#[rstest]
fn xs_anyuri_and_qname() {
    let ctx = empty_ctx();
    let u = evaluate_expr::<N>("xs:anyURI('  http://example.com/a ')", &ctx).unwrap();
    assert_eq!(u, vec![I::Atomic(A::AnyUri("http://example.com/a".into()))]);
    let qxml = evaluate_expr::<N>("xs:QName('xml:lang')", &ctx).unwrap();
    assert_eq!(
        qxml,
        vec![I::Atomic(A::QName {
            ns_uri: Some("http://www.w3.org/XML/1998/namespace".into()),
            prefix: Some("xml".into()),
            local: "lang".into()
        })]
    );
    let qunp = evaluate_expr::<N>("xs:QName('local')", &ctx).unwrap();
    assert_eq!(qunp, vec![I::Atomic(A::QName { ns_uri: None, prefix: None, local: "local".into() })]);
}

#[rstest]
fn xs_qname_unknown_prefix_error() {
    let err = evaluate_expr::<N>("xs:QName('zzz:l')", &empty_ctx()).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::FORG0001);
}

#[rstest]
#[case("xs:base64Binary(' YWJj ')", I::Atomic(A::Base64Binary("YWJj".into())))]
#[case("xs:hexBinary('0A0b')", I::Atomic(A::HexBinary("0A0b".into())))]
fn xs_binary_constructors(#[case] expr: &str, #[case] expected: I<N>) {
    let c = empty_ctx();
    let out = evaluate_expr::<N>(expr, &c).unwrap();
    assert_eq!(out, vec![expected]);
}

#[rstest]
fn xs_hex_invalid() {
    let err = evaluate_expr::<N>("xs:hexBinary('abc')", &empty_ctx()).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::FORG0001);
}

#[rstest]
fn xs_dates_times_and_durations() {
    let c = empty_ctx();
    let dt = evaluate_expr::<N>("xs:dateTime('2020-01-02T03:04:05+02:00')", &c).unwrap();
    assert_eq!(dt.len(), 1);
    let d = evaluate_expr::<N>("xs:date('2020-01-02Z')", &c).unwrap();
    assert_eq!(d.len(), 1);
    let t = evaluate_expr::<N>("xs:time('03:04:05-05:00')", &c).unwrap();
    assert_eq!(t.len(), 1);
    let ymd = evaluate_expr::<N>("xs:yearMonthDuration('P1Y2M')", &c).unwrap();
    assert_eq!(ymd, vec![I::Atomic(A::YearMonthDuration(14))]);
    let dtd = evaluate_expr::<N>("xs:dayTimeDuration('P1DT2H3M4S')", &c).unwrap();
    assert_eq!(dtd, vec![I::Atomic(A::DayTimeDuration(24 * 3600 + 2 * 3600 + 3 * 60 + 4))]);
}

#[rstest]
fn xs_integer_subtypes_ranges() {
    let c = empty_ctx();
    let ok = evaluate_expr::<N>("xs:unsignedByte('255')", &c).unwrap();
    assert_eq!(ok, vec![I::Atomic(A::UnsignedByte(255))]);
    let err = evaluate_expr::<N>("xs:unsignedByte('256')", &c).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::FORG0001);
}

#[rstest]
fn xs_string_derived() {
    let c = empty_ctx();
    let n = evaluate_expr::<N>("xs:normalizedString('\tA\nB')", &c).unwrap();
    assert_eq!(n, vec![I::Atomic(A::NormalizedString(" A B".into()))]);
    let tok = evaluate_expr::<N>("xs:token('  a   b  ')", &c).unwrap();
    assert_eq!(tok, vec![I::Atomic(A::Token("a b".into()))]);
}
