use chrono::Timelike; // for second() / nanosecond()
use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::runtime::ErrorCode;
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmAtomicValue as A, xdm::XdmItem as I};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;
fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

fn expect_err(expr: &str) {
    let c = ctx();
    let err = evaluate_expr::<N>(expr, &c).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::FORG0001);
}

#[rstest]
#[case("xs:time('10:11:12.5')", 12, 500_000_000)]
#[case("xs:time('10:11:12.050')", 12, 50_000_000)]
#[case("xs:time('10:11:12.000000001')", 12, 1)]
#[case("xs:time('10:11:12.123456789123')", 12, 123_456_789)] // truncated to 9 digits
fn cast_time_fractional(#[case] expr: &str, #[case] sec_whole: u32, #[case] nanos: u32) {
    let c = ctx();
    let r = evaluate_expr::<N>(expr, &c).unwrap();
    if let I::Atomic(A::Time { time, .. }) = &r[0] {
        assert_eq!(time.second(), sec_whole);
        assert_eq!(time.nanosecond(), nanos);
    } else {
        panic!("expected time");
    }
}

#[rstest]
#[case("xs:dateTime('2025-09-12T01:02:03.75Z')", 3, 750_000_000)]
#[case("xs:dateTime('2025-09-12T01:02:03.007Z')", 3, 7_000_000)]
fn cast_datetime_fractional(#[case] expr: &str, #[case] sec_whole: u32, #[case] nanos: u32) {
    let c = ctx();
    let r = evaluate_expr::<N>(expr, &c).unwrap();
    if let I::Atomic(A::DateTime(dt)) = &r[0] {
        assert_eq!(dt.second(), sec_whole);
        assert_eq!(dt.nanosecond(), nanos);
    } else {
        panic!("expected dateTime");
    }
}

#[rstest]
fn cast_time_fractional_invalid_trailing_dot() {
    expect_err("xs:time('10:11:12.')");
}

#[rstest]
fn cast_time_fractional_invalid_alpha() {
    expect_err("xs:time('10:11:12.a3')");
}

#[rstest]
fn cast_time_fractional_invalid_second() {
    expect_err("xs:time('10:11:60.1')");
}

#[rstest]
fn cast_time_invalid_second_no_fraction() {
    expect_err("xs:time('10:11:60')");
}

#[rstest]
fn cast_datetime_invalid_second() {
    expect_err("xs:dateTime('2025-09-12T01:02:60Z')");
}
