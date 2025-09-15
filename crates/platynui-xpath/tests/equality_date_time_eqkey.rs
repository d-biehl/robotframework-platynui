use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder};
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmItem};
use rstest::{fixture, rstest};

type N = platynui_xpath::model::simple::SimpleNode;

#[fixture]
fn ctx() -> DynamicContext<N> {
    DynamicContextBuilder::<N>::default().build()
}

#[rstest]
#[case("deep-equal(xs:date('2024-01-01Z'), xs:date('2024-01-01Z'))")]
#[case("deep-equal(xs:time('10:00:00+02:00'), xs:time('10:00:00+02:00'))")]
fn deep_equal_with_tz_true(ctx: DynamicContext<N>, #[case] expr: &str) {
    let de = evaluate_expr::<N>(expr, &ctx).unwrap();
    match &de[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("expected boolean"),
    }
}

#[rstest]
#[case("count(distinct-values((xs:date('2024-01-01Z'), xs:date('2024-01-01Z'))))")]
#[case("count(distinct-values((xs:time('10:00:00+02:00'), xs:time('10:00:00+02:00'))))")]
fn distinct_values_with_tz_collapses(ctx: DynamicContext<N>, #[case] expr: &str) {
    let dv = evaluate_expr::<N>(expr, &ctx).unwrap();
    match &dv[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(i)) => assert_eq!(*i, 1),
        _ => panic!("expected integer"),
    }
}
