use platynui_xpath::{
    engine::evaluator::evaluate_expr,
    runtime::DynamicContextBuilder,
    xdm::{XdmAtomicValue, XdmItem},
};
use rstest::rstest;

// Helper to evaluate with default dynamic context
fn eval(expr: &str) -> Vec<XdmItem<platynui_xpath::model::simple::SimpleNode>> {
    let ctx = DynamicContextBuilder::default().build();
    evaluate_expr(expr, &ctx).expect("evaluation error")
}

#[rstest]
fn fractional_second_equality_and_ordering(#[values("2024-01-01T00:00:00", "2024-12-31T23:59:59")] base: &str) {
    let lt_expr = format!("xs:dateTime('{base}.123Z') lt xs:dateTime('{base}.124Z')");
    let r = eval(&lt_expr);
    assert!(matches!(&r[0], XdmItem::Atomic(XdmAtomicValue::Boolean(true))));
    let eq_expr = format!("xs:dateTime('{base}.123Z') eq xs:dateTime('{base}.123Z')");
    let r2 = eval(&eq_expr);
    assert!(matches!(&r2[0], XdmItem::Atomic(XdmAtomicValue::Boolean(true))));
}

#[rstest]
fn timezone_normalization_equality(#[values("2024-06-01", "2024-02-29")] day: &str) {
    // 10:00Z vs 11:00+01:00 same instant
    let expr = format!("xs:dateTime('{day}T10:00:00Z') eq xs:dateTime('{day}T11:00:00+01:00')");
    let r = eval(&expr);
    assert!(matches!(&r[0], XdmItem::Atomic(XdmAtomicValue::Boolean(true))));
}

#[rstest]
fn timezone_boundaries_plus14_minus14() {
    // +14:00 boundary: 2024-01-01T00:00:00+14:00 == 2023-12-31T10:00:00Z
    let r1 = eval("xs:dateTime('2024-01-01T00:00:00+14:00') eq xs:dateTime('2023-12-31T10:00:00Z')");
    assert!(matches!(&r1[0], XdmItem::Atomic(XdmAtomicValue::Boolean(true))));
    // -14:00 boundary: 2024-01-01T00:00:00-14:00 == 2024-01-01T14:00:00Z
    let r2 = eval("xs:dateTime('2024-01-01T00:00:00-14:00') eq xs:dateTime('2024-01-01T14:00:00Z')");
    assert!(matches!(&r2[0], XdmItem::Atomic(XdmAtomicValue::Boolean(true))));
}

#[rstest]
#[case("P1D", "2024-06-10T00:00:00Z", "2024-06-09T00:00:00Z")]
#[case("P2D", "2024-06-10T00:00:00Z", "2024-06-08T00:00:00Z")]
fn negative_duration_addition_and_subtraction(#[case] dur: &str, #[case] start: &str, #[case] expected: &str) {
    let expr = format!("(xs:dateTime('{start}') - xs:dayTimeDuration('{dur}')) eq xs:dateTime('{expected}')");
    let r = eval(&expr);
    assert!(matches!(&r[0], XdmItem::Atomic(XdmAtomicValue::Boolean(true))));
}

#[rstest]
fn duration_arithmetic_mixed_year_month_and_day_time(#[values("P6M")] a: &str, #[values("P3M")] b: &str) {
    let expr = format!("xs:yearMonthDuration('{a}') div xs:yearMonthDuration('{b}')");
    let r = eval(&expr);
    match &r[0] {
        XdmItem::Atomic(XdmAtomicValue::Double(v)) => assert_eq!(*v, 2.0),
        _ => panic!("expected double 2.0"),
    }
}

#[rstest]
#[case("12:00:00")] // simple midday
#[case("00:00:00")] // midnight edge
fn time_comparison_implicit_timezone(#[case] t: &str) {
    let expr = format!("xs:time('{t}Z') eq xs:time('{t}')");
    let r = eval(&expr);
    assert!(matches!(&r[0], XdmItem::Atomic(XdmAtomicValue::Boolean(true))));
}

#[rstest]
fn time_cross_timezone_equality() {
    // 10:00:00+01:30 == 08:30:00Z
    let r = eval("xs:time('10:00:00+01:30') eq xs:time('08:30:00Z')");
    assert!(matches!(&r[0], XdmItem::Atomic(XdmAtomicValue::Boolean(true))));
}

#[rstest]
#[case("2024-01-01T00:00:00Z", "PT3H5M")]
#[case("2024-06-15T12:30:00Z", "PT1H30M")]
fn date_time_duration_round_trip(#[case] base: &str, #[case] dur: &str) {
    let expr = format!(
        "((xs:dateTime('{base}') + xs:dayTimeDuration('{dur}')) - xs:dayTimeDuration('{dur}')) eq xs:dateTime('{base}')"
    );
    let r = eval(&expr);
    assert!(matches!(&r[0], XdmItem::Atomic(XdmAtomicValue::Boolean(true))));
}

#[rstest]
fn date_time_duration_round_trip_matrix(
    #[values(0, 1, 3, 5)] hours: u32,
    #[values(0, 1, 10, 30, 58, 59)] minutes: u32,
) {
    let base = "2024-01-01T00:00:00Z";
    let dur = format!("PT{hours}H{minutes}M");
    let expr = format!(
        "((xs:dateTime('{base}') + xs:dayTimeDuration('{dur}')) - xs:dayTimeDuration('{dur}')) eq xs:dateTime('{base}')"
    );
    let r = eval(&expr);
    assert!(matches!(&r[0], XdmItem::Atomic(XdmAtomicValue::Boolean(true))));
}

#[rstest]
fn fractional_duration_truncation_on_add() {
    // Current policy: xs:dayTimeDuration parses to whole seconds (fraction truncated).
    // Therefore adding <1 second keeps the instant unchanged.
    let r = eval(
        "(xs:dateTime('2024-01-01T00:00:00Z') + xs:dayTimeDuration('PT0.1234567895S')) eq xs:dateTime('2024-01-01T00:00:00Z')",
    );
    assert!(matches!(&r[0], XdmItem::Atomic(XdmAtomicValue::Boolean(true))));
}

#[rstest]
fn timezone_addition_cross_month_end() {
    // Adding one hour over month boundary in local +01:00 timezone remains consistent
    let r = eval(
        "(xs:dateTime('2024-02-29T23:30:00+01:00') + xs:dayTimeDuration('PT3600S')) eq xs:dateTime('2024-03-01T00:30:00+01:00')",
    );
    assert!(matches!(&r[0], XdmItem::Atomic(XdmAtomicValue::Boolean(true))));
}
