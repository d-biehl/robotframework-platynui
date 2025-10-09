use platynui_xpath::{
    evaluate_expr,
    runtime::DynamicContext,
    xdm::{XdmAtomicValue, XdmItem},
};
use rstest::rstest;

fn ctx() -> DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContext::default()
}

fn string(expr: &str) -> String {
    let seq = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx()).unwrap();
    if let Some(XdmItem::Atomic(XdmAtomicValue::String(s))) = seq.first() { s.clone() } else { String::new() }
}

fn seq_ints(expr: &str) -> Vec<i64> {
    let seq = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx()).unwrap();
    let mut out = Vec::new();
    for it in seq {
        if let XdmItem::Atomic(XdmAtomicValue::Integer(i)) = it {
            out.push(i);
        }
    }
    out
}

#[rstest]
#[case("substring('abcdef', number('foo'))", "")] // NaN start -> empty
#[case("substring('abcdef', -1000)", "abcdef")] // large negative -> whole string
#[case("substring('abcdef', 9999)", "")] // large positive beyond length -> empty
#[case("substring('abcdef', 2.5)", "bcdef")] // fractional start rounding ties to even
#[case("substring('abcdef', 0.6)", "abcdef")] // start < 1 -> whole string
fn substring_basic_rounding_and_edges(#[case] expr: &str, #[case] expected: &str) {
    assert_eq!(string(expr), expected);
}

#[rstest]
#[case("substring('abcdef', 1, number('foo'))", "")] // NaN len -> empty
#[case("substring('abcdef', 2, -1)", "")] // Negative len -> empty
#[case("substring('abcdef', 9999, 5)", "")] // start beyond length -> empty
#[case("substring('abcdef', -1000, 3)", "abc")] // large negative start len 3 -> first 3
#[case("substring('abcdef', 2.5, 2.5)", "bc")] // ties -> 2 and 2
#[case("substring('abcdef', 2, 3.5)", "bcde")] // len tie rounds up
fn substring_len_rounding_and_edge_cases(#[case] expr: &str, #[case] expected: &str) {
    assert_eq!(string(expr), expected);
}

#[rstest]
#[case("subsequence((1,2,3,4,5), number('foo'))", vec![])] // NaN start -> empty
#[case("subsequence((1,2,3,4,5), -1000)", vec![1,2,3,4,5])] // large negative -> full
#[case("subsequence((1,2,3,4,5), 9999)", vec![])] // large positive -> empty
#[case("subsequence((1,2,3,4,5), 2.5)", vec![2,3,4,5])] // fractional tie 2.5 -> 2
fn subsequence_two_arg_edges(#[case] expr: &str, #[case] expected: Vec<i64>) {
    assert_eq!(seq_ints(expr), expected);
}

#[rstest]
#[case("subsequence((1,2,3), number('foo'), 2)", vec![])] // NaN start -> empty
#[case("subsequence((1,2,3), 1, number('foo'))", vec![])] // NaN len -> empty
#[case("subsequence((1,2,3,4), 2, -1)", vec![])] // Negative len -> empty
#[case("subsequence((1,2,3,4), 9999, 2)", vec![])] // start beyond length -> empty
#[case("subsequence((1,2,3,4,5), -1000, 3)", vec![1,2,3])] // large negative start len 3 -> first 3
#[case("subsequence((1,2,3,4,5), 2.5, 2.5)", vec![2,3])] // rounding ties
fn subsequence_three_arg_edges(#[case] expr: &str, #[case] expected: Vec<i64>) {
    assert_eq!(seq_ints(expr), expected);
}
