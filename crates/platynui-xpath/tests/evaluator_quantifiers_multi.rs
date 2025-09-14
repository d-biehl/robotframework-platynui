use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::{xdm::XdmItem, engine::evaluator::evaluate_expr};

type N = platynui_xpath::model::simple::SimpleNode;

#[test]
fn some_two_bindings_product_true() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    // some $x in (1,2), $y in (3) satisfies $x + $y = 5 -> true (2+3)
    let expr = "some $x in (1,2), $y in (3) satisfies $x + $y = 5";
    let out = evaluate_expr::<N>(expr, &ctx).unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("expected boolean"),
    }
}

#[test]
fn every_two_bindings_product_true() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    // every $x in (1,2), $y in (3) satisfies $x < 3 -> true
    let expr = "every $x in (1,2), $y in (3) satisfies $x lt 3";
    let out = evaluate_expr::<N>(expr, &ctx).unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("expected boolean"),
    }
}
