use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::{xdm::XdmItem, engine::evaluator::evaluate_expr};

type N = platynui_xpath::model::simple::SimpleNode;

#[test]
fn daytimeduration_fractional_seconds_truncated_positive() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let r = evaluate_expr::<N>(
        "xs:dayTimeDuration('PT1.9S') eq xs:dayTimeDuration('PT1S')",
        &ctx,
    )
    .unwrap();
    match &r[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("expected boolean"),
    }
}

#[test]
fn daytimeduration_fractional_seconds_truncated_zero() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let r = evaluate_expr::<N>(
        "xs:dayTimeDuration('PT0.4S') eq xs:dayTimeDuration('PT0S')",
        &ctx,
    )
    .unwrap();
    match &r[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("expected boolean"),
    }
}

#[test]
fn daytimeduration_fractional_seconds_truncated_negative() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let r = evaluate_expr::<N>(
        "xs:dayTimeDuration('-PT1.9S') eq xs:dayTimeDuration('-PT1S')",
        &ctx,
    )
    .unwrap();
    match &r[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("expected boolean"),
    }
}
