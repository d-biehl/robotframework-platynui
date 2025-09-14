use platynui_xpath::{
    evaluator::evaluate_expr, runtime::DynamicContextBuilder,
};
use rstest::rstest;

fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContextBuilder::new().build()
}

fn eval(expr: &str) -> String {
    let c = ctx();
    let seq = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &c).unwrap();
    if seq.is_empty() {
        return "".into();
    }
    seq[0].to_string()
}

#[rstest]
fn contains_with_untyped_atomic() {
    let v = eval("fn:contains(untypedAtomic('abc'),'b')");
    assert_eq!(v, "Boolean(true)");
    let v = eval("fn:contains(untypedAtomic('abc'),'z')");
    assert_eq!(v, "Boolean(false)");
}
