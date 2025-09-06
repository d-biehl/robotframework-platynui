use super::*;

// Verify that doubled quotes inside string literals are unescaped in the AST
#[rstest]
#[case(r#""He said ""Hi""""#, "He said \"Hi\"")]
#[case("'It''s ok'", "It's ok")]
#[case(r#""a""""#, "a\"")]
#[case("''", "")]
fn test_string_unescape(#[case] xpath: &str, #[case] expected: &str) {
    let ast = parse_and_extract_ast(xpath);
    assert_literal(&ast, expected, xpath);
}
