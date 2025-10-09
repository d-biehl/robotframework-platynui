use platynui_xpath::{evaluator::evaluate_expr, runtime::DynamicContextBuilder};
use rstest::rstest;

fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContextBuilder::new().build()
}

#[rstest]
#[case("fn:compare((), 'a', 'http://www.w3.org/2005/xpath-functions/collation/codepoint')")]
#[case("fn:compare('a', (), 'http://www.w3.org/2005/xpath-functions/collation/codepoint')")]
fn compare_3arg_empty_operand(#[case] expr: &str) {
    let c = ctx();
    let r = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &c).unwrap();
    assert!(r.is_empty());
}
