use super::*;

#[rstest]
#[case::add("1 + 2")]
#[case::sub("10 - 5")]
#[case::mul("3 * 4")]
#[case::div("10 div 2")]
#[case::mod_op("10 mod 3")]
#[case::idiv("8 idiv 3")]
#[case::and("@id and @title")]
#[case::or("@id or @title")]
#[case::lt("@price < 20")]
#[case::le("@price <= 20")]
#[case::gt("@price > 20")]
#[case::ge("@price >= 20")]
#[case::eq("@id = 'value'")]
#[case::ne("@id != 'value'")]
fn test_operators(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}': {:?}",
        xpath,
        result.err()
    );
}

#[rstest]
#[case::v_eq("$a eq $b")]
#[case::v_ne("$a ne $b")]
#[case::v_lt("$a lt $b")]
#[case::v_le("$a le $b")]
#[case::v_gt("$a gt $b")]
#[case::v_ge("$a ge $b")]
fn test_value_comparison_operators(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}': {:?}", xpath, result.err()
    );
}

#[rstest]
#[case::space_div("1 div 2")]
#[case::space_mod("5 mod 3")]
#[case::space_and("10 and 5")]
#[case::space_or("3 or 7")]
#[case::space_eq("1 eq 2")]
#[case::space_to("4 to 6")]
#[case::space_union("5 union 3")]
#[case::space_intersect("8 intersect 9")]
fn test_number_with_space_before_operators_should_pass(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    // With proper spacing, these should be valid XPath expressions
    assert!(
        result.is_ok(),
        "Should be valid with proper spacing: '{}'",
        xpath
    );
}

#[rstest]
#[case::num_space_div("1 div 2")]
#[case::num_space_mod("5 mod 3")]
#[case::num_space_and("10 and 5")]
#[case::num_space_or("3 or 7")]
#[case::num_space_eq("1 eq 2")]
#[case::num_space_ne("5 ne 3")]
#[case::num_space_lt("2 lt 8")]
fn test_number_with_proper_operator_spacing(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    // With proper spacing, these should be valid
    assert!(
        result.is_ok(),
        "Should be valid with proper spacing: '{}'",
        xpath
    );
}

#[rstest]
#[case::plus_then_mul("+ * 5")]
#[case::minus_then_mul("- * 3")]
#[case::double_mul("4 * * 2")]
#[case::double_div("8 / / 2")]
#[case::double_mod("5 mod mod 3")]
#[case::double_div_word("5 div div 2")]
#[case::double_and("true() and and false()")]
#[case::double_equals("1 == 2")]
fn test_invalid_operator_sequences(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_err(),
        "Expected to fail parsing: '{}'",
        xpath
    );
}

#[rstest]
#[case::concat_div("1div2")]
#[case::concat_mod("5mod3")]
#[case::concat_and("10and5")]
#[case::concat_or("3or7")]
#[case::concat_eq("1eq2")]
#[case::concat_ne("5ne3")]
#[case::concat_lt("2lt8")]
fn test_number_concatenated_with_operators_should_fail(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    // According to XPath 2.0 spec, these MUST fail - no space between number and operator
    assert!(
        result.is_err(),
        "MUST fail per XPath 2.0 spec: '{}'",
        xpath
    );
}

#[rstest]
#[case::precedence_and_or(
    "//item[@a = 1 and @b = 2 or @c = 3 and @d = 4 or @e = 5]",
)]
#[case::precedence_arithmetic_cmp(
    "//div[@x + @y * @z > @a - @b div @c]",
)]
#[case::precedence_or_grouping(
    "//node[@val = 1 or (@val = 2 and @type = 'A') or @val = 3]",
)]
#[case::precedence_not_and_or(
    "//elem[not(@active) and @status = 'pending' or @priority > 5]",
)]
#[case::precedence_mixed(
    "//item[@price * @qty + @tax > @budget and @category = 'A' or @discount > 0.1]",
)]
#[case::precedence_chained_eq(
    "//data[@x = @y and @y = @z and @a != @b and @b != @c]",
)]
#[case::precedence_axes(
    "//node[child::* > 5 and following::* < 10 or preceding::* = 0]",
)]
#[case::precedence_position_funcs(
    "//item[position() = 1 or position() = last() and @important = true()]",
)]
fn test_operator_precedence_stress(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse operator precedence '{}': {:?}",
        xpath,
        result.err()
    );
    let parsed = result.unwrap();
    assert!(
        parsed.len() > 0,
        "Precedence expression should parse"
    );
}
