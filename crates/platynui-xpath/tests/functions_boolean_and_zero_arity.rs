use platynui_xpath::runtime::DynamicContextBuilder;
use platynui_xpath::{
    SimpleNode, evaluate_expr,
    simple_node::{doc, elem, text},
};
use rstest::rstest;

fn ctx_with_text(t: &str) -> platynui_xpath::runtime::DynamicContext<SimpleNode> {
    let root = doc().child(elem("r").child(text(t))).build();
    DynamicContextBuilder::default()
        .with_context_item(root)
        .build()
}

#[rstest]
fn boolean_ebv() {
    let c = ctx_with_text("");
    let out = evaluate_expr::<SimpleNode>("boolean(())", &c).unwrap();
    assert_eq!(out.len(), 1);
    // EBV of empty is false
    if let platynui_xpath::xdm::XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) =
        &out[0]
    {
        assert!(!b);
    } else {
        panic!("bool");
    }

    let out2 = evaluate_expr::<SimpleNode>("boolean((1))", &c).unwrap();
    if let platynui_xpath::xdm::XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) =
        &out2[0]
    {
        assert!(*b);
    } else {
        panic!("bool");
    }
}

#[rstest]
fn string_zero_arity_uses_context() {
    let c = ctx_with_text("Hello");
    let out = evaluate_expr::<SimpleNode>("string()", &c).unwrap();
    let s = match &out[0] {
        platynui_xpath::xdm::XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::String(s)) => {
            s.clone()
        }
        _ => panic!("str"),
    };
    assert_eq!(s, "Hello");
}

#[rstest]
fn normalize_space_zero_arity() {
    let c = ctx_with_text("  A  B   C  ");
    let out = evaluate_expr::<SimpleNode>("normalize-space()", &c).unwrap();
    let s = match &out[0] {
        platynui_xpath::xdm::XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::String(s)) => {
            s.clone()
        }
        _ => panic!("str"),
    };
    assert_eq!(s, "A B C");
}
