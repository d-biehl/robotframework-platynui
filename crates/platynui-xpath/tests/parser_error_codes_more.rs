use platynui_xpath::parser::parse_xpath;
use rstest::rstest;

#[rstest]
#[case("$", "XPST0003")] // var name missing
#[case("@", "XPST0003")] // name_test missing
#[case("element(,)", "XPST0003")] // missing name
#[case("foo(,)", "XPST0003")] // missing arg
fn static_error_codes(#[case] input: &str, #[case] code: &str) {
    let err = parse_xpath(input).expect_err("expected parse error");
    assert_eq!(err.code, code);
}
