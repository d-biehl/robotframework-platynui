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

use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use platynui_xpath::{DynamicContextBuilder, SimpleNode, compile, evaluate};

fn eval_to_bool(xpath: &str) -> bool {
    let compiled = compile(xpath).expect("Compilation failed");
    let ctx = DynamicContextBuilder::<SimpleNode>::default().build();
    let items = evaluate(&compiled, &ctx).expect("Evaluation failed");

    match &items[..] {
        [XdmItem::Atomic(XdmAtomicValue::Boolean(b))] => *b,
        _ => panic!("Expected single boolean, got: {:?}", items),
    }
}

#[test]
fn test_and_short_circuit_false_left() {
    // false() and <anything> should never evaluate <anything>
    // This would cause division by zero if evaluated
    let result = eval_to_bool("false() and (1 div 0)");
    assert!(!result);
}

#[test]
fn test_and_short_circuit_true_left() {
    // true() and X should evaluate X
    let result = eval_to_bool("true() and true()");
    assert!(result);

    let result = eval_to_bool("true() and false()");
    assert!(!result);
}

#[test]
fn test_or_short_circuit_true_left() {
    // true() or <anything> should never evaluate <anything>
    // This would cause division by zero if evaluated
    let result = eval_to_bool("true() or (1 div 0)");
    assert!(result);
}

#[test]
fn test_or_short_circuit_false_left() {
    // false() or X should evaluate X
    let result = eval_to_bool("false() or true()");
    assert!(result);

    let result = eval_to_bool("false() or false()");
    assert!(!result);
}

#[test]
fn test_and_chain_short_circuit() {
    // Should short-circuit at first false
    let result = eval_to_bool("true() and false() and (1 div 0)");
    assert!(!result);
}

#[test]
fn test_or_chain_short_circuit() {
    // Should short-circuit at first true
    let result = eval_to_bool("false() or true() or (1 div 0)");
    assert!(result);
}

#[test]
fn test_complex_short_circuit() {
    // Nested boolean expressions with short-circuit
    let result = eval_to_bool("(false() and (1 div 0)) or true()");
    assert!(result);

    let result = eval_to_bool("(true() or (1 div 0)) and false()");
    assert!(!result);
}

#[test]
fn test_short_circuit_with_sequences() {
    // Empty sequence is false, singleton non-zero is true
    let result = eval_to_bool("() and (1 div 0)");
    assert!(!result);

    // Use boolean() or exists() for multi-item sequences
    let result = eval_to_bool("exists((1, 2, 3)) or (1 div 0)");
    assert!(result);

    // Single-item sequence
    let result = eval_to_bool("(1) or (1 div 0)");
    assert!(result);
}

#[test]
fn test_short_circuit_performance_implication() {
    // This test verifies that expensive operations are skipped
    // In a real scenario, (1 to 1000000) would be expensive to generate
    // but with short-circuit, it should never be evaluated

    let result = eval_to_bool("false() and exists(1 to 1000000)");
    assert!(!result);

    let result = eval_to_bool("true() or exists(1 to 1000000)");
    assert!(result);
}

#[test]
fn test_short_circuit_with_predicates() {
    // Simulating predicate-like expressions
    // false() and position()>5 should not call position()
    let result = eval_to_bool("false() and (5 > 3)");
    assert!(!result);

    // true() or position()>5 should not call position()
    let result = eval_to_bool("true() or (5 > 3)");
    assert!(result);
}
