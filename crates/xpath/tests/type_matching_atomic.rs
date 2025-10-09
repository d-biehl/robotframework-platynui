use platynui_xpath::{
    evaluator::evaluate_expr,
    runtime::DynamicContextBuilder,
    xdm::{XdmAtomicValue as A, XdmItem as I},
};
use rstest::rstest;

fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContextBuilder::new().build()
}

fn eval(expr: &str) -> Vec<I<platynui_xpath::model::simple::SimpleNode>> {
    evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx()).unwrap()
}

#[rstest]
fn instance_of_integer_true() {
    let r = eval("1 instance of xs:integer");
    assert!(matches!(&r[0], I::Atomic(A::Boolean(true))));
}

#[rstest]
fn instance_of_decimal_true() {
    let r = eval("1 instance of xs:decimal");
    assert!(matches!(&r[0], I::Atomic(A::Boolean(true))));
}

#[rstest]
fn instance_of_decimal_literal_not_integer() {
    let r = eval("1.0 instance of xs:integer");
    assert!(matches!(&r[0], I::Atomic(A::Boolean(false))));
}

#[rstest]
fn instance_of_double_true() {
    let r = eval("1e0 instance of xs:double");
    assert!(matches!(&r[0], I::Atomic(A::Boolean(true))));
}

#[rstest]
fn instance_of_string_true() {
    let r = eval("'a' instance of xs:string");
    assert!(matches!(&r[0], I::Atomic(A::Boolean(true))));
}

#[rstest]
fn instance_of_qname_true() {
    let r = eval("xs:QName('p') instance of xs:QName");
    assert!(matches!(&r[0], I::Atomic(A::Boolean(true))));
}

#[rstest]
fn instance_of_anyuri_true() {
    let r = eval("xs:anyURI('http://x') instance of xs:anyURI");
    assert!(matches!(&r[0], I::Atomic(A::Boolean(true))));
}

#[rstest]
fn treat_as_mismatch_reports() {
    // treat-as on wrong type must error with XPTY0004
    let err = platynui_xpath::engine::evaluator::evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(
        "('a') treat as xs:integer",
        &ctx(),
    )
    .unwrap_err();
    assert!(format!("{}", err).contains("XPTY0004"));
}
