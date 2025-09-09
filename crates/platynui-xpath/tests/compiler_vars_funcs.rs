use platynui_xpath::compiler::{compile_xpath, ir::*};
use platynui_xpath::xdm::ExpandedName;
use rstest::rstest;

fn ir(src: &str) -> InstrSeq {
    compile_xpath(src).expect("compile ok").instrs
}

#[rstest]
fn var_ref_load_by_name() {
    let is = ir("$x");
    assert!(is.0.iter().any(|op| matches!(op, OpCode::LoadVarByName(ExpandedName{ ns_uri: None, local }) if local=="x")));
}

#[rstest]
fn function_calls_emit_callbynane() {
    let is = ir("true() or false()");
    assert!(is.0.iter().any(|op| matches!(op, OpCode::CallByName(ExpandedName{ns_uri: _, local}, 0) if local=="true" || local=="false")));
}
