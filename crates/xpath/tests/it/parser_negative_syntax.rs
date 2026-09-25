use platynui_xpath::engine::runtime::ErrorCode;
use platynui_xpath::parser::parse;
use rstest::rstest;

#[rstest]
#[case("(")]
#[case("1 +")]
#[case("//@")]
#[case("element(a,,)")]
#[case("if (1) then else 2")]
#[case("processing-instruction('unterminated)")]
fn syntax_errors_have_code(#[case] input: &str) {
    let err = parse(input).expect_err("expected parse error");
    assert_eq!(err.code_enum(), ErrorCode::XPST0003);
}
