// Tests for fn:trace() behavior.
// Before the fix, trace() discarded the label entirely.
// Now it emits label+value via tracing::debug! and returns the input unchanged.

use platynui_xpath::xdm::{XdmAtomicValue as A, XdmItem as I};
use platynui_xpath::{SimpleNode, evaluate_expr, runtime::DynamicContext};
use rstest::rstest;

type N = SimpleNode;

fn ctx() -> DynamicContext<N> {
    DynamicContext::default()
}

#[rstest]
fn trace_returns_integer_unchanged() {
    let c = ctx();
    let seq = evaluate_expr::<N>("trace(42, 'my-label')", &c).unwrap();
    assert_eq!(seq.len(), 1);
    match &seq[0] {
        I::Atomic(A::Integer(v)) => assert_eq!(*v, 42),
        other => panic!("expected Integer(42), got {other:?}"),
    }
}

#[rstest]
fn trace_returns_string_unchanged() {
    let c = ctx();
    let seq = evaluate_expr::<N>("trace('hello', 'str-trace')", &c).unwrap();
    assert_eq!(seq.len(), 1);
    match &seq[0] {
        I::Atomic(A::String(s)) => assert_eq!(s, "hello"),
        other => panic!("expected String('hello'), got {other:?}"),
    }
}

#[rstest]
fn trace_returns_sequence_unchanged() {
    let c = ctx();
    let seq = evaluate_expr::<N>("trace((1, 2, 3), 'seq-trace')", &c).unwrap();
    assert_eq!(seq.len(), 3);
    let values: Vec<i64> = seq
        .iter()
        .map(|item| match item {
            I::Atomic(A::Integer(v)) => *v,
            other => panic!("expected Integer, got {other:?}"),
        })
        .collect();
    assert_eq!(values, vec![1, 2, 3]);
}

#[rstest]
fn trace_returns_empty_sequence_unchanged() {
    let c = ctx();
    let seq = evaluate_expr::<N>("trace((), 'empty-trace')", &c).unwrap();
    assert!(seq.is_empty());
}

#[rstest]
fn trace_returns_boolean_unchanged() {
    let c = ctx();
    let seq = evaluate_expr::<N>("trace(true(), 'bool-trace')", &c).unwrap();
    assert_eq!(seq.len(), 1);
    assert!(matches!(&seq[0], I::Atomic(A::Boolean(true))));
}

#[rstest]
fn trace_in_expression_context() {
    // trace() should be transparent in expressions — the result is the input value
    let c = ctx();
    let seq = evaluate_expr::<N>("trace(10, 'x') + trace(20, 'y')", &c).unwrap();
    assert_eq!(seq.len(), 1);
    match &seq[0] {
        I::Atomic(A::Integer(v)) => assert_eq!(*v, 30),
        other => panic!("expected Integer(30), got {other:?}"),
    }
}

#[rstest]
fn trace_nested() {
    // Nested trace calls should all pass through
    let c = ctx();
    let seq = evaluate_expr::<N>("trace(trace(5, 'inner'), 'outer')", &c).unwrap();
    assert_eq!(seq.len(), 1);
    match &seq[0] {
        I::Atomic(A::Integer(v)) => assert_eq!(*v, 5),
        other => panic!("expected Integer(5), got {other:?}"),
    }
}

#[rstest]
fn trace_with_empty_label() {
    let c = ctx();
    let seq = evaluate_expr::<N>("trace(99, '')", &c).unwrap();
    assert_eq!(seq.len(), 1);
    match &seq[0] {
        I::Atomic(A::Integer(v)) => assert_eq!(*v, 99),
        other => panic!("expected Integer(99), got {other:?}"),
    }
}
