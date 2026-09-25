use platynui_xpath::engine::evaluator::evaluate_expr;
use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem, XdmSequence};
use rstest::rstest;

fn dctx() -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContextBuilder::new().build()
}

fn eval(expr: &str) -> XdmSequence<platynui_xpath::model::simple::SimpleNode> {
    let dc = dctx();
    evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &dc).unwrap()
}

#[rstest]
#[case("fn:compare('a','b')", -1)]
#[case("fn:compare('b','a')", 1)]
#[case("fn:compare('a','a')", 0)]
#[case("fn:compare('x','y','http://www.w3.org/2005/xpath-functions/collation/codepoint')", -1)]
fn compare_codepoint_cases(#[case] expr: &str, #[case] expected: i64) {
    let r = eval(expr);
    assert_eq!(r.len(), 1);
    assert_eq!(as_int(&r[0]), expected);
}

#[rstest]
fn compare_empty_operand() {
    let r = eval("fn:compare((), 'a')");
    assert!(r.is_empty());
    let r = eval("fn:compare('a', ())");
    assert!(r.is_empty());
}

#[rstest]
fn compare_unknown_collation() {
    let dc = dctx();
    let err = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(
        "fn:compare('a','b','http://example.com/unknown-coll')",
        &dc,
    )
    .unwrap_err();
    assert!(format!("{err}").contains("FOCH0002"));
}

fn as_int<N>(it: &XdmItem<N>) -> i64 {
    match it {
        XdmItem::Atomic(XdmAtomicValue::Integer(i)) => *i,
        _ => panic!("expected integer atomic value"),
    }
}
