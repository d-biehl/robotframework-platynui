use platynui_xpath::engine::evaluator::evaluate_expr;

fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    platynui_xpath::engine::runtime::DynamicContext::default()
}
use rstest::rstest;

#[rstest]
fn regex_invalid_flag_forx0001() {
    let err = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("matches('a','a','q')", &ctx());
    assert!(err.is_err());
    let e = format!("{}", err.err().unwrap());
    assert!(e.contains("FORX0001"), "{e}");
}

#[rstest]
fn backref_in_char_class_forx0002() {
    let err = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("matches('a', '[$1]')", &ctx());
    assert!(err.is_err());
    let e = format!("{}", err.err().unwrap());
    assert!(e.contains("FORX0002"), "{e}");
}

#[rstest]
fn replace_zero_length_match_forx0003() {
    // Pattern that can match empty string: ".*?" at start will match zero-length
    let err = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("replace('abc', '.*?', 'X')", &ctx());
    assert!(err.is_err());
    let e = format!("{}", err.err().unwrap());
    assert!(e.contains("FORX0003"), "{e}");
}

#[rstest]
fn replacement_invalid_group_forx0004() {
    // $9 invalid if there are fewer groups
    let err = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("replace('abc', '(a)', '$9')", &ctx());
    assert!(err.is_err());
    let e = format!("{}", err.err().unwrap());
    assert!(e.contains("FORX0004"), "{e}");
}
