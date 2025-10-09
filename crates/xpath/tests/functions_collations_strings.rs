use platynui_xpath::engine::evaluator::evaluate_expr;
use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use rstest::rstest;

fn dctx() -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContextBuilder::new().build()
}

// Generic evaluation helper that maps the single result item using a closure.
fn eval_map<T, F>(expr: &str, f: F) -> T
where
    F: Fn(&XdmItem<platynui_xpath::model::simple::SimpleNode>) -> T,
{
    let dc = dctx();
    let seq = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &dc).unwrap();
    assert_eq!(seq.len(), 1, "expected one result item, got {}", seq.len());
    f(&seq[0])
}

fn eval_bool(expr: &str) -> bool {
    eval_map(expr, |item| match item {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => *b,
        other => panic!("expected Boolean, got {other:?}"),
    })
}

#[rstest]
#[case("fn:contains('abc','b')", true)]
#[case("fn:contains('abc','d')", false)]
#[case("fn:contains('abC','C','http://www.w3.org/2005/xpath-functions/collation/codepoint')", true)]
fn contains_cases(#[case] expr: &str, #[case] expected: bool) {
    assert_eq!(eval_bool(expr), expected);
}

#[rstest]
#[case("fn:starts-with('Hello','He')", true)]
#[case("fn:starts-with('Hello','he')", false)]
#[case("fn:ends-with('Hello','lo')", true)]
#[case("fn:ends-with('Hello','LO')", false)]
#[case("fn:ends-with('Hello','lo','http://www.w3.org/2005/xpath-functions/collation/codepoint')", true)]
fn starts_ends_cases(#[case] expr: &str, #[case] expected: bool) {
    assert_eq!(eval_bool(expr), expected);
}

#[rstest]
#[case("fn:contains('a','a','http://example.com/zzz')")]
#[case("fn:starts-with('a','a','http://example.com/zzz')")]
#[case("fn:ends-with('a','a','http://example.com/zzz')")]
fn unknown_collation_errors(#[case] expr: &str) {
    let dc = dctx();
    let err = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &dc).unwrap_err();
    assert!(format!("{err}").contains("FOCH0002"));
}
