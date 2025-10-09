use platynui_xpath::compiler::{compile, ir::*};
use rstest::rstest;

fn ir(src: &str) -> InstrSeq {
    compile(src).expect("compile ok").instrs
}

#[rstest]
#[case("@x + @y", OpCode::Add)]
#[case("@x - @y", OpCode::Sub)]
#[case("@x * @y", OpCode::Mul)]
#[case("@x div @y", OpCode::Div)]
#[case("@x idiv @y", OpCode::IDiv)]
#[case("@x mod @y", OpCode::Mod)]
fn arithmetic_ops(#[case] src: &str, #[case] tail: OpCode) {
    let is = ir(src);
    assert!(matches!(is.0.last(), Some(op) if std::mem::discriminant(op) == std::mem::discriminant(&tail)));
}

#[rstest]
fn logical_and_or() {
    // After short-circuit implementation, And/Or generate jump-based code
    // Pattern: LHS JumpIfTrue/False(...) RHS ToEBV Jump(...) PushAtomic(bool)

    let or_ir = ir("true() or false()");
    // Should contain JumpIfTrue for short-circuit
    assert!(or_ir.0.iter().any(|op| matches!(op, OpCode::JumpIfTrue(_))));
    // Should push true if LHS is true (short-circuit)
    assert!(or_ir.0.iter().any(|op| matches!(op, OpCode::PushAtomic(_))));

    let and_ir = ir("true() and false()");
    // Should contain JumpIfFalse for short-circuit
    assert!(and_ir.0.iter().any(|op| matches!(op, OpCode::JumpIfFalse(_))));
    // Should push false if LHS is false (short-circuit)
    assert!(and_ir.0.iter().any(|op| matches!(op, OpCode::PushAtomic(_))));
}

#[rstest]
fn range_op() {
    let is = ir("1 to 3");
    assert!(matches!(is.0.last(), Some(OpCode::RangeTo)));
}
