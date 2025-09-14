use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::{xdm::XdmItem, engine::evaluator::evaluate_expr};

type N = platynui_xpath::model::simple::SimpleNode;

#[test]
fn datetime_components_minutes_seconds_basic() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let m = evaluate_expr::<N>(
        "minutes-from-dateTime(xs:dateTime('2020-01-02T03:04:05Z'))",
        &ctx,
    )
    .unwrap();
    match &m[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(v)) => assert_eq!(*v, 4),
        _ => panic!("expected integer"),
    }
    let s = evaluate_expr::<N>(
        "seconds-from-dateTime(xs:dateTime('2020-01-02T03:04:05Z'))",
        &ctx,
    )
    .unwrap();
    match &s[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Decimal(v)) => assert_eq!(*v, 5.0),
        _ => panic!("expected decimal"),
    }
}

#[test]
fn seconds_from_time_fractional_decimal() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let r = evaluate_expr::<N>("seconds-from-time(xs:time('10:11:12.125'))", &ctx).unwrap();
    match &r[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Decimal(v)) => {
            assert!((*v - 12.125).abs() < 1e-12, "got {}", v)
        }
        _ => panic!("expected decimal"),
    }
}

#[test]
fn seconds_from_datetime_fractional_decimal() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let r = evaluate_expr::<N>(
        "seconds-from-dateTime(xs:dateTime('2020-01-02T03:04:05.007Z'))",
        &ctx,
    )
    .unwrap();
    match &r[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Decimal(v)) => {
            assert!((*v - 5.007).abs() < 1e-12, "got {}", v)
        }
        _ => panic!("expected decimal"),
    }
}

#[test]
fn normalize_unicode_invalid_form_errors() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let err = evaluate_expr::<N>("normalize-unicode('x','NOPE')", &ctx).unwrap_err();
    assert!(err.code.contains("FORG0001"));
}

#[test]
fn datetime_constructor_conflicting_timezones_errors() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    // date has Z, time has +01:00 -> conflict
    let err = evaluate_expr::<N>(
        "dateTime(xs:date('2020-01-02Z'), xs:time('10:00:00+01:00'))",
        &ctx,
    )
    .unwrap_err();
    assert!(err.code.contains("FORG0001"));
}
