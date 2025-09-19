use platynui_xpath::compiler::{compile_xpath, ir::*};
use rstest::rstest;

fn ir(src: &str) -> InstrSeq {
    compile_xpath(src).expect("compile ok").instrs
}

#[rstest]
#[case("1+2", OpCode::Add)]
#[case("1-2", OpCode::Sub)]
#[case("2*3", OpCode::Mul)]
#[case("4 div 2", OpCode::Div)]
#[case("5 idiv 2", OpCode::IDiv)]
#[case("5 mod 2", OpCode::Mod)]
fn arithmetic_ops(#[case] src: &str, #[case] tail: OpCode) {
    let is = ir(src);
    assert!(
        matches!(is.0.last(), Some(op) if std::mem::discriminant(op) == std::mem::discriminant(&tail))
    );
}

#[rstest]
fn logical_and_or() {
    let or_ir = ir("true() or false()");
    assert!(matches!(or_ir.0.last(), Some(OpCode::Or)));
    let and_ir = ir("true() and false()");
    assert!(matches!(and_ir.0.last(), Some(OpCode::And)));
}

#[rstest]
fn range_op() {
    let is = ir("1 to 3");
    assert!(matches!(is.0.last(), Some(OpCode::RangeTo)));
}
