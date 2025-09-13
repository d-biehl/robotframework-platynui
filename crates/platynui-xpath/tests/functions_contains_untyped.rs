use platynui_xpath::{
    evaluator::evaluate_expr, runtime::DynamicContextBuilder, simple_node::SimpleNode,
};
use rstest::rstest;

fn ctx() -> platynui_xpath::runtime::DynamicContext<SimpleNode> {
    DynamicContextBuilder::new().build()
}

fn eval(expr: &str) -> String {
    let c = ctx();
    let seq = evaluate_expr::<SimpleNode>(expr, &c).unwrap();
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
