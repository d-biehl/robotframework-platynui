use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmItem as I};
use rstest::rstest;

fn eval(expr: &str) -> Vec<I<platynui_xpath::model::simple::SimpleNode>> {
    evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &DynamicContextBuilder::default().build()).unwrap()
}

#[rstest]
fn qname_equality_and_inequality() {
    let out = eval("QName('urn:x','p:l') eq QName('urn:x','q:l')");
    assert!(matches!(&out[0], I::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(true))));
    let out2 = eval("QName('urn:x','p:l') ne QName('urn:y','p:l')");
    assert!(matches!(&out2[0], I::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(true))));
}

#[rstest]
fn qname_relational_errors() {
    let res = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(
        "QName('urn:x','p:l') lt QName('urn:x','p:m')",
        &DynamicContextBuilder::default().build(),
    );
    assert!(res.is_err());
    let msg = format!("{}", res.unwrap_err());
    assert!(msg.contains("XPTY0004"));
}

#[rstest]
fn notation_equality_and_inequality() {
    let out = eval("xs:NOTATION('p:l') eq xs:NOTATION('p:l')");
    assert!(matches!(&out[0], I::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(true))));
    let out2 = eval("xs:NOTATION('p:l') ne xs:NOTATION('q:l')");
    assert!(matches!(&out2[0], I::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(true))));
}

#[rstest]
fn notation_relational_errors() {
    let res = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(
        "xs:NOTATION('p:l') lt xs:NOTATION('p:m')",
        &DynamicContextBuilder::default().build(),
    );
    assert!(res.is_err());
    let msg = format!("{}", res.unwrap_err());
    assert!(msg.contains("XPTY0004"));
}
