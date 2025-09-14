use platynui_xpath::runtime::DynamicContextBuilder;
use platynui_xpath::{XdmItem, evaluate_expr};

type N = platynui_xpath::simple_node::SimpleNode;

#[test]
fn deep_equal_and_distinct_values_on_date_with_tz() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    // deep-equal on identical tz-aware dates
    let de = evaluate_expr::<N>(
        "deep-equal(xs:date('2024-01-01Z'), xs:date('2024-01-01Z'))",
        &ctx,
    )
    .unwrap();
    match &de[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("expected boolean"),
    }
    // distinct-values collapses identical tz-aware dates
    let dv = evaluate_expr::<N>(
        "count(distinct-values((xs:date('2024-01-01Z'), xs:date('2024-01-01Z'))))",
        &ctx,
    )
    .unwrap();
    match &dv[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(i)) => assert_eq!(*i, 1),
        _ => panic!("expected integer"),
    }
}

#[test]
fn deep_equal_and_distinct_values_on_time_with_tz() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let de = evaluate_expr::<N>(
        "deep-equal(xs:time('10:00:00+02:00'), xs:time('10:00:00+02:00'))",
        &ctx,
    )
    .unwrap();
    match &de[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("expected boolean"),
    }
    let dv = evaluate_expr::<N>(
        "count(distinct-values((xs:time('10:00:00+02:00'), xs:time('10:00:00+02:00'))))",
        &ctx,
    )
    .unwrap();
    match &dv[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(i)) => assert_eq!(*i, 1),
        _ => panic!("expected integer"),
    }
}
