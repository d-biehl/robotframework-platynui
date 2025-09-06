use super::*;

#[rstest]
#[case("//div[//span[//a[//img[@alt='test']]]]", "15-level deep nesting")]
#[case(
    "//root/level1/level2/level3/level4/level5/level6/level7/level8/level9/level10/level11/level12/level13/level14/level15[@deep='very']",
    "15-level path"
)]
#[case(
    "//item[(@a=1 and (@b=2 and (@c=3 and (@d=4 and (@e=5 and (@f=6 and (@g=7 and (@h=8 and (@i=9 and @j=10)))))))))]",
    "10-level predicate nesting"
)]
#[case(
    "//a | //b | //c | //d | //e | //f | //g | //h | //i | //j | //k | //l | //m | //n | //o | //p | //q | //r | //s | //t",
    "20 union operations"
)]
#[case(
    "//product[price[currency[code[text()='USD' and @symbol='$']]]]",
    "Deep element nesting"
)]
#[case(
    "concat(concat(concat(concat(concat('a','b'),'c'),'d'),'e'),'f')",
    "5-level function nesting"
)]
#[case(
    "//item[normalize-space(translate(substring(name(), 1, 10), 'ABCDEFGHIJ', 'abcdefghij')) = 'test']",
    "Complex string function chain"
)]
#[case(
    "//div[count(descendant::*) > 100 and count(ancestor::*) < 10 and count(following::*) > 50]",
    "Multiple count operations"
)]
#[case(
    "//element[position() = (last() - 1) and following-sibling::*[position() = 1]/@type = 'end']",
    "Complex position calculations"
)]
#[case(
    "//data[sum(descendant::value) > avg(//data/value) * 1.5 + stddev(//data/value)]",
    "Complex mathematical expression"
)]
fn test_extreme_nesting_performance(#[case] xpath: &str, #[case] description: &str) {
    let result = XPathParser::parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse extreme nesting {}: '{}'. Error: {:?}",
        description,
        xpath,
        result.err()
    );
    // Verify the parsed result is not empty
    let parsed = result.unwrap();
    assert!(
        parsed.len() > 0,
        "Parsed result should not be empty for {}",
        description
    );
}

#[rstest]
#[case("//产品[@价格 > 100]", "Chinese element and attribute names")]
#[case("//*[@النص = 'قيمة']", "Arabic attribute name")]
#[case("//рус:элемент[@атрибут = 'значение']", "Cyrillic namespace and names")]
#[case("//div[text() = '🔥 Hot Deal! 🎉']", "Emoji in string literals")]
#[case("//ελληνικά:στοιχείο", "Greek namespace")]
#[case("//日本語:要素[@属性 = '値']", "Japanese with namespace")]
#[case("//测试[contains(@标题, '重要')]", "Chinese with function")]
#[case("//item[@name = 'José María Azñar']", "Spanish special characters")]
fn test_unicode_internationalization(#[case] xpath: &str, #[case] description: &str) {
    let result = XPathParser::parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse unicode {}: '{}'. Error: {:?}",
        description,
        xpath,
        result.err()
    );
    let parsed = result.unwrap();
    assert!(
        parsed.len() > 0,
        "Unicode expression should parse to non-empty result: {}",
        description
    );
}

#[rstest]
#[case(
    "//product[@price * @quantity * (1 + @tax_rate div 100) > 1000]",
    "Complex price calculation"
)]
#[case(
    "//item[round(sum(descendant::cost) div count(descendant::cost)) > avg(//item/cost)]",
    "Average vs item average"
)]
#[case(
    "//circle[@radius * @radius * 3.14159 > @min_area]",
    "Geometric calculation"
)]
#[case(
    "//loan[@principal * pow(1 + @rate div 12, @months) > @max_payment]",
    "Compound interest"
)]
#[case(
    "//score[abs(@value - avg(//score/@value)) > 2 * stddev(//score/@value)]",
    "Statistical outlier"
)]
#[case(
    "//angle[sin(@radians) > cos(@radians) and tan(@radians) > 1]",
    "Trigonometric comparison"
)]
#[case(
    "//trade[@amount * @exchange_rate * (1 - @commission div 100) > @threshold]",
    "Financial calculation"
)]
#[case(
    "//rectangle[@width * @height > sqrt(@min_area) and @width div @height < @max_ratio]",
    "Area and ratio"
)]
#[case(
    "//data[ceiling(@value div @divisor) * @multiplier + @offset = @target]",
    "Complex arithmetic chain"
)]
#[case(
    "//stats[variance(child::value) > mean(child::value) * @sensitivity]",
    "Statistical variance"
)]
fn test_complex_mathematical_expressions(#[case] xpath: &str, #[case] description: &str) {
    let result = XPathParser::parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse mathematical {}: '{}'. Error: {:?}",
        description,
        xpath,
        result.err()
    );
    let parsed = result.unwrap();
    assert!(
        parsed.len() > 0,
        "Mathematical expression should produce result: {}",
        description
    );
}

#[rstest]
#[case(
    "//soap:Envelope/soap:Body/tns:GetCustomerResponse/tns:Customer[tns:Status='Active']",
    "SOAP web service response"
)]
#[case(
    "//wsdl:definitions/wsdl:types/xs:schema[@targetNamespace='http://example.com/service']",
    "WSDL schema definition"
)]
#[case(
    "//rss:channel/rss:item[rss:pubDate[contains(., '2024')] and rss:category='tech']",
    "RSS feed parsing"
)]
#[case(
    "//atom:feed/atom:entry[atom:updated > '2024-01-01T00:00:00Z']",
    "Atom feed with datetime"
)]
#[case(
    "//xsi:schemaLocation[contains(., 'http://www.w3.org/2001/XMLSchema-instance')]",
    "XML Schema Instance"
)]
#[case(
    "//config:application/config:database[@type='postgresql']/config:connection[@pool='main']",
    "Configuration XML"
)]
#[case(
    "//pom:project/pom:dependencies/pom:dependency[pom:groupId='org.springframework']",
    "Maven POM structure"
)]
#[case(
    "//beans:bean[@class='com.example.Service']/beans:property[@name='dataSource']",
    "Spring configuration"
)]
#[case(
    "//persistence:persistence-unit[@name='default']/persistence:properties",
    "JPA persistence.xml"
)]
#[case(
    "//log4j:configuration/log4j:appender[@type='RollingFileAppender']",
    "Log4j configuration"
)]
#[case(
    "//maven:profile[@id='production']/maven:properties/maven:env='prod'",
    "Maven profile"
)]
#[case(
    "//hibernate:session-factory/hibernate:mapping[@resource='User.hbm.xml']",
    "Hibernate mapping"
)]
fn test_enterprise_xml_scenarios(#[case] xpath: &str, #[case] description: &str) {
    let result = XPathParser::parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse enterprise XML {}: '{}'. Error: {:?}",
        description,
        xpath,
        result.err()
    );
    let parsed = result.unwrap();
    assert!(
        parsed.len() > 0,
        "Enterprise expression should parse successfully: {}",
        description
    );
}
