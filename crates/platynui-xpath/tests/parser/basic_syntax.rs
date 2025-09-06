use super::*;

// Basic Syntax Tests
// Tests for fundamental XPath syntax elements

#[rstest]
#[case("book", "Element name")]
#[case("@id", "Attribute")]
#[case(".", "Current node")]
#[case("..", "Parent node")]
#[case("*", "Wildcard")]
#[case("text()", "Text node")]
#[case("$var", "Variable")]
fn test_basic_syntax_elements(#[case] xpath: &str, #[case] description: &str) {
    let result = XPathParser::parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse {}: '{}'. Error: {:?}",
        description,
        xpath,
        result.err()
    );
}

#[rstest]
#[case("123", "Integer literal")]
#[case("123.45", "Decimal literal")]
#[case("'hello'", "String literal with single quotes")]
#[case("\"world\"", "String literal with double quotes")]
fn test_literals(#[case] xpath: &str, #[case] description: &str) {
    let result = XPathParser::parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse {}: '{}'. Error: {:?}",
        description,
        xpath,
        result.err()
    );
}

#[rstest]
#[case("$node1 is $node2", "Node identity")]
#[case("$node1 << $node2", "Node before")]
#[case("$node1 >> $node2", "Node after")]
fn test_node_comparison(#[case] xpath: &str, #[case] description: &str) {
    let result = XPathParser::parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse {}: '{}'. Error: {:?}",
        description,
        xpath,
        result.err()
    );
}
