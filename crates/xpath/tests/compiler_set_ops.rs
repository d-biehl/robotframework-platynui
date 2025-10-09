use platynui_xpath::compiler::{compile, ir::*};
use rstest::rstest;

fn ir(src: &str) -> InstrSeq {
    compile(src).expect("compile ok").instrs
}

#[rstest]
#[case("(1,2) union (2,3)", OpCode::Union)]
#[case("(1,2) intersect (2,3)", OpCode::Intersect)]
#[case("(1,2) except (2,3)", OpCode::Except)]
fn set_ops(#[case] src: &str, #[case] tail: OpCode) {
    let is = ir(src);
    assert!(matches!(is.0.last(), Some(op) if std::mem::discriminant(op) == std::mem::discriminant(&tail)));
}
