use platynui_xpath::{evaluator::evaluate_expr, runtime::DynamicContextBuilder};
use rstest::rstest;

fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContextBuilder::new().build()
}

#[rstest]
#[case("fn:min(())")]
#[case("fn:max(())")]
fn min_max_empty_sequence(#[case] expr: &str) {
    let c = ctx();
    let r = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &c).unwrap();
    assert!(r.is_empty());
}
