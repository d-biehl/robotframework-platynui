#![allow(clippy::approx_constant)]

use platynui_xpath::compiler::{compile_with_context, ir::*};
use platynui_xpath::engine::runtime::StaticContext;
use platynui_xpath::xdm::XdmAtomicValue;
use rstest::{fixture, rstest};

#[fixture]
fn ctx() -> StaticContext {
    return StaticContext::default();
}

fn ir(src: &str, ctx: &StaticContext) -> InstrSeq {
    compile_with_context(src, ctx).expect("compile ok").instrs
}

#[rstest]
#[allow(clippy::approx_constant)]
#[case("42", XdmAtomicValue::Integer(42))]
#[case("3.14", XdmAtomicValue::Decimal(3.14))]
#[case("1e2", XdmAtomicValue::Double(100.0))]
#[case("'x'", XdmAtomicValue::String("x".into()))]
fn literals_lower(#[case] src: &str, #[case] expect: XdmAtomicValue, ctx: StaticContext) {
    let is = ir(src, &ctx);
    assert_eq!(is.0, vec![OpCode::PushAtomic(expect)]);
}

#[rstest]
fn sequence_make_seq(ctx: StaticContext) {
    let is = ir("(1,2,3)", &ctx);
    assert!(is.0.starts_with(&[
        OpCode::PushAtomic(XdmAtomicValue::Integer(1)),
        OpCode::PushAtomic(XdmAtomicValue::Integer(2)),
        OpCode::PushAtomic(XdmAtomicValue::Integer(3)),
    ]));
    assert!(matches!(is.0.last(), Some(OpCode::MakeSeq(3))));
}

#[rstest]
fn unary_plus_minus(ctx: StaticContext) {
    let plus = ir("+2", &ctx);
    assert!(matches!(plus.0.last(), Some(OpCode::PushAtomic(XdmAtomicValue::Integer(2)))));

    // With constant folding, -2 is folded to PushAtomic(-2) instead of 0 - 2
    let minus = ir("-2", &ctx);
    assert!(matches!(minus.0.last(), Some(OpCode::PushAtomic(XdmAtomicValue::Integer(-2)))));
}
