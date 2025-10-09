use platynui_xpath::compiler::{compile, ir::*};
use platynui_xpath::xdm::XdmAtomicValue;
use rstest::rstest;

fn ir(src: &str) -> InstrSeq {
    compile(src).expect("compile ok").instrs
}

#[rstest]
fn if_then_else_shape() {
    let is = ir("if (1) then 2 else 3");
    assert!(is.0.iter().any(|op| matches!(op, OpCode::ToEBV)));
    assert!(is.0.iter().any(|op| matches!(op, OpCode::JumpIfFalse(_))));
    assert!(is.0.iter().any(|op| matches!(op, OpCode::Jump(_))));
    assert!(is.0.iter().filter(|op| matches!(op, OpCode::PushAtomic(XdmAtomicValue::Integer(2)))).count() >= 1);
    assert!(is.0.iter().filter(|op| matches!(op, OpCode::PushAtomic(XdmAtomicValue::Integer(3)))).count() >= 1);
}
