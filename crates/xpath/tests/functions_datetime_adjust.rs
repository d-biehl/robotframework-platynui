use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmItem};
use rstest::{fixture, rstest};

type N = platynui_xpath::model::simple::SimpleNode;

#[fixture]
fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::<N>::default().build()
}

#[rstest]
fn date_time_construct_hours(ctx: platynui_xpath::engine::runtime::DynamicContext<N>) {
    let out =
        evaluate_expr::<N>("hours-from-dateTime(dateTime(xs:date('2020-01-02'), xs:time('03:04:05')))", &ctx).unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(h)) => assert_eq!(*h, 3),
        _ => panic!("expected integer"),
    }
}

#[rstest]
fn adjust_date_to_timezone_builds(ctx: platynui_xpath::engine::runtime::DynamicContext<N>) {
    let d = evaluate_expr::<N>("adjust-date-to-timezone(xs:date('2020-01-02'), xs:dayTimeDuration('PT60S'))", &ctx)
        .unwrap();
    assert_eq!(d.len(), 1);
}

#[rstest]
fn adjust_time_to_timezone_builds(ctx: platynui_xpath::engine::runtime::DynamicContext<N>) {
    let t =
        evaluate_expr::<N>("adjust-time-to-timezone(xs:time('10:00:00'), xs:dayTimeDuration('PT0S'))", &ctx).unwrap();
    assert_eq!(t.len(), 1);
}

#[rstest]
fn adjust_datetime_to_timezone_builds(ctx: platynui_xpath::engine::runtime::DynamicContext<N>) {
    let dt = evaluate_expr::<N>(
        "adjust-dateTime-to-timezone(xs:dateTime('2020-01-02T10:00:00Z'), xs:dayTimeDuration('PT0S'))",
        &ctx,
    )
    .unwrap();
    assert_eq!(dt.len(), 1);
}

#[rstest]
fn normalize_unicode_basic(ctx: platynui_xpath::engine::runtime::DynamicContext<N>) {
    let out = evaluate_expr::<N>("normalize-unicode('A\u{030A}','NFC')", &ctx).unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::String(s)) => {
            assert_eq!(s, "Å")
        }
        _ => panic!("expected string"),
    }
}
