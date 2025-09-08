use super::*;

#[rstest]
#[case::first_index("book[1]")]
#[case::position_equals_one("book[position() = 1]")]
#[case::has_attribute("book[@id]")]
#[case::attribute_equals("book[@id='123']")]
#[case::element_text_equals("book[title='XPath']")]
#[case::numeric_comparison("book[price > 20]")]
#[case::contains_fn("book[contains(@title, 'XML')]")]
#[case::multiple_conditions("book[@category='fiction' and @price < 30]")]
#[case::position_range("book[position() > 2 and position() < 5]")]
#[case::position_predicate("book[1]")]
#[case::attribute_existence("book[@id]")]
#[case::attribute_value("book[@id='123']")]
#[case::position_function_no_spaces("book[position()=1]")]
#[case::element_existence("book[title]")]
#[case::price_comparison("book[@price < 20]")]
#[case::logical_ops("//book[@id and @title]")]
fn test_predicates(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}': {:?}", xpath, result.err()
    );
}

#[rstest]
#[case::unclosed_bracket("//div[")]
#[case::unexpected_closing("//div]")]
#[case::double_opening("//div[[")]
#[case::double_closing("//div]]")]
#[case::incomplete_attr_cmp("//div[@class=")]
#[case::unclosed_quote("//div[@class='test'")]
#[case::mismatched_quotes("//div[@class=\"test]")]
#[case::empty_attribute_name("//div[@=value]")]
#[case::missing_left_operand("//div[='test']")]
#[case::missing_right_operand("//div[@class=]")]
fn test_malformed_predicates(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_err(),
        "Expected to fail parsing: '{}'", xpath
    );
}
