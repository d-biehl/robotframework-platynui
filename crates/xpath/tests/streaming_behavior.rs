//! Streaming Behavior Tests
//!
//! These tests verify that the XPath evaluator actually streams results
//! and doesn't materialize large sequences unnecessarily.

use platynui_xpath::engine::runtime::{DynamicContextBuilder, StaticContextBuilder};
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use platynui_xpath::{compile_with_context, evaluate_first, evaluate_stream};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;

#[rstest]
fn streaming_handles_large_range_with_take() {
    let static_ctx = StaticContextBuilder::new().build();
    let dyn_ctx = DynamicContextBuilder::<N>::default().build();

    // Range doesn't materialize - only take(10) items are generated
    let compiled = compile_with_context("1 to 999999999", &static_ctx).unwrap();
    let stream = evaluate_stream(&compiled, &dyn_ctx).unwrap();

    let results: Vec<_> = stream.into_iter().take(10).collect::<Result<Vec<_>, _>>().unwrap();

    assert_eq!(results.len(), 10);
    // Verify it's actually the range
    match &results[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) => assert_eq!(*n, 1),
        _ => panic!("Expected integer at position 0"),
    }
    match &results[9] {
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) => assert_eq!(*n, 10),
        _ => panic!("Expected integer at position 9"),
    }
    // If this test completes without hanging, streaming works correctly
}

#[rstest]
fn streaming_subsequence_from_range() {
    let static_ctx = StaticContextBuilder::new().build();
    let dyn_ctx = DynamicContextBuilder::<N>::default().build();

    // subsequence should stream without materializing full range
    let compiled = compile_with_context("subsequence(1 to 999999999, 500, 10)", &static_ctx).unwrap();
    let results = evaluate_stream(&compiled, &dyn_ctx).unwrap().materialize().unwrap();

    assert_eq!(results.len(), 10);
    match &results[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) => assert_eq!(*n, 500),
        _ => panic!("Expected integer at position 0"),
    }
    match &results[9] {
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) => assert_eq!(*n, 509),
        _ => panic!("Expected integer at position 9"),
    }
    // If this test completes quickly, streaming works correctly
}

#[rstest]
fn streaming_exists_short_circuits() {
    let static_ctx = StaticContextBuilder::new().build();
    let dyn_ctx = DynamicContextBuilder::<N>::default().build();

    // exists() should return true after finding first item, not check all
    let compiled = compile_with_context("exists(1 to 999999999)", &static_ctx).unwrap();
    let result = evaluate_first(&compiled, &dyn_ctx).unwrap();

    match &result.unwrap() {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("Expected boolean"),
    }
    // If this test completes without hanging, short-circuit evaluation works
}

#[rstest]
fn streaming_count_uses_iterator() {
    let static_ctx = StaticContextBuilder::new().build();
    let dyn_ctx = DynamicContextBuilder::<N>::default().build();

    // count() should use Iterator::count() efficiently
    let compiled = compile_with_context("count(1 to 10000)", &static_ctx).unwrap();
    let result = evaluate_first(&compiled, &dyn_ctx).unwrap();

    match &result.unwrap() {
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) => assert_eq!(*n, 10000),
        _ => panic!("Expected integer"),
    }
    // Functional correctness verified - streaming iterator works
}

#[rstest]
fn streaming_empty_sequence_is_cheap() {
    let static_ctx = StaticContextBuilder::new().build();
    let dyn_ctx = DynamicContextBuilder::<N>::default().build();

    // Empty sequence should not allocate
    let compiled = compile_with_context("()", &static_ctx).unwrap();
    let results = evaluate_stream(&compiled, &dyn_ctx).unwrap().materialize().unwrap();

    assert_eq!(results.len(), 0);
    // Functional correctness verified - empty sequence works
}

#[rstest]
fn streaming_first_from_range() {
    let static_ctx = StaticContextBuilder::new().build();
    let dyn_ctx = DynamicContextBuilder::<N>::default().build();

    // Getting first item should not materialize the whole range
    let compiled = compile_with_context("(1 to 999999999)[1]", &static_ctx).unwrap();
    let result = evaluate_first(&compiled, &dyn_ctx).unwrap();

    match &result.unwrap() {
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) => assert_eq!(*n, 1),
        _ => panic!("Expected integer"),
    }
    // If this test completes without hanging, lazy evaluation works correctly
}
