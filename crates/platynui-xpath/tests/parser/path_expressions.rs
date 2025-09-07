use super::*;

#[rstest]
#[case("//", "Double slash without node test")]
#[case("/text()", "Root with function (valid)")]
#[case(".", "Current context node (valid)")]
#[case("..", "Parent node (valid)")]
#[case("*", "Any element (valid)")]
#[case("@", "Lone at symbol")]
#[case("@*", "Any attribute (valid)")]
#[case("child::", "Axis without node test")]
#[case("descendant::", "Descendant axis without node test")]
#[case("following-sibling::", "Following-sibling axis without node test")]
fn test_path_expressions_validation(#[case] xpath: &str, #[case] description: &str) {
    let result = parse_xpath(xpath);
    // Only certain expressions should fail
    if xpath == "//" || xpath == "@" || xpath.ends_with("::") {
        assert!(
            result.is_err(),
            "Expected {} to fail parsing: '{}'",
            description,
            xpath
        );
    } else {
        // These are actually valid in XPath
        assert!(
            result.is_ok(),
            "Expected {} to parse successfully: '{}'",
            description,
            xpath
        );
    }
}

#[rstest]
#[case("book/title", "Simple path")]
#[case("//book", "Descendant path")]
#[case("/bookstore/book[1]", "Absolute path with predicate")]
#[case("book[@id='123']", "Path with attribute predicate")]
#[case("//book/author/../title", "Path with parent reference")]
#[case(".//chapter", "Relative descendant path")]
#[case("book/chapter[last()]", "Path with position function")]
#[case("/book", "Root element")]
#[case("/book/chapter", "Simple path")]
#[case("//book", "Descendant-or-self")]
#[case("book/chapter", "Relative path")]
#[case("book/chapter/title", "Multi-level path")]
#[case("child::book", "Explicit axis")]
#[case("descendant::title", "Descendant axis")]
#[case("parent::*", "Parent axis with wildcard")]
fn test_path_expressions(#[case] xpath: &str, #[case] description: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse {}: '{}'. Error: {:?}",
        description,
        xpath,
        result.err()
    );
}

#[rstest]
#[case("child::div", "Child axis")]
#[case("descendant::span", "Descendant axis")]
#[case("descendant-or-self::*", "Descendant or self axis")]
#[case("parent::section", "Parent axis")]
#[case("ancestor::article", "Ancestor axis")]
#[case("ancestor-or-self::body", "Ancestor or self axis")]
#[case("following::p", "Following axis")]
#[case("following-sibling::div", "Following sibling axis")]
#[case("preceding::h1", "Preceding axis")]
#[case("preceding-sibling::nav", "Preceding sibling axis")]
#[case("attribute::id", "Attribute axis")]
#[case("self::node()", "Self axis")]
#[case("namespace::prefix", "Namespace axis")]
fn test_all_xpath_axes(#[case] xpath: &str, #[case] description: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse {}: '{}'. Error: {:?}",
        description,
        xpath,
        result.err()
    );
}
