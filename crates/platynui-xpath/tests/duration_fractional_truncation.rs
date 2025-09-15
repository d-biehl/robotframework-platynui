use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder};
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmItem};
use rstest::{fixture, rstest};

type N = platynui_xpath::model::simple::SimpleNode;

#[fixture]
fn ctx() -> DynamicContext<N> {
    DynamicContextBuilder::<N>::default().build()
}

#[rstest]
#[case("xs:dayTimeDuration('PT1.9S') eq xs:dayTimeDuration('PT1S')")]
#[case("xs:dayTimeDuration('PT0.4S') eq xs:dayTimeDuration('PT0S')")]
#[case("xs:dayTimeDuration('-PT1.9S') eq xs:dayTimeDuration('-PT1S')")]
fn daytimeduration_fractional_seconds_truncated(ctx: DynamicContext<N>, #[case] expr: &str) {
    let r = evaluate_expr::<N>(expr, &ctx).unwrap();
    match &r[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("expected boolean"),
    }
}
