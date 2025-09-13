use platynui_xpath::{
    evaluator::evaluate_expr, runtime::DynamicContextBuilder, simple_node::SimpleNode,
};
use rstest::rstest;

fn ctx() -> platynui_xpath::runtime::DynamicContext<SimpleNode> {
    DynamicContextBuilder::new().build()
}

#[rstest]
#[case("fn:min(())")]
#[case("fn:max(())")]
fn min_max_empty_sequence(#[case] expr: &str) {
    let c = ctx();
    let r = evaluate_expr::<SimpleNode>(expr, &c).unwrap();
    assert!(r.is_empty());
}
