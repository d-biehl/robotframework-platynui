use super::*;

// Basic Syntax Tests
// Tests for fundamental XPath syntax elements

#[rstest]
#[case::element_name("book")]
#[case::attribute("@id")]
#[case::current_node(".")]
#[case::parent_node("..")]
#[case::wildcard("*")]
#[case::text_node("text()")]
#[case::variable("$var")]
fn test_basic_syntax_elements(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}': {:?}", xpath, result.err()
    );
}

#[rstest]
#[case::int_literal("123")]
#[case::decimal_literal("123.45")]
#[case::string_single("'hello'")]
#[case::string_double("\"world\"")]
#[case::empty_sequence("()")]
fn test_literals(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}': {:?}", xpath, result.err()
    );
}

#[rstest]
#[case::node_is("$node1 is $node2")]
#[case::node_before("$node1 << $node2")]
#[case::node_after("$node1 >> $node2")]
fn test_node_comparison(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}': {:?}", xpath, result.err()
    );
}
