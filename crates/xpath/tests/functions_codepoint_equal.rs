use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use platynui_xpath::{evaluator::evaluate_expr, runtime::DynamicContextBuilder};
use rstest::rstest;

fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContextBuilder::new().build()
}

// no helper needed; tests evaluate directly

#[rstest]
#[case("fn:codepoint-equal('a','a')", Some(true))]
#[case("fn:codepoint-equal('a','b')", Some(false))]
#[case("fn:codepoint-equal((), 'a')", None)]
#[case("fn:codepoint-equal('a', ())", None)]
fn codepoint_equal_param(#[case] expr: &str, #[case] expected_opt: Option<bool>) {
    let c = ctx();
    let seq = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &c).unwrap();
    let got_opt = seq.first().and_then(|i| match i {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => Some(*b),
        _ => None,
    });
    assert_eq!(got_opt, expected_opt);
}
