use platynui_xpath::evaluator::evaluate_expr;
use platynui_xpath::runtime::DynamicContextBuilder;
use platynui_xpath::simple_node::SimpleNode;
use rstest::rstest;

fn dctx() -> platynui_xpath::runtime::DynamicContext<SimpleNode> {
    DynamicContextBuilder::new().build()
}

fn eval(expr: &str) -> String {
    let dc = dctx();
    let seq = evaluate_expr::<SimpleNode>(expr, &dc).unwrap();
    if seq.is_empty() {
        return "".into();
    }
    // Items format via Debug for atomic values (e.g., Boolean(true)); normalize
    // to plain lexical form for boolean to simplify assertions.
    let s = seq[0].to_string();
    if s == "Boolean(true)" {
        "true".into()
    } else if s == "Boolean(false)" {
        "false".into()
    } else {
        s
    }
}

#[rstest]
#[case("fn:contains('abc','b')", "true")]
#[case("fn:contains('abc','d')", "false")]
#[case(
    "fn:contains('abC','C','http://www.w3.org/2005/xpath-functions/collation/codepoint')",
    "true"
)]
fn contains_cases(#[case] expr: &str, #[case] expected: &str) {
    assert_eq!(eval(expr), expected);
}

#[rstest]
#[case("fn:starts-with('Hello','He')", "true")]
#[case("fn:starts-with('Hello','he')", "false")]
#[case("fn:ends-with('Hello','lo')", "true")]
#[case("fn:ends-with('Hello','LO')", "false")]
#[case(
    "fn:ends-with('Hello','lo','http://www.w3.org/2005/xpath-functions/collation/codepoint')",
    "true"
)]
fn starts_ends_cases(#[case] expr: &str, #[case] expected: &str) {
    assert_eq!(eval(expr), expected);
}

#[rstest]
#[case("fn:contains('a','a','http://example.com/zzz')")]
#[case("fn:starts-with('a','a','http://example.com/zzz')")]
#[case("fn:ends-with('a','a','http://example.com/zzz')")]
fn unknown_collation_errors(#[case] expr: &str) {
    let dc = dctx();
    let err = evaluate_expr::<SimpleNode>(expr, &dc).unwrap_err();
    assert!(format!("{err}").contains("FOCH0002"));
}
