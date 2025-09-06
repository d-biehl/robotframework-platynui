use super::*;

#[rstest]
#[case("count(//book)", "Count function")]
#[case("position()", "Position function")]
#[case("last()", "Last function")]
#[case("starts-with(@name, 'A')", "Starts with")]
#[case("contains(@title, 'XML')", "Contains")]
#[case("substring(@name, 1, 3)", "Substring")]
#[case("string-length(@title)", "String length")]
#[case("normalize-space(@text)", "Normalize space")]
#[case("sum(//price)", "Sum function")]
#[case("concat(@first, ' ', @last)", "Concat function")]
#[case("not(@disabled)", "Not function")]
#[case("true()", "True function")]
#[case("false()", "False function")]
fn test_functions(#[case] xpath: &str, #[case] description: &str) {
    let result = XPathParser::parse_xpath(xpath);
    assert!(result.is_ok(), "Failed to parse {}: '{}'. Error: {:?}", 
            description, xpath, result.err());
}

#[rstest]
#[case("matches(@class, \".*\")", "Regular expression function")]
#[case("replace(@text, 'old', 'new')", "String replacement function")]
#[case("tokenize(@tags, ',')", "String tokenization")]
#[case("substring(@name, 1, 5)", "Substring function")]
#[case("substring-before(@email, '@')", "Substring before function")]
#[case("substring-after(@email, '@')", "Substring after function")]
#[case("upper-case(@name)", "Upper case function")]
#[case("lower-case(@title)", "Lower case function")]
#[case("normalize-space(@description)", "Normalize space function")]
#[case("translate(@phone, '()-', '')", "Translate function")]
#[case("starts-with(@url, 'https')", "Starts with function")]
#[case("ends-with(@file, '.pdf')", "Ends with function")]
#[case("number(@price)", "Number conversion")]
#[case("string(@id)", "String conversion")]
#[case("boolean(@enabled)", "Boolean conversion")]
#[case("floor(@price * 1.2)", "Floor function")]
#[case("ceiling(@rating)", "Ceiling function")]
#[case("round(@average)", "Round function")]
#[case("abs(@difference)", "Absolute value")]
#[case("min(//product/@price)", "Minimum function")]
#[case("max(//product/@price)", "Maximum function")]
#[case("sum(//item/@quantity)", "Sum function")]
#[case("avg(//rating/@value)", "Average function")]
fn test_comprehensive_functions(#[case] xpath: &str, #[case] description: &str) {
    let result = XPathParser::parse_xpath(xpath);
    assert!(result.is_ok(), "Failed to parse {}: '{}'. Error: {:?}", 
            description, xpath, result.err());
}

#[rstest]
#[case("position(", "Unclosed function call")]
#[case("position)", "Function call without opening")]
#[case("count(", "Unclosed count function")]
#[case("substring(,)", "Function with empty parameters")]
#[case("normalize-space(", "Unclosed normalize-space")]
#[case("5()", "Number with function call syntax")]
#[case("position(", "Unclosed function call")]
#[case("position)", "Missing opening parenthesis")]
#[case("position((", "Extra opening parenthesis")]
#[case("position())", "Extra closing parenthesis")]
#[case("position(1, 2, )", "Trailing comma in arguments")]
#[case("position(,)", "Leading comma in arguments")]
#[case("position(1,, 2)", "Double comma in arguments")]
#[case("123()", "Number as function name")]
#[case("'string'()", "String as function name")]
fn test_malformed_function_calls(#[case] xpath: &str, #[case] description: &str) {
    let result = XPathParser::parse_xpath(xpath);
    assert!(result.is_err(), "Expected {} to fail parsing: '{}'", description, xpath);
}
