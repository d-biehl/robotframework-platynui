use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder};
use platynui_xpath::{
    XdmNode, evaluate_expr,
    simple_node::{doc, elem, text},
    xdm::XdmItem as I,
};
use rstest::{fixture, rstest};

type N = platynui_xpath::model::simple::SimpleNode;

fn ctx_with_root(root: N) -> DynamicContext<N> {
    let mut b = DynamicContextBuilder::default();
    b = b.with_context_item(I::Node(root));
    b.build()
}

fn build_doc() -> N {
    // <root><n v="10">ten</n><n v="20">twenty</n></root>
    use platynui_xpath::model::simple::attr as mkattr;
    let d = doc()
        .child(
            elem("root")
                .child(elem("n").attr(mkattr("v", "10")).child(text("ten")))
                .child(elem("n").attr(mkattr("v", "20")).child(text("twenty"))),
        )
        .build();
    d.children().next().unwrap()
}

#[fixture]
fn root() -> N {
    return build_doc();
}
#[fixture]
fn ctx(root: N) -> DynamicContext<N> {
    return ctx_with_root(root);
}

#[rstest]
fn general_eq_numbers() {
    let ctx = DynamicContextBuilder::default().build();
    let out = evaluate_expr::<N>("(1,2,3) = (5,4,3)", &ctx).unwrap();
    assert_eq!(out.len(), 1);
    assert!(matches!(&out[0], I::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(true))));
}

#[rstest]
fn general_eq_numbers_false() {
    let ctx = DynamicContextBuilder::default().build();
    let out = evaluate_expr::<N>("(1,2) = (3,4)", &ctx).unwrap();
    assert!(matches!(&out[0], I::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(false))));
}

#[rstest]
fn general_ne_mixed() {
    let ctx = DynamicContextBuilder::default().build();
    let out = evaluate_expr::<N>("(1,'a') != ('b',2)", &ctx).unwrap();
    assert!(matches!(&out[0], I::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(true))));
}

#[rstest]
fn general_lt_numeric() {
    let ctx = DynamicContextBuilder::default().build();
    // 2 < 5 should be true (pair 2,5)
    let out = evaluate_expr::<N>("(2,8) < (5,1)", &ctx).unwrap();
    assert!(matches!(&out[0], I::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(true))));
}

#[rstest]
fn general_gt_false() {
    let ctx = DynamicContextBuilder::default().build();
    let out = evaluate_expr::<N>("(1,2) > (3,4)", &ctx).unwrap();
    assert!(matches!(&out[0], I::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(false))));
}

#[rstest]
fn general_eq_empty_sequence() {
    let ctx = DynamicContextBuilder::default().build();
    let out = evaluate_expr::<N>("() = (1,2)", &ctx).unwrap();
    assert!(matches!(&out[0], I::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(false))));
}

#[rstest]
fn general_ne_empty_sequence() {
    let ctx = DynamicContextBuilder::default().build();
    let out = evaluate_expr::<N>("() != (1,2)", &ctx).unwrap();
    assert!(matches!(&out[0], I::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(false))));
}
