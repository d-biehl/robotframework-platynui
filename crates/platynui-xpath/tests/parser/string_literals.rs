use super::*;

// Verify that doubled quotes inside string literals are unescaped in the AST
#[rstest]
#[case(r#""He said ""Hi""""#, "He said \"Hi\"")]
#[case("'It''s ok'", "It's ok")]
#[case(r#""a""""#, "a\"")]
#[case("''", "")]
fn test_string_unescape(#[case] xpath: &str, #[case] expected: &str) {
    let ast = parse_ast(xpath);
    expect_literal_text(&ast, expected);
}

#[rstest]
fn test_parenthesized_literal() {
    let ast = parse_ast("('x')");
    expect_literal_text(&ast, "x");
}
