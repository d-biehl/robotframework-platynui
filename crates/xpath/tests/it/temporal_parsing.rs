use chrono::{Datelike, Timelike};
use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::runtime::ErrorCode;
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmAtomicValue as A, xdm::XdmItem as I};
use rstest::rstest;

fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContextBuilder::default().build()
}

fn expect_err(expr: &str) {
    let c = ctx();
    let err = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &c).unwrap_err();
    assert!(err.code_enum() == ErrorCode::FORG0001, "expected FORG0001 got {:?} for {}", err.code_qname(), expr);
}

#[rstest]
fn time_fraction_truncation() {
    let c = ctx();
    let r = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("xs:time('10:11:12.123456789123+02:30')", &c)
        .unwrap();
    if let I::Atomic(A::Time { time, tz }) = &r[0] {
        assert_eq!(time.nanosecond(), 123_456_789);
        assert!(tz.is_some());
    } else {
        panic!("expected time");
    }
}

#[rstest]
fn time_invalid_second() {
    expect_err("xs:time('23:59:60Z')");
}

#[rstest]
fn time_invalid_tz_minutes() {
    expect_err("xs:time('01:02:03+05:99')");
}

#[rstest]
fn time_invalid_tz_hours() {
    expect_err("xs:time('01:02:03+15:00')");
}

#[rstest]
fn date_negative_year() {
    let c = ctx();
    let r = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("xs:date('-0010-05-01Z')", &c).unwrap();
    if let I::Atomic(A::Date { date, .. }) = &r[0] {
        assert_eq!(date.year(), -10);
    } else {
        panic!("expected date");
    }
}

#[rstest]
fn date_zero_year_invalid() {
    expect_err("xs:date('0000-01-01')");
}

#[rstest]
fn datetime_negative_year_fraction_tz() {
    let c = ctx();
    let r = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(
        "xs:dateTime('-0456-07-08T09:10:11.987654321-03:15')",
        &c,
    )
    .unwrap();
    if let I::Atomic(A::DateTime(dt)) = &r[0] {
        assert_eq!(dt.year(), -456);
        assert_eq!(dt.second(), 11);
        assert_eq!(dt.nanosecond(), 987_654_321);
    } else {
        panic!("expected dateTime");
    }
}

#[rstest]
fn datetime_invalid_tz_minutes() {
    expect_err("xs:dateTime('2025-01-02T03:04:05+12:75')");
}

#[rstest]
fn datetime_invalid_second() {
    expect_err("xs:dateTime('2025-01-02T03:04:60Z')");
}
