// Tests for short-circuit evaluation of boolean operators (and, or).
//
// Short-circuit evaluation ensures that:
// - `false() and X` never evaluates X
// - `true() or X` never evaluates X
//
// This is crucial for:
// 1. Performance (avoiding expensive operations)
// 2. Correctness (preventing errors in unreachable branches)
// 3. XPath spec compliance
//
// The tests use `1 div 0` ("poison") as the side we expect to be skipped.
// If short-circuit breaks, the poison evaluates and surfaces a division error,
// which is reported with the offending xpath so the failure is self-describing.

use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use platynui_xpath::{DynamicContextBuilder, SimpleNode, compile, evaluate};
use rstest::rstest;

fn eval_to_bool(xpath: &str) -> bool {
    let compiled = compile(xpath).unwrap_or_else(|e| panic!("compile `{xpath}`: {e:?}"));
    let ctx = DynamicContextBuilder::<SimpleNode>::default().build();
    let items = evaluate(&compiled, &ctx).unwrap_or_else(|e| {
        panic!("evaluate `{xpath}`: {e:?} — did short-circuit fail and let the poison expression run?")
    });

    match &items[..] {
        [XdmItem::Atomic(XdmAtomicValue::Boolean(b))] => *b,
        other => panic!("expected single boolean for `{xpath}`, got: {other:?}"),
    }
}

// ===== Short-circuit succeeds: poison side must never run =====

#[rstest]
// `and` with poisoned right side
#[case::and_false_skips_poison("false() and (1 div 0)", false)]
#[case::and_chain_stops_at_first_false("true() and false() and (1 div 0)", false)]
// `and` without short-circuit: both sides are bools
#[case::and_true_true("true() and true()", true)]
#[case::and_true_false("true() and false()", false)]
// `or` with poisoned right side
#[case::or_true_skips_poison("true() or (1 div 0)", true)]
#[case::or_chain_stops_at_first_true("false() or true() or (1 div 0)", true)]
// `or` without short-circuit: both sides are bools
#[case::or_false_true("false() or true()", true)]
#[case::or_false_false("false() or false()", false)]
// Nested poison
#[case::nested_and_inside_or("(false() and (1 div 0)) or true()", true)]
#[case::nested_or_inside_and("(true() or (1 div 0)) and false()", false)]
// EBV-driven short-circuit (empty/singleton sequences)
#[case::empty_sequence_is_false("() and (1 div 0)", false)]
#[case::exists_on_sequence_short_circuits_or("exists((1, 2, 3)) or (1 div 0)", true)]
#[case::singleton_nonzero_is_true("(1) or (1 div 0)", true)]
// Expensive right-hand side is never materialized
#[case::and_false_skips_large_range("false() and exists(1 to 1000000)", false)]
#[case::or_true_skips_large_range("true() or exists(1 to 1000000)", true)]
// Predicate-shaped right-hand side
#[case::and_false_skips_comparison("false() and (5 > 3)", false)]
#[case::or_true_skips_comparison("true() or (5 > 3)", true)]
fn short_circuit_produces_expected_boolean(#[case] xpath: &str, #[case] expected: bool) {
    assert_eq!(eval_to_bool(xpath), expected, "xpath: `{xpath}`");
}

// ===== Short-circuit is one-way: poison on the LEFT must still error =====
//
// These guard the converse — we don't want a future optimizer to accidentally
// "short-circuit" the left side too. XPath evaluation order is left-to-right.

#[rstest]
#[case::and_poison_on_left_with_false_right("(1 div 0) and false()")]
#[case::and_poison_on_left_with_true_right("(1 div 0) and true()")]
#[case::or_poison_on_left_with_true_right("(1 div 0) or true()")]
#[case::or_poison_on_left_with_false_right("(1 div 0) or false()")]
fn poison_on_left_must_propagate(#[case] xpath: &str) {
    let compiled = compile(xpath).unwrap_or_else(|e| panic!("compile `{xpath}`: {e:?}"));
    let ctx = DynamicContextBuilder::<SimpleNode>::default().build();
    let result = evaluate(&compiled, &ctx);
    assert!(result.is_err(), "expected left-side poison to surface for `{xpath}`, got Ok({result:?})");
}
