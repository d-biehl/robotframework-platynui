use super::*;

#[rstest]
#[case::incomplete_expr("1 +")]
#[case::incomplete_for("for $x")]
#[case::unmatched_paren(")")]
#[case::incomplete_predicate("book[")]
#[case::incomplete_attribute("@")]
fn test_invalid_expressions(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_err(),
    "Expected '{}' to fail parsing",
    xpath
    );
}

#[rstest]
#[case::unclosed_pred("//div[")]
#[case::incomplete_attr_cmp("//div[@class=]")]
#[case::invalid_axis("book child::")]
#[case::incomplete_fn_call("//element[position() =]")]
#[case::invalid_for("for $x in return $x")]
#[case::invalid_if("if then else")]
#[case::incomplete_var("$")]
#[case::invalid_attr_wc("@*[")]
#[case::unclosed_paren("(//book")]
#[case::unmatched_closing_paren("//book)")]
fn test_additional_invalid_expressions(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_err(),
    "Expected '{}' to fail parsing",
    xpath
    );
}

#[rstest]
#[case::unclosed_pred_bracket("//element[")]
#[case::unopened_pred_bracket("//element]")]
#[case::incomplete_attr_cmp2("//element[@attr=")]
#[case::missing_attr_name("//element[@=value]")]
#[case::unclosed_fn("function(")]
#[case::fn_without_open("function)")]
#[case::incomplete_position_cmp("//element[position()=")]
#[case::unclosed_single_string("'unclosed string")]
#[case::unclosed_double_string("\"unclosed string")]
#[case::unclosed_string_in_pred("//element[@class='unclosed")]
#[case::lone_at("@")]
#[case::double_at("@@attr")]
#[case::double_slash("//")]
#[case::invalid_axis_name("axis::")]
#[case::triple_colon("child:::")]
#[case::lone_dollar("$")]
#[case::double_dollar("$$variable")]
#[case::empty_expr("")]
fn test_genuinely_invalid_expressions(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_err(),
    "Expected '{}' to fail parsing",
    xpath
    );
}

#[rstest]
#[case::unclosed_single_quote("'unclosed string")]
#[case::unclosed_double_quote("\"unclosed string")]
#[case::mixed_quotes_1("'mixed quotes\"")]
#[case::mixed_quotes_2("\"mixed quotes'")]
#[case::nested_single_quotes("'nested 'quotes' here'")]
#[case::nested_double_quotes("\"nested \"quotes\" here\"")]
#[case::empty_single_quote("'")]
#[case::empty_double_quote("\"")]
#[case::triple_single_quotes("'''")]
#[case::triple_double_quotes("\"\"\"")]
fn test_malformed_string_literals(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_err(),
    "Expected '{}' to fail parsing",
    xpath
    );
}

#[rstest]
#[case::nonexistent_axis("invalid-axis::")]
#[case::hyphenated_axis("child-axis::")]
#[case::plural_children_axis("children::")]
#[case::plural_parents_axis("parents::")]
#[case::plural_descendants_axis("descendants::")]
#[case::plural_ancestors_axis("ancestors::")]
#[case::triple_colon_axis("child:::")]
#[case::single_colon_axis("child:")]
#[case::reversed_axis("::child")]
#[case::double_axis_in_path("child::element::")]
fn test_invalid_axis_syntax(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_err(),
    "Expected '{}' to fail parsing",
    xpath
    );
}

#[rstest]
#[case::lone_variable("$")]
#[case::numeric_var_name("$123")]
#[case::leading_hyphen_var("$-var")]
#[case::var_with_dot("$var.")]
#[case::double_dollar_var("$$var")]
#[case::trailing_dollar_var("$var$")]
#[case::space_in_var_name("$var iable")]
fn test_invalid_variable_syntax(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_err(),
    "Expected '{}' to fail parsing",
    xpath
    );
}

#[rstest]
#[case::empty_parens_sequence("()()")]
#[case::single_open_paren("(")]
#[case::single_close_paren(")")]
#[case::mismatch_extra_open("(()")]
#[case::mismatch_extra_close("())")]
#[case::unclosed_arithmetic("(5 + 3")]
#[case::unmatched_closing_arithmetic("5 + 3)")]
#[case::missing_closing_paren("((5 + 3)")]
#[case::extra_closing_paren("(5 + 3))")]
#[case::empty_parens_before_number("()5")]
fn test_malformed_parentheses(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_err(),
    "Expected '{}' to fail parsing",
    xpath
    );
}

#[rstest]
#[case::multiple_decimal_points("123.456.789")]
#[case::decimal_start_end(".123.")]
#[case::double_decimal_point("..123")]
#[case::multiple_decimals_with_exponent("1.2.3e4")]
#[case::incomplete_sci_notation("1e")]
#[case::incomplete_positive_exponent("1e+")]
#[case::incomplete_negative_exponent("1e-")]
#[case::double_exponent("1ee5")]
#[case::decimal_in_exponent("1e5.5")]
fn test_malformed_numeric_literals(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_err(),
    "Expected '{}' to fail parsing",
    xpath
    );
}

#[rstest]
#[case::empty_descendant("//")]
#[case::incomplete_range("1 to")]
#[case::incomplete_book_predicate("book[")]
#[case::unmatched_bracket("book]")]
#[case::incomplete_attrib("@")]
#[case::invalid_path_separator("book..chapter")]
#[case::incomplete_comparison("book[@id=]")]
#[case::unclosed_string_literal("book[@id='unclosed")]
#[case::unclosed_double_string_literal("book[@id=\"unclosed")]
#[case::double_attribute_symbol("book[@@id]")]
#[case::nested_predicates("book[[1]]")]
#[case::multiple_equality_operators("book[@id=123=456")]
#[case::missing_operator_between_numbers("book[1 2]")]
#[case::trailing_slash("book/")]
#[case::invalid_numeric_suffix("5++")]
#[case::incomplete_logical_expression("book and")]
#[case::double_multiply_ops("5 * * 3")]
#[case::double_and_ops("5 and and 3")]
#[case::double_or_ops("5 or or 3")]
#[case::double_equality_ops("5 = = 3")]
#[case::conflicting_comparison_ops("5 < > 3")]
#[case::conflicting_comparison_ops_reversed("5 > < 3")]
fn test_syntax_error_cases(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_err(),
    "Expected '{}' to fail parsing",
    xpath
    );
}
