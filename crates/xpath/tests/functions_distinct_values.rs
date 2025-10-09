use platynui_xpath::{
    evaluator::evaluate_expr,
    runtime::DynamicContextBuilder,
    xdm::{XdmAtomicValue, XdmItem},
};
use rstest::rstest;

fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContextBuilder::new().build()
}

fn eval(expr: &str) -> Vec<XdmItem<platynui_xpath::model::simple::SimpleNode>> {
    evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx()).unwrap()
}

#[rstest]
fn distinct_values(
    #[values(
        // strings with repeats preserve first occurrence order
        ("distinct-values(('a','b','a','c','b'))", vec!["a","b","c"]) ,
        // case-sensitive behavior under codepoint collation
        ("distinct-values(('a','A','a'))", vec!["a","A"]) ,
    )]
    case: (&str, Vec<&str>),
) {
    let (expr, expected) = case;
    let r = eval(expr);
    let vals: Vec<String> = r
        .iter()
        .filter_map(|i| if let XdmItem::Atomic(XdmAtomicValue::String(s)) = i { Some(s.clone()) } else { None })
        .collect();
    let expected: Vec<String> = expected.into_iter().map(|s| s.to_string()).collect();
    assert_eq!(vals, expected);
}

#[rstest]
fn distinct_values_numbers_mixed() {
    let r = eval("distinct-values((1, 1.0, '1', 2))");
    assert_eq!(r.len(), 3);
    match &r[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(1)) => {}
        _ => panic!("expected Integer(1) first"),
    }
    match &r[1] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) if s == "1" => {}
        _ => panic!("expected String(\"1\") second"),
    }
    match &r[2] {
        XdmItem::Atomic(XdmAtomicValue::Integer(2)) => {}
        _ => panic!("expected Integer(2) third"),
    }
}

#[rstest]
fn distinct_values_nan_collapse() {
    let r = eval("distinct-values((number('NaN'), number('NaN'), number('NaN')))");
    assert_eq!(r.len(), 1, "NaN values should collapse to one");
    match &r[0] {
        XdmItem::Atomic(XdmAtomicValue::Double(v)) if v.is_nan() => {}
        XdmItem::Atomic(XdmAtomicValue::Float(v)) if v.is_nan() => {}
        _ => panic!("expected single NaN representative"),
    }
}

#[rstest]
fn distinct_values_with_collation_case_insensitive() {
    let r = eval("distinct-values(('Ab','ab','AB'), 'urn:platynui:collation:simple-case')");
    assert_eq!(r.len(), 1);
}

#[rstest]
fn distinct_values_non_atomic_error_ok() {
    let ok = platynui_xpath::engine::evaluator::evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(
        "distinct-values((1,2))",
        &ctx(),
    );
    assert!(ok.is_ok());
}
