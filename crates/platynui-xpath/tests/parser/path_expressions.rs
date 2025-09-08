use super::*;

#[rstest]
#[case::double_slash_only("//")]
#[case::root_with_text("/text()")]
#[case::current_node(".")]
#[case::parent_node("..")]
#[case::any_element("*")]
#[case::lone_at("@")]
#[case::any_attribute("@*")]
#[case::axis_without_node_child("child::")]
#[case::axis_without_node_descendant("descendant::")]
#[case::axis_without_node_following_sibling("following-sibling::")]
fn test_path_expressions_validation(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    // Only certain expressions should fail
    if xpath == "//" || xpath == "@" || xpath.ends_with("::") {
        assert!(
            result.is_err(),
            "Expected '{}' to fail parsing",
            xpath
        );
    } else {
        // These are actually valid in XPath
        assert!(
            result.is_ok(),
            "Expected '{}' to parse successfully",
            xpath
        );
    }
}

#[rstest]
#[case::simple_path("book/title")]
#[case::descendant_path("//book")]
#[case::absolute_with_predicate("/bookstore/book[1]")]
#[case::with_attribute_predicate("book[@id='123']")]
#[case::parent_reference("//book/author/../title")]
#[case::relative_descendant(".//chapter")]
#[case::position_function("book/chapter[last()]")]
#[case::root_element("/book")]
#[case::simple_path_again("/book/chapter")]
#[case::descendant_or_self("//book")]
#[case::relative_path("book/chapter")]
#[case::multi_level("book/chapter/title")]
#[case::explicit_axis_child("child::book")]
#[case::axis_descendant("descendant::title")]
#[case::axis_parent_wildcard("parent::*")]
fn test_path_expressions(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
    "Failed to parse '{}'. Error: {:?}",
    xpath,
    result.err()
    );
}

#[rstest]
#[case::axis_child("child::div")]
#[case::axis_descendant2("descendant::span")]
#[case::axis_descendant_or_self("descendant-or-self::*")]
#[case::axis_parent("parent::section")]
#[case::axis_ancestor("ancestor::article")]
#[case::axis_ancestor_or_self("ancestor-or-self::body")]
#[case::axis_following("following::p")]
#[case::axis_following_sibling("following-sibling::div")]
#[case::axis_preceding("preceding::h1")]
#[case::axis_preceding_sibling("preceding-sibling::nav")]
#[case::axis_attribute("attribute::id")]
#[case::axis_self("self::node()")]
#[case::axis_namespace("namespace::prefix")]
fn test_all_xpath_axes(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
    "Failed to parse '{}'. Error: {:?}",
    xpath,
    result.err()
    );
}
