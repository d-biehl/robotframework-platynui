use super::*;

#[rstest]
#[case("1 +", "Incomplete expression")]
#[case("for $x", "Incomplete for expression")]
#[case(")", "Unmatched parenthesis")]
#[case("book[", "Incomplete predicate")]
#[case("@", "Incomplete attribute")]
fn test_invalid_expressions(#[case] xpath: &str, #[case] description: &str) {
    let result = XPath2Parser::parse_xpath(xpath);
    assert!(result.is_err(), "Expected {} to fail parsing: '{}'", description, xpath);
}

#[rstest]
#[case("//div[", "Unclosed predicate")]
#[case("//div[@class=]", "Incomplete attribute comparison")]
#[case("book child::", "Invalid axis syntax")]
#[case("//element[position() =]", "Incomplete function call")]
#[case("for $x in return $x", "Invalid for expression")]
#[case("if then else", "Invalid if expression")]
#[case("$", "Incomplete variable")]
#[case("@*[", "Invalid attribute wildcard")]
#[case("(//book", "Unclosed parenthesis")]
#[case("//book)", "Unmatched closing parenthesis")]
fn test_additional_invalid_expressions(#[case] xpath: &str, #[case] description: &str) {
    let result = XPath2Parser::parse_xpath(xpath);
    assert!(result.is_err(), "Expected {} to fail parsing: '{}'", description, xpath);
}

#[rstest]
#[case("//element[", "Unclosed predicate bracket")]
#[case("//element]", "Unopened predicate bracket")]
#[case("//element[@attr=", "Incomplete attribute comparison")]
#[case("//element[@=value]", "Missing attribute name")]
#[case("function(", "Unclosed function call")]
#[case("function)", "Function without opening parenthesis")]
#[case("//element[position()=", "Incomplete position comparison")]
#[case("'unclosed string", "Unclosed string literal")]
#[case("\"unclosed string", "Unclosed double quote string")]
#[case("//element[@class='unclosed", "Unclosed string in predicate")]
#[case("@", "Lone at symbol")]
#[case("@@attr", "Double at symbol")]
#[case("//", "Double slash without node")]
#[case("axis::", "Invalid axis name")]
#[case("child:::", "Triple colon")]
#[case("$", "Lone dollar sign")]
#[case("$$variable", "Double dollar sign")]
#[case("", "Empty expression")]
fn test_genuinely_invalid_expressions(#[case] xpath: &str, #[case] description: &str) {
    let result = XPath2Parser::parse_xpath(xpath);
    assert!(result.is_err(), "Expected {} to fail parsing: '{}'", description, xpath);
}

#[rstest]
#[case("'unclosed string", "Unclosed single quote")]
#[case("\"unclosed string", "Unclosed double quote")]
#[case("'mixed quotes\"", "Mixed quote types")]
#[case("\"mixed quotes'", "Mixed quote types reverse")]
#[case("'nested 'quotes' here'", "Nested single quotes")]
#[case("\"nested \"quotes\" here\"", "Nested double quotes")]
#[case("'", "Empty single quote")]
#[case("\"", "Empty double quote")]
#[case("'''", "Triple single quotes")]
#[case("\"\"\"", "Triple double quotes")]
fn test_malformed_string_literals(#[case] xpath: &str, #[case] description: &str) {
    let result = XPath2Parser::parse_xpath(xpath);
    assert!(result.is_err(), "Expected {} to fail parsing: '{}'", description, xpath);
}

#[rstest]
#[case("invalid-axis::", "Non-existent axis")]
#[case("child-axis::", "Hyphenated invalid axis")]
#[case("children::", "Plural axis name")]
#[case("parents::", "Plural parent axis")]
#[case("descendants::", "Plural descendant axis")]
#[case("ancestors::", "Plural ancestor axis")]
#[case("child:::", "Triple colon")]
#[case("child:", "Single colon")]
#[case("::child", "Reversed axis syntax")]
#[case("child::element::", "Double axis in path")]
fn test_invalid_axis_syntax(#[case] xpath: &str, #[case] description: &str) {
    let result = XPath2Parser::parse_xpath(xpath);
    assert!(result.is_err(), "Expected {} to fail parsing: '{}'", description, xpath);
}

