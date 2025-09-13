use platynui_xpath::{
    SimpleNode, evaluate_expr,
    xdm::{XdmAtomicValue, XdmItem},
};

fn ctx() -> platynui_xpath::runtime::DynamicContext<SimpleNode> {
    platynui_xpath::runtime::DynamicContext::default()
}

fn bool_val(expr: &str) -> bool {
    let seq = evaluate_expr::<SimpleNode>(expr, &ctx()).unwrap();
    if let Some(XdmItem::Atomic(XdmAtomicValue::Boolean(b))) = seq.get(0) {
        *b
    } else {
        panic!("expected boolean")
    }
}

#[test]
fn pattern_backreference_basic() {
    assert!(bool_val("matches('aba', '(a)b\\1')"));
}

#[test]
fn pattern_backreference_case_insensitive() {
    assert!(bool_val("matches('AbA', '(a)b\\1', 'i')"));
}

#[test]
fn replacement_group_references_basic() {
    // Replace using $1 reference
    let seq = evaluate_expr::<SimpleNode>("replace('abc', '(a)(b)(c)', '$3$2$1')", &ctx()).unwrap();
    if let Some(XdmItem::Atomic(XdmAtomicValue::String(s))) = seq.get(0) {
        assert_eq!(s, "cba");
    } else {
        panic!("expected string result")
    }
}

#[test]
fn replacement_group_zero_invalid() {
    let err = evaluate_expr::<SimpleNode>("replace('abc', '(a)', '$0')", &ctx());
    assert!(err.is_err());
    let e = format!("{}", err.err().unwrap());
    assert!(e.contains("FORX0004"), "{e}");
}
