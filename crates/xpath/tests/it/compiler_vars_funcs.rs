use platynui_xpath::compiler::{compile, compile_with_context, ir::*};
use platynui_xpath::engine::runtime::{ErrorCode, StaticContextBuilder};
use platynui_xpath::xdm::ExpandedName;
use rstest::rstest;

fn ir(src: &str) -> InstrSeq {
    compile(src).expect("compile ok").instrs
}

#[rstest]
fn undeclared_variable_is_static_error() {
    let err = compile("$x").expect_err("variable reference must be rejected");
    assert_eq!(err.code_enum(), ErrorCode::XPST0008);
}

#[rstest]
fn declared_variable_compiles() {
    let static_ctx = StaticContextBuilder::new().with_variable(ExpandedName::new(None, "x")).build();
    let compiled = compile_with_context("$x", &static_ctx).expect("compile ok");
    assert!(
        compiled
            .instrs
            .0
            .iter()
            .any(|op| matches!(op, OpCode::LoadVarByName(ExpandedName{ ns_uri: None, local }) if local=="x"))
    );
}

#[rstest]
fn function_calls_emit_callbynane() {
    let is = ir("true() or false()");
    assert!(is.0.iter().any(
        |op| matches!(op, OpCode::CallByName(ExpandedName{ns_uri: _, local}, 0) if local=="true" || local=="false")
    ));
}
