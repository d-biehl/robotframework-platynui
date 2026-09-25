use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::model::simple::{doc, elem};
use platynui_xpath::runtime::ErrorCode;
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmItem as I};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;

fn assert_boolean(result: &[XdmItem<N>], expected: bool, msg: &str) {
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => assert_eq!(*b, expected, "{msg}"),
        other => panic!("expected boolean, got {other:?}: {msg}"),
    }
}

#[rstest]
fn boolean_on_decimal_respects_zero_nonzero() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let t = evaluate_expr::<N>("boolean(xs:decimal('1.25'))", &ctx).unwrap();
    assert_boolean(&t, true, "decimal 1.25 should be true");
    let f = evaluate_expr::<N>("boolean(xs:decimal('0'))", &ctx).unwrap();
    assert_boolean(&f, false, "decimal 0 should be false");
}

#[rstest]
fn ebv_unsupported_atomic_raises_forg0006() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let err = evaluate_expr::<N>("boolean(QName('', 'a'))", &ctx).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::FORG0006);
}

// --- EBV correctness: multi-item sequences starting with a node (§2.4.3 rule 2) ---

#[rstest]
fn ebv_multi_node_sequence_is_true() {
    let root = doc().child(elem("root").child(elem("a")).child(elem("b"))).build();
    let ctx = DynamicContextBuilder::<N>::default().with_context_item(I::Node(root)).build();
    let result = evaluate_expr::<N>("boolean((/root/a, /root/b))", &ctx).unwrap();
    assert_boolean(&result, true, "EBV of (node, node) must be true");
}

#[rstest]
fn ebv_node_then_atomic_is_true() {
    let root = doc().child(elem("root").child(elem("a"))).build();
    let ctx = DynamicContextBuilder::<N>::default().with_context_item(I::Node(root)).build();
    let result = evaluate_expr::<N>("boolean((/root/a, 42))", &ctx).unwrap();
    assert_boolean(&result, true, "EBV of (node, atomic) must be true");
}

#[rstest]
fn not_of_multi_node_sequence_is_false() {
    let root = doc().child(elem("root").child(elem("a")).child(elem("b"))).build();
    let ctx = DynamicContextBuilder::<N>::default().with_context_item(I::Node(root)).build();
    let result = evaluate_expr::<N>("not((/root/a, /root/b))", &ctx).unwrap();
    assert_boolean(&result, false, "not((node, node)) must be false");
}

#[rstest]
fn ebv_empty_sequence_is_false() {
    let root = doc().child(elem("root")).build();
    let ctx = DynamicContextBuilder::<N>::default().with_context_item(I::Node(root)).build();
    let result = evaluate_expr::<N>("boolean(/root/nonexistent)", &ctx).unwrap();
    assert_boolean(&result, false, "EBV of empty sequence must be false");
}

#[rstest]
fn ebv_single_node_is_true() {
    let root = doc().child(elem("root").child(elem("a"))).build();
    let ctx = DynamicContextBuilder::<N>::default().with_context_item(I::Node(root)).build();
    let result = evaluate_expr::<N>("boolean(/root/a)", &ctx).unwrap();
    assert_boolean(&result, true, "EBV of single node must be true");
}

#[rstest]
fn ebv_multi_atomic_raises_forg0006() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let err = evaluate_expr::<N>("boolean((1, 2))", &ctx).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::FORG0006);
}
