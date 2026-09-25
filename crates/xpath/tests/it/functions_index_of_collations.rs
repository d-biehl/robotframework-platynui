use platynui_xpath::engine::evaluator::evaluate_expr;
use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use rstest::rstest;

fn dctx() -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContextBuilder::new().build()
}

fn eval(expr: &str) -> Vec<i64> {
    let dc = dctx();
    let seq = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &dc).unwrap();
    seq.into_iter()
        .map(|i| match i {
            XdmItem::Atomic(XdmAtomicValue::Integer(v)) => v,
            other => panic!("expected Integer, got {other:?}"),
        })
        .collect()
}

#[rstest]
fn index_of_case_insensitive_with_collation() {
    // Use simple-case collation to find 'B' inside a sequence with 'b'
    let out = eval("index-of(('a','b','B','c'), 'b', 'urn:platynui:collation:simple-case')");
    // positions 2 and 3 (1-based)
    assert_eq!(out, vec![2, 3]);
}

#[rstest]
fn index_of_unknown_collation_errors() {
    let dc = dctx();
    let res = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(
        "index-of(('a','b'),'a','http://example.com/zzz')",
        &dc,
    );
    assert!(res.is_err());
    let msg = format!("{}", res.unwrap_err());
    assert!(msg.contains("FOCH0002"), "expected FOCH0002, got {msg}");
}
