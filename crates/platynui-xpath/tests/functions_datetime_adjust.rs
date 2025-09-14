use platynui_xpath::runtime::DynamicContextBuilder;
use platynui_xpath::{XdmItem, evaluate_expr};

type N = platynui_xpath::simple_node::SimpleNode;

#[test]
fn date_time_construction_and_components() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let out = evaluate_expr::<N>(
        "hours-from-dateTime(dateTime(xs:date('2020-01-02'), xs:time('03:04:05')))",
        &ctx,
    )
    .unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(h)) => assert_eq!(*h, 3),
        _ => panic!("expected integer"),
    }
}

#[test]
fn adjust_date_time_timezone_basic() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let d = evaluate_expr::<N>(
        "adjust-date-to-timezone(xs:date('2020-01-02'), xs:dayTimeDuration('PT60S'))",
        &ctx,
    )
    .unwrap();
    assert_eq!(d.len(), 1);
    let t = evaluate_expr::<N>(
        "adjust-time-to-timezone(xs:time('10:00:00'), xs:dayTimeDuration('PT0S'))",
        &ctx,
    )
    .unwrap();
    assert_eq!(t.len(), 1);
    let dt = evaluate_expr::<N>(
        "adjust-dateTime-to-timezone(xs:dateTime('2020-01-02T10:00:00Z'), xs:dayTimeDuration('PT0S'))",
        &ctx,
    )
    .unwrap();
    assert_eq!(dt.len(), 1);
}

#[test]
fn normalize_unicode_basic() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let out = evaluate_expr::<N>("normalize-unicode('A\u{030A}','NFC')", &ctx).unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::String(s)) => {
            assert_eq!(s, "Å")
        }
        _ => panic!("expected string"),
    }
}
