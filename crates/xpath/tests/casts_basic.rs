use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmAtomicValue as A, xdm::XdmItem as I};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;
fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

fn expect_err(expr: &str, frag: &str) {
    let c = ctx();
    let err = evaluate_expr::<N>(expr, &c).unwrap_err();
    assert!(err.code_qname().unwrap().local.contains(frag), "expected fragment {frag} in {:?}", err.code_qname());
}

#[rstest]
#[case("xs:boolean('true')", true)]
#[case("xs:boolean('1')", true)]
#[case("xs:boolean('false')", false)]
#[case("xs:boolean('0')", false)]
fn cast_boolean_lexical(#[case] expr: &str, #[case] expected: bool) {
    let c = ctx();
    let r = evaluate_expr::<N>(expr, &c).unwrap();
    assert_eq!(r.len(), 1);
    if let I::Atomic(A::Boolean(b)) = &r[0] {
        assert_eq!(*b, expected);
    } else {
        panic!("expected boolean");
    }
}

#[rstest]
fn cast_boolean_invalid() {
    expect_err("xs:boolean('yes')", "FORG0001");
}

#[rstest]
#[case("xs:integer('42')", 42)]
#[case("xs:integer(42)", 42)]
fn cast_integer_basic(#[case] expr: &str, #[case] expected: i64) {
    let c = ctx();
    let r = evaluate_expr::<N>(expr, &c).unwrap();
    if let I::Atomic(A::Integer(i)) = &r[0] {
        assert_eq!(*i, expected);
    } else {
        panic!("expected integer");
    }
}

#[rstest]
fn cast_integer_fraction_error() {
    expect_err("xs:integer(3.14)", "FOCA0001");
}

#[rstest]
#[case("xs:decimal(10)", 10.0)]
#[case("xs:decimal('10.5')", 10.5)]
fn cast_decimal_basic(#[case] expr: &str, #[case] expected: f64) {
    let c = ctx();
    let r = evaluate_expr::<N>(expr, &c).unwrap();
    if let I::Atomic(A::Decimal(d)) = &r[0] {
        assert!((*d - expected).abs() < 1e-9);
    } else {
        panic!("expected decimal");
    }
}

#[rstest]
fn cast_decimal_invalid() {
    expect_err("xs:decimal('abc')", "FORG0001");
}

#[rstest]
fn cast_anyuri_basic() {
    let c = ctx();
    let r = evaluate_expr::<N>("xs:anyURI('http://example.com')", &c).unwrap();
    if let I::Atomic(A::AnyUri(u)) = &r[0] {
        assert_eq!(u, "http://example.com");
    } else {
        panic!("expected anyURI");
    }
}

#[rstest]
fn cast_anyuri_empty_ok() {
    let c = ctx();
    let r = evaluate_expr::<N>("xs:anyURI('')", &c).unwrap();
    if let I::Atomic(A::AnyUri(u)) = &r[0] {
        assert_eq!(u, "");
    } else {
        panic!("expected anyURI");
    }
}

#[rstest]
#[case("xs:QName('local')", (None, "local"))]
fn cast_qname_lex(#[case] expr: &str, #[case] expected: (Option<&str>, &str)) {
    let c = ctx();
    let r = evaluate_expr::<N>(expr, &c).unwrap();
    if let I::Atomic(A::QName { prefix, local, .. }) = &r[0] {
        assert_eq!(prefix.as_deref(), expected.0);
        assert_eq!(local, expected.1);
    } else {
        panic!("expected QName");
    }
}

#[rstest]
fn cast_qname_invalid() {
    expect_err("xs:QName(':local')", "FORG0001");
}

#[rstest]
#[case("xs:yearMonthDuration('P2Y3M')", 27)]
#[case("xs:yearMonthDuration('P5M')", 5)]
fn cast_ym_duration(#[case] expr: &str, #[case] months: i32) {
    let c = ctx();
    let r = evaluate_expr::<N>(expr, &c).unwrap();
    if let I::Atomic(A::YearMonthDuration(m)) = &r[0] {
        assert_eq!(*m, months);
    } else {
        panic!("expected yearMonthDuration");
    }
}

#[rstest]
fn cast_ym_duration_invalid() {
    expect_err("xs:yearMonthDuration('PT5H')", "FORG0001");
}

#[rstest]
#[case("xs:dayTimeDuration('P1DT2H')", 86400 + 2*3600)]
#[case("xs:dayTimeDuration('PT30M')", 1800)]
fn cast_dt_duration(#[case] expr: &str, #[case] secs: i64) {
    let c = ctx();
    let r = evaluate_expr::<N>(expr, &c).unwrap();
    if let I::Atomic(A::DayTimeDuration(s)) = &r[0] {
        assert_eq!(*s, secs);
    } else {
        panic!("expected dayTimeDuration");
    }
}

#[rstest]
fn cast_dt_duration_invalid() {
    expect_err("xs:dayTimeDuration('P2Y')", "FORG0001");
}
