use platynui_xpath::compiler::{compile, ir::*};
use platynui_xpath::xdm::ExpandedName;
use rstest::rstest;

fn ir(src: &str) -> InstrSeq {
    compile(src).expect("compile ok").instrs
}

#[rstest]
fn let_binding_emits_opcodes() {
    let instrs = ir("let $x := 1 return $x");
    assert!(
        instrs
            .0
            .iter()
            .any(|op| matches!(op, OpCode::LetStartByName(ExpandedName { ns_uri: None, local }) if local == "x"))
    );
    assert_eq!(instrs.0.iter().filter(|op| matches!(op, OpCode::LetEnd)).count(), 1);
}

#[rstest]
fn sequential_let_bindings_are_declared() {
    let instrs = ir("let $x := 1, $y := $x + 1 return $y");
    let let_starts = instrs
        .0
        .iter()
        .filter_map(|op| match op {
            OpCode::LetStartByName(name) => Some(name.local.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(let_starts, vec!["x", "y"]);
    assert_eq!(instrs.0.iter().filter(|op| matches!(op, OpCode::LetEnd)).count(), 2);
}
