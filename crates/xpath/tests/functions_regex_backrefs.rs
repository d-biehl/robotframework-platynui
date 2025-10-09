use platynui_xpath::{
    evaluate_expr,
    xdm::{XdmAtomicValue, XdmItem},
};
use rstest::rstest;

fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    platynui_xpath::engine::runtime::DynamicContext::default()
}

fn bool_val(expr: &str) -> bool {
    let seq = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx()).unwrap();
    if let Some(XdmItem::Atomic(XdmAtomicValue::Boolean(b))) = seq.first() { *b } else { panic!("expected boolean") }
}

#[rstest]
fn pattern_backreference_basic() {
    assert!(bool_val("matches('aba', '(a)b\\1')"));
}

#[rstest]
fn pattern_backreference_case_insensitive() {
    assert!(bool_val("matches('AbA', '(a)b\\1', 'i')"));
}

#[rstest]
fn replacement_group_references_basic() {
    // Replace using $1 reference
    let seq =
        evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("replace('abc', '(a)(b)(c)', '$3$2$1')", &ctx())
            .unwrap();
    if let Some(XdmItem::Atomic(XdmAtomicValue::String(s))) = seq.first() {
        assert_eq!(s, "cba");
    } else {
        panic!("expected string result")
    }
}

#[rstest]
fn replacement_group_zero_invalid() {
    let err = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("replace('abc', '(a)', '$0')", &ctx());
    assert!(err.is_err());
    let e = format!("{}", err.err().unwrap());
    assert!(e.contains("FORX0004"), "{e}");
}
