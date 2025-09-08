use super::*;

// Verify that doubled quotes inside string literals are unescaped in the AST
#[rstest]
#[case::dbl_quoted_with_escaped_quotes("'He said \"Hi\"'", "He said \"Hi\"")]
#[case::single_quoted_with_escaped_apostrophe("'It''s ok'", "It's ok")]
#[case::dbl_quoted_trailing_quote("'a\"'", "a\"")]
#[case::empty_single_quotes("''", "")]
fn test_string_unescape(#[case] xpath: &str, #[case] expected: &str) {
    let ast = parse_ast(xpath);
    expect_literal_text(&ast, expected);
}

#[rstest]
fn test_parenthesized_literal() {
    let ast = parse_ast("('x')");
    expect_literal_text(&ast, "x");
}
