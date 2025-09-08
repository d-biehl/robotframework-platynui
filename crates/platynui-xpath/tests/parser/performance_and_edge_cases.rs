use super::*;

#[rstest]
#[case::nesting_15_levels("//div[//span[//a[//img[@alt='test']]]]")]
#[case::path_15_levels(
    "//root/level1/level2/level3/level4/level5/level6/level7/level8/level9/level10/level11/level12/level13/level14/level15[@deep='very']",
)]
#[case::predicate_nesting_10(
    "//item[(@a=1 and (@b=2 and (@c=3 and (@d=4 and (@e=5 and (@f=6 and (@g=7 and (@h=8 and (@i=9 and @j=10)))))))))]",
)]
#[case::many_union_ops(
    "//a | //b | //c | //d | //e | //f | //g | //h | //i | //j | //k | //l | //m | //n | //o | //p | //q | //r | //s | //t",
)]
#[case::deep_element_nesting(
    "//product[price[currency[code[text()='USD' and @symbol='$']]]]",
)]
#[case::function_nesting_5(
    "concat(concat(concat(concat(concat('a','b'),'c'),'d'),'e'),'f')",
)]
#[case::string_function_chain(
    "//item[normalize-space(translate(substring(name(), 1, 10), 'ABCDEFGHIJ', 'abcdefghij')) = 'test']",
)]
#[case::multiple_count_ops(
    "//div[count(descendant::*) > 100 and count(ancestor::*) < 10 and count(following::*) > 50]",
)]
#[case::complex_position_calc(
    "//element[position() = (last() - 1) and following-sibling::*[position() = 1]/@type = 'end']",
)]
#[case::complex_math(
    "//data[sum(descendant::value) > avg(//data/value) * 1.5 + stddev(//data/value)]",
)]
fn test_extreme_nesting_performance(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse extreme nesting '{}': {:?}", xpath, result.err()
    );
    // Verify the parsed result is not empty
    let parsed = result.unwrap();
    assert!(
        parsed.len() > 0,
        "Parsed result should not be empty"
    );
}

#[rstest]
#[case::zh_element_attr("//产品[@价格 > 100]")]
#[case::ar_attr("//*[@النص = 'قيمة']")]
#[case::ru_ns_names("//рус:элемент[@атрибут = 'значение']")]
#[case::emoji_text_literal("//div[text() = '🔥 Hot Deal! 🎉']")]
#[case::el_ns("//ελληνικά:στοιχείο")]
#[case::ja_ns("//日本語:要素[@属性 = '値']")]
#[case::zh_fn("//测试[contains(@标题, '重要')]")]
#[case::es_special_chars("//item[@name = 'José María Azñar']")]
fn test_unicode_internationalization(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse unicode '{}': {:?}", xpath, result.err()
    );
    let parsed = result.unwrap();
    assert!(
        parsed.len() > 0,
        "Unicode expression should parse to non-empty result"
    );
}

#[rstest]
#[case::complex_price_calc(
    "//product[@price * @quantity * (1 + @tax_rate div 100) > 1000]",
)]
#[case::avg_vs_item_avg(
    "//item[round(sum(descendant::cost) div count(descendant::cost)) > avg(//item/cost)]",
)]
#[case::geometric_calc(
    "//circle[@radius * @radius * 3.14159 > @min_area]",
)]
#[case::compound_interest(
    "//loan[@principal * pow(1 + @rate div 12, @months) > @max_payment]",
)]
#[case::statistical_outlier(
    "//score[abs(@value - avg(//score/@value)) > 2 * stddev(//score/@value)]",
)]
#[case::trigonometric_comparison(
    "//angle[sin(@radians) > cos(@radians) and tan(@radians) > 1]",
)]
#[case::financial_calc(
    "//trade[@amount * @exchange_rate * (1 - @commission div 100) > @threshold]",
)]
#[case::area_ratio(
    "//rectangle[@width * @height > sqrt(@min_area) and @width div @height < @max_ratio]",
)]
#[case::arithmetic_chain(
    "//data[ceiling(@value div @divisor) * @multiplier + @offset = @target]",
)]
#[case::statistical_variance(
    "//stats[variance(child::value) > mean(child::value) * @sensitivity]",
)]
fn test_complex_mathematical_expressions(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse mathematical '{}': {:?}", xpath, result.err()
    );
    let parsed = result.unwrap();
    assert!(
        parsed.len() > 0,
        "Mathematical expression should produce result"
    );
}

#[rstest]
#[case::soap_response(
    "//soap:Envelope/soap:Body/tns:GetCustomerResponse/tns:Customer[tns:Status='Active']",
)]
#[case::wsdl_schema(
    "//wsdl:definitions/wsdl:types/xs:schema[@targetNamespace='http://example.com/service']",
)]
#[case::rss_feed(
    "//rss:channel/rss:item[rss:pubDate[contains(., '2024')] and rss:category='tech']",
)]
#[case::atom_feed(
    "//atom:feed/atom:entry[atom:updated > '2024-01-01T00:00:00Z']",
)]
#[case::xsi_schema_location(
    "//xsi:schemaLocation[contains(., 'http://www.w3.org/2001/XMLSchema-instance')]",
)]
#[case::config_xml(
    "//config:application/config:database[@type='postgresql']/config:connection[@pool='main']",
)]
#[case::maven_pom(
    "//pom:project/pom:dependencies/pom:dependency[pom:groupId='org.springframework']",
)]
#[case::spring_config(
    "//beans:bean[@class='com.example.Service']/beans:property[@name='dataSource']",
)]
#[case::jpa_persistence(
    "//persistence:persistence-unit[@name='default']/persistence:properties",
)]
#[case::log4j_config(
    "//log4j:configuration/log4j:appender[@type='RollingFileAppender']",
)]
#[case::maven_profile(
    "//maven:profile[@id='production']/maven:properties/maven:env='prod'",
)]
#[case::hibernate_mapping(
    "//hibernate:session-factory/hibernate:mapping[@resource='User.hbm.xml']",
)]
fn test_enterprise_xml_scenarios(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse enterprise XML '{}': {:?}", xpath, result.err()
    );
    let parsed = result.unwrap();
    assert!(
        parsed.len() > 0,
        "Enterprise expression should parse successfully"
    );
}
