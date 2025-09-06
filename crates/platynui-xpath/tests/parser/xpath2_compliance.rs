use super::*;

#[rstest]
#[case(
    "10div 2",
    "Number directly followed by div operator (violates A.2.4.1)"
)]
#[case(
    "10 div2",
    "Div operator directly followed by number (violates A.2.4.1)"
)]
#[case(
    "10div2",
    "Number and div operator concatenated with number (violates A.2.4.1)"
)]
#[case(
    "5mod 3",
    "Number directly followed by mod operator (violates A.2.4.1)"
)]
#[case(
    "5 mod3",
    "Mod operator directly followed by number (violates A.2.4.1)"
)]
#[case(
    "5mod3",
    "Number and mod operator concatenated with number (violates A.2.4.1)"
)]
#[case(
    "10and 5",
    "Number directly followed by and operator (violates A.2.4.1)"
)]
#[case(
    "10 and5",
    "And operator directly followed by number (violates A.2.4.1)"
)]
#[case(
    "10and5",
    "Number and and operator concatenated with number (violates A.2.4.1)"
)]
#[case("3or 7", "Number directly followed by or operator (violates A.2.4.1)")]
#[case("3 or7", "Or operator directly followed by number (violates A.2.4.1)")]
#[case(
    "3or7",
    "Number and or operator concatenated with number (violates A.2.4.1)"
)]
fn test_xpath2_whitespace_violations_should_fail(#[case] xpath: &str, #[case] description: &str) {
    let result = XPath2Parser::parse_xpath(xpath);
    // According to XPath 2.0 A.2.4.1: "10div 3", "10 div3", and "10div3" all result in syntax errors
    assert!(
        result.is_err(),
        "Expected {} to fail parsing according to XPath 2.0 A.2.4.1: '{}'",
        description,
        xpath
    );
}

#[rstest]
#[case("10 div 2", "Division with proper whitespace")]
#[case("5 mod 3", "Modulo with proper whitespace")]
#[case("10 and 5", "Logical AND with proper whitespace")]
#[case("3 or 7", "Logical OR with proper whitespace")]
#[case("1 eq 2", "Equality with proper whitespace")]
#[case("4 ne 6", "Not equal with proper whitespace")]
#[case("2 lt 8", "Less than with proper whitespace")]
#[case("9 gt 3", "Greater than with proper whitespace")]
#[case("abn - 2", "Subtraction with proper whitespace")]
#[case("foo - bar", "Subtraction between identifiers with proper whitespace")]
fn test_xpath2_proper_whitespace_should_pass(#[case] xpath: &str, #[case] description: &str) {
    let result = XPath2Parser::parse_xpath(xpath);
    // With proper whitespace according to XPath 2.0 A.2.4.1, these should parse successfully
    assert!(
        result.is_ok(),
        "Expected {} to parse successfully with proper whitespace: '{}'",
        description,
        xpath
    );
}

/// Tests for XPath 2.0 tokenization rules according to A.2.4.1 Default Whitespace Handling
/// and A.2.2 Terminal Delimitation which state that "-" requires symbol separators after QName/NCName
#[rstest]
#[case(
    "foo- foo",
    "foo- foo should result in syntax error per XPath 2.0 A.2.4.1"
)]
#[case("abn- 2", "abn- 2 should result in syntax error per XPath 2.0 A.2.4.1")]
#[case(
    "test- value",
    "test- value should result in syntax error per XPath 2.0 A.2.4.1"
)]
#[case(
    "element- name",
    "element- name should result in syntax error per XPath 2.0 A.2.4.1"
)]
fn test_xpath2_qname_minus_tokenization_violations_should_fail(
    #[case] expression: &str,
    #[case] description: &str,
) {
    let result = XPath2Parser::parse_xpath(expression);
    assert!(result.is_err(), "{}", description);
}

#[rstest]
#[case("foo -foo", "foo -foo should parse as subtraction")]
#[case("foo - foo", "foo - foo should parse as subtraction")]
#[case(
    "10- 2",
    "10- 2 should parse as subtraction (number literal, not QName)"
)]
#[case("10 - 2", "10 - 2 should parse as subtraction")]
#[case("foo-foo", "foo-foo should parse as single QName")]
#[case("test-name", "test-name should parse as single QName")]
fn test_xpath2_correct_minus_tokenization_should_pass(
    #[case] expression: &str,
    #[case] description: &str,
) {
    let result = XPath2Parser::parse_xpath(expression);
    assert!(result.is_ok(), "{}: {}", description, result.unwrap_err());
}
