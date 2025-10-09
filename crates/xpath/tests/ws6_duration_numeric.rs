use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder};
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmItem};
use rstest::{fixture, rstest};

type N = platynui_xpath::model::simple::SimpleNode;

#[fixture]
fn ctx() -> DynamicContext<N> {
    DynamicContextBuilder::<N>::default().build()
}

#[rstest]
fn distinct_values_collapses_truncated_daytimeduration(ctx: DynamicContext<N>) {
    // PT1.1S -> PT1S via truncation; duplicates should collapse
    let r = evaluate_expr::<N>(
        "count(distinct-values((xs:dayTimeDuration('PT1S'), xs:dayTimeDuration('PT1.1S'), xs:dayTimeDuration('PT2S'), xs:dayTimeDuration('PT1S'))))",
        &ctx,
    )
    .unwrap();
    match &r[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(i)) => assert_eq!(*i, 2),
        _ => panic!("expected integer"),
    }
}

#[rstest]
fn index_of_reports_all_equal_positions_after_truncation(ctx: DynamicContext<N>) {
    let r = evaluate_expr::<N>(
        "index-of((xs:dayTimeDuration('PT1S'), xs:dayTimeDuration('PT2S'), xs:dayTimeDuration('PT1.5S')), xs:dayTimeDuration('PT1S'))",
        &ctx,
    )
    .unwrap();
    assert_eq!(r.len(), 2);
    let v1 = match &r[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(i)) => *i,
        _ => panic!(),
    };
    let v2 = match &r[1] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(i)) => *i,
        _ => panic!(),
    };
    assert_eq!((v1, v2), (1, 3));
}

#[rstest]
fn duration_division_returns_double(ctx: DynamicContext<N>) {
    let r = evaluate_expr::<N>("xs:dayTimeDuration('PT20S') div xs:dayTimeDuration('PT5S')", &ctx).unwrap();
    match &r[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Double(d)) => {
            assert!((*d - 4.0).abs() < 1e-12)
        }
        _ => panic!("expected double"),
    }
}

#[rstest]
fn duration_multiply_truncates_fractional_result(ctx: DynamicContext<N>) {
    // 3 * 2.5 = 7.5 -> truncates to 7 seconds
    let r = evaluate_expr::<N>("(xs:dayTimeDuration('PT3S') * 2.5) eq xs:dayTimeDuration('PT7S')", &ctx).unwrap();
    match &r[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("expected boolean"),
    }
}
