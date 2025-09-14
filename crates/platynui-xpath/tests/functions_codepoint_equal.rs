use platynui_xpath::{
    evaluator::evaluate_expr, runtime::DynamicContextBuilder, simple_node::SimpleNode,
};
use rstest::rstest;

fn ctx() -> platynui_xpath::runtime::DynamicContext<SimpleNode> {
    DynamicContextBuilder::new().build()
}

// no helper needed; tests evaluate directly

#[rstest]
#[case("fn:codepoint-equal('a','a')", Some("Boolean(true)".to_string()))]
#[case("fn:codepoint-equal('a','b')", Some("Boolean(false)".to_string()))]
#[case("fn:codepoint-equal((), 'a')", None)]
#[case("fn:codepoint-equal('a', ())", None)]
fn codepoint_equal_param(#[case] expr: &str, #[case] expected_opt: Option<String>) {
    let c = ctx();
    let seq = evaluate_expr::<SimpleNode>(expr, &c).unwrap();
    let got = seq.first().map(|i| i.to_string());
    assert_eq!(got, expected_opt);
}
