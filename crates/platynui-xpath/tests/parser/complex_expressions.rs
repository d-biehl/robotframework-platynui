use super::*;

#[rstest]
#[case::regex_and_bool_ops(
    "div[matches(@abc, \".*\") and @lo=123 or @qwe=true()]",
)]
#[case::string_funcs_path(
    "//book[@isbn and string-length(@isbn) = 13]/title[normalize-space(.) != '']",
)]
#[case::multiple_attr_conditions(
    "//product[@price > 100 and @category='electronics' and contains(@description, 'wireless')]",
)]
#[case::position_and_mod(
    "//div[contains(@class, 'main') and (position() mod 2 = 0) and last() > 5]",
)]
#[case::count_and_conditions(
    "//table[count(tr) > 10 and @id and starts-with(@class, 'data')]",
)]
#[case::nested_element_conditions(
    "//form[.//input[@type='password'] and .//input[@type='email']]",
)]
#[case::element_text_conditions(
    "//article[author[contains(., 'Smith')] or editor[contains(., 'Jones')]]",
)]
#[case::math_in_predicates(
    "//book[(@price * @discount div 100) < 20 and @published > 2020]",
)]
fn test_advanced_complex_expressions(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}': {:?}", xpath, result.err()
    );
}

#[rstest]
#[case::keywords_as_elements("//div[and, or, mod, div]")]
#[case::keywords_with_attributes("//for[@if='value']")]
#[case::keywords_with_text("//if[text()]")]
#[case::function_as_element("//function[@name]")]
#[case::comment_element("//comment()[@type]")]
#[case::text_as_element("//text[@node]")]
fn test_complex_expressions(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}': {:?}", xpath, result.err()
    );
}

#[rstest]
#[case::empty_attribute(
    "//div[@class=''][normalize-space(@class) = '']",
)]
#[case::empty_text("//element[. = '' or not(text())]")]
#[case::nan_check("//item[number(@price) = number(@price)]")]
#[case::ancestor_depth_limit("//div[count(ancestor::div) < 5]")]
#[case::table_structure("//table[count(.//tr) > count(.//th)]")]
#[case::non_empty_attr("//element[@attr][not(@attr = '')]")]
#[case::node_type_testing(
    "//node()[self::* or self::text() or self::comment()]",
)]
#[case::next_sibling_type(
    "//element[following-sibling::*[1][self::span]]",
)]
fn test_complex_edge_cases(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}': {:?}", xpath, result.err()
    );
}

#[rstest]
#[case::logical_grouping(
    "//div[(@class='a' or @class='b') and (@id='x' or @id='y')]",
)]
#[case::last_position("//element[position() = last()]")]
#[case::position_range("//item[position() > 5 and position() < 10]")]
#[case::nested_value_conditions("//book[price[. > 20 and . < 50]]")]
#[case::negation_in_predicates(
    "//product[not(@discontinued) and @available='true']",
)]
#[case::css_class_matching(
    "//div[contains(concat(' ', @class, ' '), ' active ')]",
)]
#[case::first_row_header("//table[tr[1]/th[contains(., 'Name')]]")]
#[case::multiple_negations(
    "//form//input[@type='text'][not(@readonly)][not(@disabled)]",
)]
#[case::multiple_element_alternatives("//section[h1 or h2 or h3][p]")]
#[case::complex_nested_conditions(
    "//article[date[@year > 2020] and author[position() <= 3]]",
)]
fn test_edge_cases_complex_scenarios(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}': {:?}", xpath, result.err()
    );
}

#[rstest]
#[case::kw_and("//and")]
#[case::kw_or("//or")]
#[case::kw_mod("//mod")]
#[case::kw_div("//div")]
#[case::kw_if("//if")]
// XPath 2.0 Flow Control Keywords
#[case::kw_for("for")]
#[case::kw_in("in")]
#[case::kw_return("return")]
#[case::kw_if2("if")]
#[case::kw_then("then")]
#[case::kw_else("else")]
#[case::kw_some("some")]
#[case::kw_every("every")]
#[case::kw_satisfies("satisfies")]
// XPath 2.0 Type Keywords
#[case::kw_instance("instance")]
#[case::kw_of("of")]
#[case::kw_treat("treat")]
#[case::kw_as("as")]
#[case::kw_cast("cast")]
#[case::kw_castable("castable")]
#[case::kw_to("to")]
// XPath 2.0 Set Operators
#[case::kw_union("union")]
#[case::kw_intersect("intersect")]
#[case::kw_except("except")]
// XPath 2.0 Logical Operators
#[case::kw_and2("and")]
#[case::kw_or2("or")]
// XPath 2.0 Value Comparison Operators
#[case::kw_eq("eq")]
#[case::kw_ne("ne")]
#[case::kw_lt("lt")]
#[case::kw_le("le")]
#[case::kw_gt("gt")]
#[case::kw_ge("ge")]
// XPath 2.0 Node Comparison
#[case::kw_is("is")]
// XPath 2.0 Arithmetic Operators (as elements)
#[case::kw_div2("div")]
#[case::kw_idiv("idiv")]
#[case::kw_mod2("mod")]
fn test_keywords_as_elements(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}': {:?}", xpath, result.err()
    );
}

#[rstest]
// Keywords in realistic path contexts
#[case::path_for("//for")]
#[case::path_and("/root/and")]
#[case::path_div_calc(
    "parent/div[@class='container']",
)]
#[case::path_or_between("//form/or/input")]
#[case::path_if_attr("doc/if[@condition='true']")]
#[case::path_eq_child("//table/eq/td")]
#[case::path_mod_attr(
    "/html/body/mod[@type='calculation']",
)]
#[case::path_union_config("config/union[@mode='append']")]
fn test_keywords_as_elements_in_paths(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}': {:?}", xpath, result.err()
    );
}
