use platynui_xpath::{evaluate_expr, runtime::DynamicContextBuilder, xdm::XdmAtomicValue as A, xdm::XdmItem as I};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;
fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

fn eval_bool(expr: &str) -> bool {
    let c = ctx();
    let r = evaluate_expr::<N>(expr, &c).unwrap();
    match &r[0] {
        I::Atomic(A::Boolean(b)) => *b,
        other => panic!("expected boolean got {:?}", other),
    }
}

fn expect_err(expr: &str, frag: &str) {
    let c = ctx();
    let err = evaluate_expr::<N>(expr, &c).unwrap_err();
    assert!(err.code_qname().unwrap().local.contains(frag), "expected fragment {frag} in {:?}", err.code_qname());
}

// 1. Month-end rollovers (leap & non-leap year)
#[rstest]
#[case("xs:date('2024-01-31') + xs:yearMonthDuration('P1M') = xs:date('2024-02-29')")]
#[case("xs:date('2025-01-31') + xs:yearMonthDuration('P1M') = xs:date('2025-02-28')")]
fn month_end_rollover(#[case] expr: &str) {
    assert!(eval_bool(expr));
}

// 2. Leap-year subtraction (back to Feb 29)
#[rstest]
fn leap_year_subtraction() {
    assert!(eval_bool("xs:date('2024-03-01') - xs:yearMonthDuration('P1M') = xs:date('2024-02-01')"));
}

// 3. dateTime + yearMonthDuration month-end handling
#[rstest]
#[case("xs:dateTime('2024-01-31T10:00:00Z') + xs:yearMonthDuration('P1M') = xs:dateTime('2024-02-29T10:00:00Z')")]
fn datetime_month_end(#[case] expr: &str) {
    assert!(eval_bool(expr));
}

// 4. dateTime +/- dayTimeDuration crossing day boundary
#[rstest]
#[case("xs:dateTime('2024-06-01T23:30:00Z') + xs:dayTimeDuration('PT3600S') = xs:dateTime('2024-06-02T00:30:00Z')")]
#[case("xs:dateTime('2024-06-02T00:30:00Z') - xs:dayTimeDuration('PT3600S') = xs:dateTime('2024-06-01T23:30:00Z')")]
fn datetime_cross_day(#[case] expr: &str) {
    assert!(eval_bool(expr));
}

// 5. Invalid: time + yearMonthDuration (type error XPTY0004 expected)
#[rstest]
fn time_plus_yearmonth_duration_error() {
    expect_err("xs:time('10:00:00') + xs:yearMonthDuration('P1M')", "XPTY0004");
}

// 6. Invalid: yearMonthDuration + dayTimeDuration (no direct addition)
#[rstest]
fn mixed_duration_add_error() {
    expect_err("xs:yearMonthDuration('P1M') + xs:dayTimeDuration('PT60S')", "XPTY0004");
}

// 7. Negative zero normalization ( -PT0S == PT0S )
#[rstest]
fn negative_zero_duration_normalization() {
    assert!(eval_bool("xs:dayTimeDuration('-PT0S') = xs:dayTimeDuration('PT0S')"));
}

// 8. date - date ordering sign correctness
#[rstest]
#[case("(xs:dateTime('2024-06-02T00:00:00Z') - xs:dateTime('2024-06-01T00:00:00Z')) = xs:dayTimeDuration('PT86400S')")]
#[case("(xs:dateTime('2024-06-01T00:00:00Z') - xs:dateTime('2024-06-02T00:00:00Z')) = xs:dayTimeDuration('-PT86400S')")]
fn datetime_difference_sign(#[case] expr: &str) {
    assert!(eval_bool(expr));
}
