use super::*;

#[rstest]
#[case("book[1]", "First book")]
#[case("book[position() = 1]", "Position function")]
#[case("book[@id]", "Has attribute")]
#[case("book[@id='123']", "Attribute equals")]
#[case("book[title='XPath']", "Element text equals")]
#[case("book[price > 20]", "Numeric comparison")]
#[case("book[contains(@title, 'XML')]", "Contains function")]
#[case("book[@category='fiction' and @price < 30]", "Multiple conditions")]
#[case("book[position() > 2 and position() < 5]", "Position range")]
#[case("book[1]", "Position predicate")]
#[case("book[@id]", "Attribute existence")]
#[case("book[@id='123']", "Attribute value")]
#[case("book[position()=1]", "Position function")]
#[case("book[title]", "Element existence")]
#[case("book[@price < 20]", "Comparison in predicate")]
#[case("//book[@id and @title]", "Logical operators in predicate")]
fn test_predicates(#[case] xpath: &str, #[case] description: &str) {
    let result = XPath2Parser::parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse {}: '{}'. Error: {:?}",
        description,
        xpath,
        result.err()
    );
}

#[rstest]
#[case("//div[", "Unclosed predicate bracket")]
#[case("//div]", "Unexpected closing bracket")]
#[case("//div[[", "Double opening brackets")]
#[case("//div]]", "Double closing brackets")]
#[case("//div[@class=", "Incomplete attribute comparison")]
#[case("//div[@class='test'", "Unclosed quote in predicate")]
#[case("//div[@class=\"test]", "Mismatched quotes")]
#[case("//div[@=value]", "Empty attribute name")]
#[case("//div[='test']", "Missing left operand")]
#[case("//div[@class=]", "Missing right operand")]
fn test_malformed_predicates(#[case] xpath: &str, #[case] description: &str) {
    let result = XPath2Parser::parse_xpath(xpath);
    assert!(
        result.is_err(),
        "Expected {} to fail parsing: '{}'",
        description,
        xpath
    );
}
