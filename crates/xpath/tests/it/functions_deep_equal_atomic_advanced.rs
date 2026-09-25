use chrono::{FixedOffset, TimeZone};
use platynui_xpath::functions::deep_equal_with_collation;
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use rstest::rstest;

fn wrap(a: XdmAtomicValue) -> Vec<XdmItem<platynui_xpath::model::simple::SimpleNode>> {
    vec![XdmItem::Atomic(a)]
}

#[rstest]
fn deepequal_qname_prefix_ignored() {
    let a = XdmAtomicValue::QName { ns_uri: Some("urn:x".into()), prefix: Some("p1".into()), local: "n".into() };
    let b = XdmAtomicValue::QName { ns_uri: Some("urn:x".into()), prefix: Some("p2".into()), local: "n".into() };
    assert!(deep_equal_with_collation(&wrap(a), &wrap(b), None).unwrap());
}

#[rstest]
fn deepequal_datetime_timezone_normalized() {
    let tz_utc = FixedOffset::east_opt(0).unwrap();
    let tz_plus1 = FixedOffset::east_opt(3600).unwrap(); // +01:00
    let dt_utc = tz_utc.with_ymd_and_hms(2024, 5, 1, 12, 0, 0).unwrap();
    // Same absolute instant: 12:00Z == 13:00+01:00
    let dt_plus1 = tz_plus1.with_ymd_and_hms(2024, 5, 1, 13, 0, 0).unwrap();
    let a = XdmAtomicValue::DateTime(dt_utc);
    let b = XdmAtomicValue::DateTime(dt_plus1);
    assert!(deep_equal_with_collation(&wrap(a), &wrap(b), None).unwrap());
}

#[rstest]
fn deepequal_durations_yearmonth_and_daytime() {
    let a = XdmAtomicValue::YearMonthDuration(14);
    let b = XdmAtomicValue::YearMonthDuration(14);
    assert!(deep_equal_with_collation(&wrap(a), &wrap(b), None).unwrap());
    let c = XdmAtomicValue::DayTimeDuration(86_400); // representation assumes seconds
    let d = XdmAtomicValue::DayTimeDuration(86_400);
    assert!(deep_equal_with_collation(&wrap(c), &wrap(d), None).unwrap());
}
