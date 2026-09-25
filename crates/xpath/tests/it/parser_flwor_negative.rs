use platynui_xpath::engine::runtime::ErrorCode;
use platynui_xpath::parser::parse;
use rstest::rstest;

#[rstest]
#[case("for $x in 1 group by $x return $x")]
#[case("for $x in 1 order by $x return $x")]
fn unsupported_flwor_syntax(#[case] input: &str) {
    let err = parse(input).expect_err("expected parse error");
    assert_eq!(err.code_enum(), ErrorCode::XPST0003);
}
