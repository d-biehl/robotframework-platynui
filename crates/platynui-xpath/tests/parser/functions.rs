use super::*;

#[rstest]
#[case::count("count(//book)")]
#[case::position("position()")]
#[case::last("last()")]
#[case::starts_with("starts-with(@name, 'A')")]
#[case::contains("contains(@title, 'XML')")]
#[case::substring("substring(@name, 1, 3)")]
#[case::string_length("string-length(@title)")]
#[case::normalize_space("normalize-space(@text)")]
#[case::sum("sum(//price)")]
#[case::concat("concat(@first, ' ', @last)")]
#[case::not("not(@disabled)")]
#[case::true_fn("true()")]
#[case::false_fn("false()")]
fn test_functions(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}': {:?}", xpath, result.err()
    );
}

#[rstest]
#[case::matches("matches(@class, \".*\")")]
#[case::replace("replace(@text, 'old', 'new')")]
#[case::tokenize("tokenize(@tags, ',')")]
#[case::substring2("substring(@name, 1, 5)")]
#[case::substring_before("substring-before(@email, '@')")]
#[case::substring_after("substring-after(@email, '@')")]
#[case::upper_case("upper-case(@name)")]
#[case::lower_case("lower-case(@title)")]
#[case::normalize_space2("normalize-space(@description)")]
#[case::translate("translate(@phone, '()-', '')")]
#[case::starts_with2("starts-with(@url, 'https')")]
#[case::ends_with("ends-with(@file, '.pdf')")]
#[case::number("number(@price)")]
#[case::string("string(@id)")]
#[case::boolean("boolean(@enabled)")]
#[case::floor("floor(@price * 1.2)")]
#[case::ceiling("ceiling(@rating)")]
#[case::round("round(@average)")]
#[case::abs("abs(@difference)")]
#[case::min("min(//product/@price)")]
#[case::max("max(//product/@price)")]
#[case::sum2("sum(//item/@quantity)")]
#[case::avg("avg(//rating/@value)")]
fn test_comprehensive_functions(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}': {:?}", xpath, result.err()
    );
}

#[rstest]
#[case::unclosed_position("position(")]
#[case::position_without_open("position)")]
#[case::unclosed_count("count(")]
#[case::empty_params("substring(,)")]
#[case::unclosed_normalize_space("normalize-space(")]
#[case::number_as_fn("5()")]
#[case::unclosed_position2("position(")]
#[case::position_missing_open_paren("position)")]
#[case::position_extra_open("position((")]
#[case::position_extra_close("position())")]
#[case::position_trailing_comma("position(1, 2, )")]
#[case::position_leading_comma("position(,)")]
#[case::position_double_comma("position(1,, 2)")]
#[case::number_fn_name("123()")]
#[case::string_fn_name("'string'()")]
fn test_malformed_function_calls(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_err(),
        "Expected to fail parsing: '{}'", xpath
    );
}
