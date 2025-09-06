use super::*;

#[rstest]
#[case("1 + 2", "Addition")]
#[case("10 - 5", "Subtraction")]
#[case("3 * 4", "Multiplication")]
#[case("10 div 2", "Division")]
#[case("10 mod 3", "Modulo")]
#[case("@id and @title", "Logical AND")]
#[case("@id or @title", "Logical OR")]
#[case("@price < 20", "Less than")]
#[case("@price <= 20", "Less than or equal")]
#[case("@price > 20", "Greater than")]
#[case("@price >= 20", "Greater than or equal")]
#[case("@id = 'value'", "Equality")]
#[case("@id != 'value'", "Inequality")]
fn test_operators(#[case] xpath: &str, #[case] description: &str) {
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
#[case("$a eq $b", "Value equal")]
#[case("$a ne $b", "Value not equal")]
#[case("$a lt $b", "Value less than")]
#[case("$a le $b", "Value less than or equal")]
#[case("$a gt $b", "Value greater than")]
#[case("$a ge $b", "Value greater than or equal")]
fn test_value_comparison_operators(#[case] xpath: &str, #[case] description: &str) {
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
#[case("1 div 2", "Division operator with proper spacing")]
#[case("5 mod 3", "Modulo operator with proper spacing")]
#[case("10 and 5", "Logical AND with proper spacing")]
#[case("3 or 7", "Logical OR with proper spacing")]
#[case("1 eq 2", "Value comparison with proper spacing")]
#[case("4 to 6", "Range expression with proper spacing")]
#[case("5 union 3", "Union operator with proper spacing")]
#[case("8 intersect 9", "Intersect operator with proper spacing")]
fn test_number_with_space_before_operators_should_pass(
    #[case] xpath: &str,
    #[case] description: &str,
) {
    let result = XPath2Parser::parse_xpath(xpath);
    // With proper spacing, these should be valid XPath expressions
    assert!(
        result.is_ok(),
        "Should be valid with proper spacing: {} - '{}'",
        description,
        xpath
    );
}

#[rstest]
#[case("1 div 2", "Number with space before div (valid)")]
#[case("5 mod 3", "Number with space before mod (valid)")]
#[case("10 and 5", "Number with space before and (valid)")]
#[case("3 or 7", "Number with space before or (valid)")]
#[case("1 eq 2", "Number with space before eq (valid)")]
#[case("5 ne 3", "Number with space before ne (valid)")]
#[case("2 lt 8", "Number with space before lt (valid)")]
fn test_number_with_proper_operator_spacing(#[case] xpath: &str, #[case] description: &str) {
    let result = XPath2Parser::parse_xpath(xpath);
    // With proper spacing, these should be valid
    assert!(
        result.is_ok(),
        "Should be valid with proper spacing: {} - '{}'",
        description,
        xpath
    );
}

#[rstest]
#[case("+ * 5", "Plus followed by multiplication")]
#[case("- * 3", "Invalid operator sequence")]
#[case("4 * * 2", "Double multiplication")]
#[case("8 / / 2", "Double division")]
#[case("5 mod mod 3", "Double mod operator")]
#[case("5 div div 2", "Double div operator")]
#[case("true() and and false()", "Double and operator")]
#[case("1 == 2", "Double equals")]
fn test_invalid_operator_sequences(#[case] xpath: &str, #[case] description: &str) {
    let result = XPath2Parser::parse_xpath(xpath);
    assert!(
        result.is_err(),
        "Expected {} to fail parsing: '{}'",
        description,
        xpath
    );
}

#[rstest]
#[case("1div2", "Number concatenated with div (should fail!)")]
#[case("5mod3", "Number concatenated with mod (should fail!)")]
#[case("10and5", "Number concatenated with and (should fail!)")]
#[case("3or7", "Number concatenated with or (should fail!)")]
#[case("1eq2", "Number concatenated with eq (should fail!)")]
#[case("5ne3", "Number concatenated with ne (should fail!)")]
#[case("2lt8", "Number concatenated with lt (should fail!)")]
fn test_number_concatenated_with_operators_should_fail(
    #[case] xpath: &str,
    #[case] description: &str,
) {
    let result = XPath2Parser::parse_xpath(xpath);
    // According to XPath 2.0 spec, these MUST fail - no space between number and operator
    assert!(
        result.is_err(),
        "MUST fail per XPath 2.0 spec: {} - '{}'",
        description,
        xpath
    );
}

#[rstest]
#[case(
    "//item[@a = 1 and @b = 2 or @c = 3 and @d = 4 or @e = 5]",
    "Complex and/or precedence"
)]
#[case(
    "//div[@x + @y * @z > @a - @b div @c]",
    "Arithmetic operator precedence"
)]
#[case(
    "//node[@val = 1 or (@val = 2 and @type = 'A') or @val = 3]",
    "Union-like logic with or precedence"
)]
#[case(
    "//elem[not(@active) and @status = 'pending' or @priority > 5]",
    "Not with and/or"
)]
#[case(
    "//item[@price * @qty + @tax > @budget and @category = 'A' or @discount > 0.1]",
    "Mixed arithmetic and logical"
)]
#[case(
    "//data[@x = @y and @y = @z and @a != @b and @b != @c]",
    "Chained equality"
)]
#[case(
    "//node[child::* > 5 and following::* < 10 or preceding::* = 0]",
    "Multiple axis comparisons"
)]
#[case(
    "//item[position() = 1 or position() = last() and @important = true()]",
    "Position with logical"
)]
fn test_operator_precedence_stress(#[case] xpath: &str, #[case] description: &str) {
    let result = XPath2Parser::parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse operator precedence {}: '{}'. Error: {:?}",
        description,
        xpath,
        result.err()
    );
    let parsed = result.unwrap();
    assert!(
        parsed.len() > 0,
        "Precedence expression should parse: {}",
        description
    );
}
