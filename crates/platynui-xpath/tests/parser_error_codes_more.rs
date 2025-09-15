use platynui_xpath::parser::parse_xpath;
use platynui_xpath::engine::runtime::ErrorCode;
use rstest::rstest;

#[rstest]
#[case("$")] // var name missing
#[case("@")] // name_test missing
#[case("element(,)")] // missing name
#[case("foo(,)")] // missing arg
fn static_error_codes(#[case] input: &str) {
    let err = parse_xpath(input).expect_err("expected parse error");
    assert_eq!(err.code_enum(), ErrorCode::XPST0003);
}
