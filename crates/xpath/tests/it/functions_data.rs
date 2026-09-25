use platynui_xpath::{
    evaluator::evaluate_expr,
    runtime::DynamicContextBuilder,
    xdm::{XdmAtomicValue, XdmItem},
};
use rstest::rstest;
type N = platynui_xpath::model::simple::SimpleNode;
fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

#[rstest]
fn data_on_empty_sequence() {
    let res = evaluate_expr::<N>("fn:data(())", &ctx()).unwrap();
    assert!(res.is_empty());
}

#[rstest]
fn data_on_atomic_values() {
    let res = evaluate_expr::<N>("fn:data((1,2,3))", &ctx()).unwrap();
    assert_eq!(res.len(), 3);
}

#[rstest]
fn data_zero_arity_uses_context_item() {
    let mut b = DynamicContextBuilder::default();
    b = b.with_context_item(XdmItem::Atomic(XdmAtomicValue::String("hello".into())));
    let c = b.build();
    let res = evaluate_expr::<N>("data()", &c).unwrap();
    assert_eq!(res.len(), 1);
    match &res[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert_eq!(s, "hello"),
        _ => panic!("expected string"),
    }
}

// NOTE: Node sequence atomization test deferred until node literal construction is implemented.
