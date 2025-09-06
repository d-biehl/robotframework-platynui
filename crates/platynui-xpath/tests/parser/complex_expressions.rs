use super::*;

#[rstest]
#[case("div[matches(@abc, \".*\") and @lo=123 or @qwe=true()]", "Complex predicate with regex and functions")]
#[case("//book[@isbn and string-length(@isbn) = 13]/title[normalize-space(.) != '']", "Complex path with string functions")]
#[case("//product[@price > 100 and @category='electronics' and contains(@description, 'wireless')]", "Multiple attribute conditions")]
#[case("//div[contains(@class, 'main') and (position() mod 2 = 0) and last() > 5]", "Position and modulo operations")]
#[case("//table[count(tr) > 10 and @id and starts-with(@class, 'data')]", "Count function with complex conditions")]
#[case("//form[.//input[@type='password'] and .//input[@type='email']]", "Nested element conditions")]
#[case("//article[author[contains(., 'Smith')] or editor[contains(., 'Jones')]]", "Complex element text conditions")]
#[case("//book[(@price * @discount div 100) < 20 and @published > 2020]", "Mathematical expressions in predicates")]
fn test_advanced_complex_expressions(#[case] xpath: &str, #[case] description: &str) {
    let result = XPathParser::parse_xpath(xpath);
    assert!(result.is_ok(), "Failed to parse {}: '{}'. Error: {:?}", 
            description, xpath, result.err());
}

#[rstest]
#[case("//div[and, or, mod, div]", "Keywords as elements")]
#[case("//for[@if='value']", "Keywords with attributes")]
#[case("//if[text()]", "Keywords with text content")]
#[case("//function[@name]", "Function as element")]
#[case("//comment()[@type]", "Comment element")]
#[case("//text[@node]", "Text as element")]
fn test_complex_expressions(#[case] xpath: &str, #[case] description: &str) {
    let result = XPathParser::parse_xpath(xpath);
    assert!(result.is_ok(), "Failed to parse {}: '{}'. Error: {:?}", 
            description, xpath, result.err());
}

#[rstest]
#[case("//div[@class=''][normalize-space(@class) = '']", "Empty attribute handling")]
#[case("//element[. = '' or not(text())]", "Empty text handling")]
#[case("//item[number(@price) = number(@price)]", "NaN checking")]
#[case("//div[count(ancestor::div) < 5]", "Ancestor depth limiting")]
#[case("//table[count(.//tr) > count(.//th)]", "Table structure validation")]
#[case("//element[@attr][not(@attr = '')]", "Non-empty attribute requirement")]
#[case("//node()[self::* or self::text() or self::comment()]", "Node type testing")]
#[case("//element[following-sibling::*[1][self::span]]", "Next sibling type check")]
fn test_complex_edge_cases(#[case] xpath: &str, #[case] description: &str) {
    let result = XPathParser::parse_xpath(xpath);
    assert!(result.is_ok(), "Failed to parse {}: '{}'. Error: {:?}", 
            description, xpath, result.err());
}

#[rstest]
#[case("//div[(@class='a' or @class='b') and (@id='x' or @id='y')]", "Complex logical grouping")]
#[case("//element[position() = last()]", "Last position")]
#[case("//item[position() > 5 and position() < 10]", "Position range")]
#[case("//book[price[. > 20 and . < 50]]", "Nested value conditions")]
#[case("//product[not(@discontinued) and @available='true']", "Negation in predicates")]
#[case("//div[contains(concat(' ', @class, ' '), ' active ')]", "CSS class matching")]
#[case("//table[tr[1]/th[contains(., 'Name')]]", "First row header check")]
#[case("//form//input[@type='text'][not(@readonly)][not(@disabled)]", "Multiple negations")]
#[case("//section[h1 or h2 or h3][p]", "Multiple element alternatives")]
#[case("//article[date[@year > 2020] and author[position() <= 3]]", "Complex nested conditions")]
fn test_edge_cases_complex_scenarios(#[case] xpath: &str, #[case] description: &str) {
    let result = XPathParser::parse_xpath(xpath);
    assert!(result.is_ok(), "Failed to parse {}: '{}'. Error: {:?}", 
            description, xpath, result.err());
}

#[rstest]
#[case("//and", "and keyword as element")]
#[case("//or", "or keyword as element")]
#[case("//mod", "mod keyword as element")]
#[case("//div", "div keyword as element")]
#[case("//if", "if keyword as element")]
// XPath 2.0 Flow Control Keywords
#[case("for", "for keyword as element")]
#[case("in", "in keyword as element")]
#[case("return", "return keyword as element")]
#[case("if", "if keyword as element")]
#[case("then", "then keyword as element")]
#[case("else", "else keyword as element")]
#[case("some", "some keyword as element")]
#[case("every", "every keyword as element")]
#[case("satisfies", "satisfies keyword as element")]
// XPath 2.0 Type Keywords
#[case("instance", "instance keyword as element")]
#[case("of", "of keyword as element")]
#[case("treat", "treat keyword as element")]
#[case("as", "as keyword as element")]
#[case("cast", "cast keyword as element")]
#[case("castable", "castable keyword as element")]
#[case("to", "to keyword as element")]
// XPath 2.0 Set Operators
#[case("union", "union keyword as element")]
#[case("intersect", "intersect keyword as element")]
#[case("except", "except keyword as element")]
// XPath 2.0 Logical Operators  
#[case("and", "and keyword as element")]
#[case("or", "or keyword as element")]
// XPath 2.0 Value Comparison Operators
#[case("eq", "eq keyword as element")]
#[case("ne", "ne keyword as element")]
#[case("lt", "lt keyword as element")]
#[case("le", "le keyword as element")]
#[case("gt", "gt keyword as element")]
#[case("ge", "ge keyword as element")]
// XPath 2.0 Node Comparison
#[case("is", "is keyword as element")]
// XPath 2.0 Arithmetic Operators (as elements)
#[case("div", "div keyword as element")]
#[case("idiv", "idiv keyword as element")]
#[case("mod", "mod keyword as element")]
fn test_keywords_as_elements(#[case] xpath: &str, #[case] description: &str) {
    let result = XPathParser::parse_xpath(xpath);
    assert!(result.is_ok(), "Failed to parse {}: '{}'. Error: {:?}", 
            description, xpath, result.err());
}

#[rstest]
// Keywords in realistic path contexts
#[case("//for", "for element in descendant path")]
#[case("/root/and", "and element in absolute path")]
#[case("parent/div[@class='container']", "div with div arithmetic keyword in path")]
#[case("//form/or/input", "or element between form and input")]
#[case("doc/if[@condition='true']", "if element with attribute")]
#[case("//table/eq/td", "eq element as table child")]
#[case("/html/body/mod[@type='calculation']", "mod element with namespaced attribute")]
#[case("config/union[@mode='append']", "union element in configuration")]
fn test_keywords_as_elements_in_paths(#[case] xpath: &str, #[case] description: &str) {
    let result = XPathParser::parse_xpath(xpath);
    assert!(result.is_ok(), "Failed to parse {}: '{}'. Error: {:?}", 
            description, xpath, result.err());
}
