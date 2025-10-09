use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::runtime::ErrorCode;
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmItem};
use rstest::{fixture, rstest};

type N = platynui_xpath::model::simple::SimpleNode;

#[fixture]
fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::<N>::default().build()
}

#[rstest]
fn minutes_from_datetime_basic(ctx: platynui_xpath::engine::runtime::DynamicContext<N>) {
    let m = evaluate_expr::<N>("minutes-from-dateTime(xs:dateTime('2020-01-02T03:04:05Z'))", &ctx).unwrap();
    match &m[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(v)) => assert_eq!(*v, 4),
        _ => panic!("expected integer"),
    }
}

#[rstest]
fn seconds_from_datetime_basic(ctx: platynui_xpath::engine::runtime::DynamicContext<N>) {
    let s = evaluate_expr::<N>("seconds-from-dateTime(xs:dateTime('2020-01-02T03:04:05Z'))", &ctx).unwrap();
    match &s[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Decimal(v)) => assert_eq!(*v, 5.0),
        _ => panic!("expected decimal"),
    }
}

#[rstest]
#[case("seconds-from-time(xs:time('10:11:12.125'))", 12.125)]
#[case("seconds-from-dateTime(xs:dateTime('2020-01-02T03:04:05.007Z'))", 5.007)]
fn seconds_fractional_decimal(
    ctx: platynui_xpath::engine::runtime::DynamicContext<N>,
    #[case] expr: &str,
    #[case] expected: f64,
) {
    let r = evaluate_expr::<N>(expr, &ctx).unwrap();
    match &r[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Decimal(v)) => {
            assert!((*v - expected).abs() < 1e-12, "got {}", v)
        }
        _ => panic!("expected decimal"),
    }
}

#[rstest]
fn normalize_unicode_invalid_form_errors(ctx: platynui_xpath::engine::runtime::DynamicContext<N>) {
    let err = evaluate_expr::<N>("normalize-unicode('x','NOPE')", &ctx).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::FOCH0003);
}

#[rstest]
fn datetime_constructor_conflicting_timezones_errors(ctx: platynui_xpath::engine::runtime::DynamicContext<N>) {
    // date has Z, time has +01:00 -> conflict
    let err = evaluate_expr::<N>("dateTime(xs:date('2020-01-02Z'), xs:time('10:00:00+01:00'))", &ctx).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::FORG0001);
}