#[rstest]
#[case("$", "Lone variable symbol")]
#[case("$123", "Numeric variable name")]
#[case("$-var", "Variable with leading hyphen")]
#[case("$var.", "Variable with dot")]
#[case("$$var", "Double dollar")]
#[case("$var$", "Variable with trailing dollar")]
#[case("$var iable", "Space in variable name")]
fn test_invalid_variable_syntax(#[case] xpath: &str, #[case] description: &str) {
    let result = XPath2Parser::parse_xpath(xpath);
    assert!(result.is_err(), "Expected {} to fail parsing: '{}'", description, xpath);
}

#[rstest]
#[case("()()", "Empty parentheses sequence")]
#[case("(", "Single opening parenthesis")]
#[case(")", "Single closing parenthesis")]
#[case("(()", "Mismatched parentheses - extra opening")]
#[case("())", "Mismatched parentheses - extra closing")]
#[case("(5 + 3", "Unclosed arithmetic expression")]
#[case("5 + 3)", "Unmatched closing in arithmetic")]
#[case("((5 + 3)", "Missing closing parenthesis")]
#[case("(5 + 3))", "Extra closing parenthesis")]
#[case("()5", "Empty parentheses before number")]
fn test_malformed_parentheses(#[case] xpath: &str, #[case] description: &str) {
    let result = XPath2Parser::parse_xpath(xpath);
    assert!(result.is_err(), "Expected {} to fail parsing: '{}'", description, xpath);
}

#[rstest]
#[case("123.456.789", "Multiple decimal points")]
#[case(".123.", "Decimal point at start and end")]
#[case("..123", "Double decimal point")]
#[case("1.2.3e4", "Multiple decimals with exponent")]
#[case("1e", "Incomplete scientific notation")]
#[case("1e+", "Incomplete positive exponent")]
#[case("1e-", "Incomplete negative exponent")]
#[case("1ee5", "Double exponent")]
#[case("1e5.5", "Decimal in exponent")]
fn test_malformed_numeric_literals(#[case] xpath: &str, #[case] description: &str) {
    let result = XPath2Parser::parse_xpath(xpath);
    assert!(result.is_err(), "Expected {} to fail parsing: '{}'", description, xpath);
}

#[rstest]
#[case("//", "Empty descendant")]
#[case("book[", "Incomplete predicate")]
#[case("book]", "Unmatched bracket")]
#[case("@", "Incomplete attribute")]
#[case("book..chapter", "Invalid path separator")]
#[case("book[@id=]", "Incomplete comparison")]
#[case("book[@id='unclosed", "Unclosed string literal")]
#[case("book[@id=\"unclosed", "Unclosed double-quoted string")]
#[case("book[@@id]", "Double attribute symbol")]
#[case("book[[1]]", "Nested predicates")]
#[case("book[@id=123=456]", "Multiple equality operators")]
#[case("book[1 2]", "Missing operator between numbers")]
#[case("book/", "Trailing slash")]
#[case("5++", "Invalid numeric suffix")]
#[case("book and", "Incomplete logical expression")]
#[case("5 * * 3", "Double multiply operators")]
#[case("5 and and 3", "Double and operators")]
#[case("5 or or 3", "Double or operators")]
#[case("5 = = 3", "Double equality operators")]
#[case("5 < > 3", "Conflicting comparison operators")]
#[case("5 > < 3", "Conflicting comparison operators reversed")]
fn test_syntax_error_cases(#[case] xpath: &str, #[case] description: &str) {
    let result = XPath2Parser::parse_xpath(xpath);
    assert!(result.is_err(), "Expected {} to fail parsing: '{}'", description, xpath);
}

