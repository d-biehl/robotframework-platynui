use platynui_xpath::parser::parse_xpath;
use rstest::rstest;

#[rstest]
#[case("for $x in 1 group by $x return $x")]
#[case("for $x in 1 order by $x return $x")]
#[case("let $x := 1 return $x")]
fn unsupported_flwor_syntax(#[case] input: &str) {
    let err = parse_xpath(input).expect_err("expected parse error");
    assert_eq!(err.code, "XPST0003");
}
