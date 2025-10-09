use platynui_xpath::{
    evaluate_expr,
    runtime::DynamicContext,
    xdm::{XdmAtomicValue, XdmItem},
};
use rstest::rstest;
fn ctx() -> DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContext::default()
}

fn bool_val(expr: &str) -> bool {
    let seq = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx()).unwrap();
    if let Some(XdmItem::Atomic(XdmAtomicValue::Boolean(b))) = seq.first() { *b } else { panic!("expected boolean") }
}

fn string(expr: &str) -> String {
    let seq = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx()).unwrap();
    if let Some(XdmItem::Atomic(XdmAtomicValue::String(s))) = seq.first() { s.clone() } else { String::new() }
}

#[rstest]
#[case("matches('AbC\\nfoo', '^abc', 'im')", true)]
#[case("matches('AbC', 'abc', 'i')", true)]
#[case("matches('AbC', '^abc$')", false)]
fn regex_flags_case_insensitive_and_multiline(#[case] expr: &str, #[case] expected: bool) {
    assert_eq!(bool_val(expr), expected);
}

#[rstest]
#[case("matches('a\nb', 'a.b', 's')", true)] // dotall
#[case("matches('abc', 'a  b  c', 'x')", true)] // ignore whitespace
fn regex_flags_dotall_and_whitespace(#[case] expr: &str, #[case] expected: bool) {
    assert_eq!(bool_val(expr), expected);
}

#[rstest]
#[case("replace('AbcABC', 'abc', 'X', 'i')", "XX")]
fn regex_replace_with_flags(#[case] expr: &str, #[case] expected: &str) {
    assert_eq!(string(expr), expected);
}

#[rstest]
#[case("tokenize('AbC-abc', 'abc', 'i')", 3)] // tokens: "", "-", ""
fn regex_tokenize_with_flags(#[case] expr: &str, #[case] expected_len: usize) {
    let tokens = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx()).unwrap();
    assert_eq!(tokens.len(), expected_len);
}

#[rstest]
fn regex_invalid_flag_errors() {
    let err =
        platynui_xpath::evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("matches('a','a','q')", &ctx());
    assert!(err.is_err());
    let e = format!("{}", err.err().unwrap());
    assert!(e.contains("FORX0001"));
}
