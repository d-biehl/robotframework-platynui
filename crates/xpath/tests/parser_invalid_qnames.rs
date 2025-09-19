use platynui_xpath::engine::runtime::ErrorCode;
use platynui_xpath::parser::parse;
use rstest::rstest;

#[rstest]
#[case(":a")]
#[case("a:")]
#[case(":*")]
#[case("*:*")]
#[case("9foo")] // starts with digit
#[case("a:9bar")] // local starts with digit
#[case("a:b:c")] // multiple colons
#[case(".foo")] // dot not allowed in start
#[case("a:\u{0301}b")] // combining mark after colon (no base)
#[case("\u{0301}a")] // combining mark as first char is invalid
#[case("\u{00B7}foo")] // middle dot is not allowed as start char
#[case("a::b")] // double colon in QName
#[case("a:*b")] // wildcard misuse in local part
#[case("a:#bar")] // illegal character '#' in NCName
fn invalid_qname_patterns(#[case] input: &str) {
    let err = parse(input).expect_err("expected parse error");
    assert_eq!(err.code_enum(), ErrorCode::XPST0003);
}
