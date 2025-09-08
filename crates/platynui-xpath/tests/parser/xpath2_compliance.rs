use super::*;

#[rstest]
#[case::num_followed_by_div("10div 2")]
#[case::div_followed_by_num("10 div2")]
#[case::num_div_num("10div2")]
#[case::num_followed_by_mod("5mod 3")]
#[case::mod_followed_by_num("5 mod3")]
#[case::num_mod_num("5mod3")]
#[case::num_followed_by_and("10and 5")]
#[case::and_followed_by_num("10 and5")]
#[case::num_and_num("10and5")]
#[case::num_followed_by_or("3or 7")]
#[case::or_followed_by_num("3 or7")]
#[case::num_or_num("3or7")]
#[case::num_followed_by_idiv("8idiv 3")]
#[case::idiv_followed_by_num("8 idiv3")]
#[case::num_idiv_num("8idiv3")]
fn test_xpath2_whitespace_violations_should_fail(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    // According to XPath 2.0 A.2.4.1: "10div 3", "10 div3", and "10div3" all result in syntax errors
    assert!(
        result.is_err(),
        "Expected to fail parsing according to XPath 2.0 A.2.4.1: '{}'",
        xpath
    );
}

#[rstest]
#[case::div_ws("10 div 2")]
#[case::mod_ws("5 mod 3")]
#[case::and_ws("10 and 5")]
#[case::or_ws("3 or 7")]
#[case::eq_ws("1 eq 2")]
#[case::ne_ws("4 ne 6")]
#[case::lt_ws("2 lt 8")]
#[case::gt_ws("9 gt 3")]
#[case::minus_qname_ws("abn - 2")]
#[case::minus_ident_ws("foo - bar")]
fn test_xpath2_proper_whitespace_should_pass(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    // With proper whitespace according to XPath 2.0 A.2.4.1, these should parse successfully
    assert!(
        result.is_ok(),
        "Expected to parse successfully with proper whitespace: '{}'",
        xpath
    );
}

/// Tests for XPath 2.0 tokenization rules according to A.2.4.1 Default Whitespace Handling
/// and A.2.2 Terminal Delimitation which state that "-" requires symbol separators after QName/NCName
#[rstest]
#[case::qname_minus_trailing_space("foo- foo")]
#[case::qname_minus_number("abn- 2")]
#[case::qname_minus_value("test- value")]
#[case::element_minus_name("element- name")]
fn test_xpath2_qname_minus_tokenization_violations_should_fail(#[case] expression: &str) {
    let result = parse_xpath(expression);
    assert!(result.is_err(), "Tokenization violation should fail");
}

#[rstest]
#[case::minus_ident_no_space("foo -foo")]
#[case::minus_ident_with_space("foo - foo")]
#[case::minus_number_space("10- 2")]
#[case::minus_number_both("10 - 2")]
#[case::qname_with_hyphen("foo-foo")]
#[case::qname_with_hyphen2("test-name")]
fn test_xpath2_correct_minus_tokenization_should_pass(#[case] expression: &str) {
    let result = parse_xpath(expression);
    assert!(result.is_ok(), "Minus tokenization parse failed: {}", result.unwrap_err());
}
