// Tests for constant folding optimization
//
// These tests verify that constant expressions are evaluated at compile-time
// rather than runtime, reducing the instruction count and improving performance.

use platynui_xpath::compiler::compile;
use platynui_xpath::compiler::ir::{InstrSeq, OpCode};
use platynui_xpath::xdm::XdmAtomicValue;

fn get_ir(xpath: &str) -> InstrSeq {
    compile(xpath).expect("Compilation should succeed").instrs
}

fn assert_single_push_atomic(ir: &InstrSeq, expected: XdmAtomicValue) {
    assert_eq!(ir.0.len(), 1, "Expected single instruction, got: {:?}", ir.0);
    match &ir.0[0] {
        OpCode::PushAtomic(val) => {
            assert_eq!(val, &expected, "Expected {:?}, got {:?}", expected, val);
        }
        other => panic!("Expected PushAtomic, got: {:?}", other),
    }
}

#[test]
fn test_constant_folding_add_integers() {
    let ir = get_ir("1 + 2");
    assert_single_push_atomic(&ir, XdmAtomicValue::Integer(3));
}

#[test]
fn test_constant_folding_sub_integers() {
    let ir = get_ir("5 - 3");
    assert_single_push_atomic(&ir, XdmAtomicValue::Integer(2));
}

#[test]
fn test_constant_folding_mul_integers() {
    let ir = get_ir("3 * 4");
    assert_single_push_atomic(&ir, XdmAtomicValue::Integer(12));
}

#[test]
fn test_constant_folding_idiv_integers() {
    let ir = get_ir("10 idiv 3");
    assert_single_push_atomic(&ir, XdmAtomicValue::Integer(3));
}

#[test]
fn test_constant_folding_mod_integers() {
    let ir = get_ir("10 mod 3");
    assert_single_push_atomic(&ir, XdmAtomicValue::Integer(1));
}

#[test]
fn test_constant_folding_add_doubles() {
    // 1.5 + 2.5 should fold to 4.0
    let ir = get_ir("1.5 + 2.5");
    assert_single_push_atomic(&ir, XdmAtomicValue::Decimal(4.0));
}

#[test]
fn test_constant_folding_mixed_integer_double() {
    // 1 + 2.5 should fold to 3.5 (integer promoted to double)
    let ir = get_ir("1 + 2.5");
    assert_single_push_atomic(&ir, XdmAtomicValue::Decimal(3.5));
}

#[test]
fn test_constant_folding_chained() {
    // 1 + 2 + 3 should fold to 6
    let ir = get_ir("1 + 2 + 3");
    assert_single_push_atomic(&ir, XdmAtomicValue::Integer(6));
}

#[test]
fn test_constant_folding_complex_expression() {
    // 3 * 4 - 5 should fold to 7
    let ir = get_ir("3 * 4 - 5");
    assert_single_push_atomic(&ir, XdmAtomicValue::Integer(7));
}

#[test]
fn test_constant_folding_preserves_order() {
    // (2 + 3) * 4 should fold to 20
    let ir = get_ir("(2 + 3) * 4");
    assert_single_push_atomic(&ir, XdmAtomicValue::Integer(20));
}

#[test]
fn test_no_folding_with_variables() {
    // Attributes like @value + 2 should NOT be folded (runtime value)
    let ir = get_ir("//@value + 2");
    // Should have Add operation (not folded)
    assert!(ir.0.iter().any(|op| matches!(op, OpCode::Add)), "Should not fold attribute references");
}

#[test]
fn test_no_folding_with_functions() {
    // count((1, 2, 3)) + 1 should NOT fold the count()
    let ir = get_ir("count((1, 2, 3)) + 1");
    // Should have function call, not folded
    assert!(ir.0.len() > 1, "Should not fold function calls");
}

#[test]
fn test_constant_folding_in_predicates() {
    // (1, 2, 3)[. = 1 + 2] should fold 1 + 2 to 3
    let ir = get_ir("(1, 2, 3)[. = 1 + 2]");

    // The folded constant 3 should appear somewhere in the IR
    let has_folded = ir.0.iter().any(|op| matches!(op, OpCode::PushAtomic(XdmAtomicValue::Integer(3))));

    assert!(has_folded, "Should contain folded constant 3 from predicate, got IR: {:?}", ir.0);
}

#[test]
fn test_constant_folding_division_by_zero_preserved() {
    // 1 div 0 should NOT be folded (would cause error)
    // The folder checks for zero divisor
    let ir = get_ir("1 div 0");
    // Should still have Div operation (not folded)
    assert!(ir.0.iter().any(|op| matches!(op, OpCode::Div)), "Division by zero should not be folded");
}
