use platynui_xpath::parser::parse_xpath;
use rstest::rstest;

#[rstest]
#[case("(")]
#[case("1 +")]
#[case("//@")]
#[case("element(a,,)")]
#[case("if (1) then else 2")]
#[case("processing-instruction('unterminated)")]
fn syntax_errors_have_code(#[case] input: &str) {
    let err = parse_xpath(input).expect_err("expected parse error");
    assert_eq!(err.code, "XPST0003");
}
