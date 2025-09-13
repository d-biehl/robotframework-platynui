use platynui_xpath::evaluator::evaluate_expr;
use platynui_xpath::runtime::DynamicContextBuilder;
use platynui_xpath::simple_node::SimpleNode;
use rstest::rstest;

fn dctx() -> platynui_xpath::runtime::DynamicContext<SimpleNode> {
    DynamicContextBuilder::new().build()
}

fn eval(expr: &str) -> Vec<String> {
    let dc = dctx();
    let seq = evaluate_expr::<SimpleNode>(expr, &dc).unwrap();
    seq.into_iter().map(|i| i.to_string()).collect()
}

#[rstest]
fn index_of_case_insensitive_with_collation() {
    // Use simple-case collation to find 'B' inside a sequence with 'b'
    let out = eval("index-of(('a','b','B','c'), 'b', 'urn:platynui:collation:simple-case')");
    // positions 2 and 3 (1-based)
    assert_eq!(out, vec!["Integer(2)", "Integer(3)"]);
}

#[rstest]
fn index_of_unknown_collation_errors() {
    let dc = dctx();
    let res = evaluate_expr::<SimpleNode>("index-of(('a','b'),'a','http://example.com/zzz')", &dc);
    assert!(res.is_err());
    let msg = format!("{}", res.unwrap_err());
    assert!(msg.contains("FOCH0002"), "expected FOCH0002, got {msg}");
}
