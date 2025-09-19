use platynui_xpath::parser::parse;
use rstest::rstest;

#[rstest]
#[case("+")]
#[case("//")]
#[case("(1+")]
#[case("@")]
fn syntax_errors_have_code(#[case] input: &str) {
    let err = parse(input).expect_err("should fail");
    // Must contain XPST0003 for syntax errors per spec (static error)
    let msg = err.to_string();
    assert!(msg.contains("XPST0003"), "expected XPST0003 in: {}", msg);
}
