use platynui_xpath::{
    SimpleNode, evaluate_expr,
    runtime::DynamicContext,
    xdm::{XdmAtomicValue, XdmItem},
};
use rstest::rstest;
fn ctx() -> DynamicContext<SimpleNode> {
    DynamicContext::default()
}

fn bool_val(expr: &str) -> bool {
    let seq = evaluate_expr::<SimpleNode>(expr, &ctx()).unwrap();
    if let Some(XdmItem::Atomic(XdmAtomicValue::Boolean(b))) = seq.first() {
        *b
    } else {
        panic!("expected boolean")
    }
}

#[rstest]
#[case("deep-equal( (1, 2.0, 3.00), (1.0, 2, 3) )", true)]
#[case("deep-equal( (number('NaN')), (number('NaN')) )", true)]
#[case(
    "deep-equal( ('Ab','CD'), ('ab','cd'), 'urn:platynui:collation:simple-case')",
    true
)]
#[case(
    "deep-equal( ('Ab','CD'), ('ab','cx'), 'urn:platynui:collation:simple-case')",
    false
)]
#[case("deep-equal( (1,2), (1,2,3) )", false)]
fn deep_equal_cases(#[case] expr: &str, #[case] expected: bool) {
    assert_eq!(bool_val(expr), expected);
}
