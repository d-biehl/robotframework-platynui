use platynui_xpath::evaluator::evaluate_expr;
use platynui_xpath::runtime::DynamicContextBuilder;
use platynui_xpath::simple_node::SimpleNode;
use rstest::rstest;

fn dc() -> platynui_xpath::runtime::DynamicContext<SimpleNode> {
    DynamicContextBuilder::new().build()
}

#[rstest]
fn min_max_with_codepoint_named() {
    let d = dc();
    let r = evaluate_expr::<SimpleNode>(
        "fn:min(('b','a'),'http://www.w3.org/2005/xpath-functions/collation/codepoint')",
        &d,
    )
    .unwrap();
    // String atomic displays as String("a"); extract expected tail
    assert!(r[0].to_string().contains("\"a\""));
    let d = dc();
    let r = evaluate_expr::<SimpleNode>(
        "fn:max(('b','a'),'http://www.w3.org/2005/xpath-functions/collation/codepoint')",
        &d,
    )
    .unwrap();
    assert!(r[0].to_string().contains("\"b\""));
}

#[rstest]
fn min_max_unknown_collation() {
    let d = dc();
    let err =
        evaluate_expr::<SimpleNode>("fn:min(('a','b'),'http://example.com/nope')", &d).unwrap_err();
    assert!(format!("{err}").contains("FOCH0002"));
    let d = dc();
    let err =
        evaluate_expr::<SimpleNode>("fn:max(('a','b'),'http://example.com/nope')", &d).unwrap_err();
    assert!(format!("{err}").contains("FOCH0002"));
}

#[rstest]
fn deep_equal_codepoint_named() {
    let d = dc();
    let r = evaluate_expr::<SimpleNode>("fn:deep-equal(('a','b'),('a','b'),'http://www.w3.org/2005/xpath-functions/collation/codepoint')", &d).unwrap();
    assert_eq!(r[0].to_string(), "Boolean(true)");
}

#[rstest]
fn deep_equal_unknown_collation() {
    let d = dc();
    let err =
        evaluate_expr::<SimpleNode>("fn:deep-equal(('a'),('a'),'http://example.com/nope')", &d)
            .unwrap_err();
    assert!(format!("{err}").contains("FOCH0002"));
}
