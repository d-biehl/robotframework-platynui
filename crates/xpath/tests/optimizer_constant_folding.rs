// Tests for constant folding optimization
//
// These tests verify that constant expressions are evaluated at compile-time
// rather than runtime, reducing the instruction count and improving performance.

use platynui_xpath::compiler::compile;
use platynui_xpath::compiler::ir::{InstrSeq, OpCode};
use platynui_xpath::xdm::XdmAtomicValue;
use rstest::rstest;

fn get_ir(xpath: &str) -> InstrSeq {
    compile(xpath).expect("Compilation should succeed").instrs
}

fn assert_single_push_atomic(ir: &InstrSeq, expected: &XdmAtomicValue) {
    assert_eq!(ir.0.len(), 1, "Expected single instruction, got: {:?}", ir.0);
    match &ir.0[0] {
        OpCode::PushAtomic(val) => {
            assert_eq!(val, expected, "Expected {:?}, got {:?}", expected, val);
        }
        other => panic!("Expected PushAtomic, got: {:?}", other),
    }
}

fn decimal(literal: &str) -> XdmAtomicValue {
    XdmAtomicValue::Decimal(rust_decimal::Decimal::from_str_exact(literal).expect("valid decimal literal"))
}

// ===== Integer arithmetic folding =====

#[rstest]
// Basic operators — one case per fold path on i64
#[case::add_integers("1 + 2", XdmAtomicValue::Integer(3))]
#[case::sub_integers("5 - 3", XdmAtomicValue::Integer(2))]
#[case::sub_negative_result("10 - 15", XdmAtomicValue::Integer(-5))]
#[case::mul_integers("3 * 4", XdmAtomicValue::Integer(12))]
#[case::mul_by_zero("7 * 0", XdmAtomicValue::Integer(0))]
#[case::idiv_integers("10 idiv 3", XdmAtomicValue::Integer(3))]
#[case::idiv_negative("(-10) idiv 3", XdmAtomicValue::Integer(-3))]
#[case::mod_integers("10 mod 3", XdmAtomicValue::Integer(1))]
#[case::mod_exact("10 mod 5", XdmAtomicValue::Integer(0))]
// Add/zero boundaries — guard against accidental short-circuit shortcuts
#[case::add_zero_left("0 + 5", XdmAtomicValue::Integer(5))]
#[case::add_zero_right("5 + 0", XdmAtomicValue::Integer(5))]
// Chaining and precedence
#[case::chained_add("1 + 2 + 3", XdmAtomicValue::Integer(6))]
#[case::mul_before_sub("3 * 4 - 5", XdmAtomicValue::Integer(7))]
#[case::parentheses("(2 + 3) * 4", XdmAtomicValue::Integer(20))]
fn constant_folding_integer(#[case] xpath: &str, #[case] expected: XdmAtomicValue) {
    let ir = get_ir(xpath);
    assert_single_push_atomic(&ir, &expected);
}

// ===== Decimal arithmetic folding =====

#[rstest]
#[case::add_decimals("1.5 + 2.5", decimal("4"))]
#[case::sub_decimals("5.5 - 1.5", decimal("4"))]
#[case::mul_decimals("1.5 * 2.0", decimal("3.00"))]
#[case::mixed_int_dec_add("1 + 2.5", decimal("3.5"))]
#[case::mixed_dec_int_add("2.5 + 1", decimal("3.5"))]
#[case::mixed_int_dec_mul("3 * 1.5", decimal("4.5"))]
#[case::mixed_dec_int_sub("2.5 - 1", decimal("1.5"))]
fn constant_folding_decimal(#[case] xpath: &str, #[case] expected: XdmAtomicValue) {
    let ir = get_ir(xpath);
    assert_single_push_atomic(&ir, &expected);
}

// ===== Cases that must NOT fold =====
//
// These assertions inspect for the *specific* un-folded op rather than just
// `ir.0.len() > 1`, so a future optimizer that replaces the op with a different
// instruction shape still trips the test instead of quietly passing.

fn contains_op(ir: &InstrSeq, matcher: impl Fn(&OpCode) -> bool) -> bool {
    ir.0.iter().any(matcher)
}

#[test]
fn no_folding_with_attribute_reference() {
    // Runtime value — must not fold.
    let ir = get_ir("//@value + 2");
    assert!(contains_op(&ir, |op| matches!(op, OpCode::Add)), "expected unfolded Add, got {:?}", ir.0);
}

#[test]
fn no_folding_with_function_call() {
    // count() result is a runtime value — must not fold.
    let ir = get_ir("count((1, 2, 3)) + 1");
    assert!(
        contains_op(&ir, |op| matches!(op, OpCode::CallByName(_, _))),
        "expected unfolded function call, got {:?}",
        ir.0,
    );
    assert!(contains_op(&ir, |op| matches!(op, OpCode::Add)), "expected unfolded Add, got {:?}", ir.0);
}

#[rstest]
#[case::div_by_zero("1 div 0", OpCode::Div)]
#[case::idiv_by_zero("5 idiv 0", OpCode::IDiv)]
#[case::mod_by_zero("5 mod 0", OpCode::Mod)]
#[case::decimal_div_by_zero("1.5 div 0.0", OpCode::Div)]
fn division_and_modulo_by_zero_preserved(#[case] xpath: &str, #[case] expected_op: OpCode) {
    let ir = get_ir(xpath);
    assert!(
        contains_op(&ir, |op| std::mem::discriminant(op) == std::mem::discriminant(&expected_op)),
        "expected unfolded {:?}, got {:?}",
        expected_op,
        ir.0,
    );
}

#[test]
fn integer_overflow_in_add_preserved() {
    // i64::MAX + 1 overflows — folder must back off (checked_add returns None).
    let ir = get_ir("9223372036854775807 + 1");
    assert!(contains_op(&ir, |op| matches!(op, OpCode::Add)), "expected unfolded Add, got {:?}", ir.0);
}

#[test]
fn integer_overflow_in_mul_preserved() {
    let ir = get_ir("9223372036854775807 * 2");
    assert!(contains_op(&ir, |op| matches!(op, OpCode::Mul)), "expected unfolded Mul, got {:?}", ir.0);
}

// ===== Folding inside nested sequences =====

#[test]
fn constant_folding_inside_predicate() {
    // (1, 2, 3)[. = 1 + 2] — the `1 + 2` inside the predicate should fold to 3.
    let ir = get_ir("(1, 2, 3)[. = 1 + 2]");
    let folded_three = ir.0.iter().any(|op| matches!(op, OpCode::PushAtomic(XdmAtomicValue::Integer(3))));
    assert!(folded_three, "expected folded `1 + 2` → PushAtomic(Integer(3)), got {:?}", ir.0);
}
