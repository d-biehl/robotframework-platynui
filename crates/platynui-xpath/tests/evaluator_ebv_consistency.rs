use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::{xdm::XdmItem, engine::evaluator::evaluate_expr};

type N = platynui_xpath::model::simple::SimpleNode;

#[test]
fn boolean_on_decimal_respects_zero_nonzero() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let t = evaluate_expr::<N>("boolean(xs:decimal('1.25'))", &ctx).unwrap();
    match &t[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("expected boolean"),
    }
    let f = evaluate_expr::<N>("boolean(xs:decimal('0'))", &ctx).unwrap();
    match &f[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(!*b),
        _ => panic!("expected boolean"),
    }
}

#[test]
fn ebv_unsupported_atomic_raises_forg0006() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let err = evaluate_expr::<N>("boolean(QName('', 'a'))", &ctx).unwrap_err();
    assert!(err.code.contains("FORG0006"), "got {}", err.code);
}
