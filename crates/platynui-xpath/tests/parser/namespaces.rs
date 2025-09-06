use super::*;

#[rstest]
#[case("ns:book", "Namespaced element")]
#[case(
    "//ns:product[@ns:id='123']",
    "Namespaced element with namespaced attribute"
)]
#[case("html:div/html:span[@xml:lang='en']", "Multiple namespaces")]
#[case("//soap:Envelope/soap:Body/m:GetPrice", "SOAP namespace example")]
#[case("//rdf:Description[@rdf:about]", "RDF namespace example")]
#[case("xs:element[@name='person']", "XML Schema namespace")]
#[case("//office:document/office:body//text:p", "OpenDocument namespace")]
#[case(
    "local-name() = 'element' and namespace-uri() = 'http://example.com'",
    "Namespace functions"
)]
fn test_namespace_expressions(#[case] xpath: &str, #[case] description: &str) {
    let result = XPathParser::parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse {}: '{}'. Error: {:?}",
        description,
        xpath,
        result.err()
    );
}

#[rstest]
#[case("//ns:", "Namespace prefix without local name")]
#[case("//:", "Colon without namespace")]
#[case("//:element", "Colon at start")]
#[case("//ns::element", "Double colon in namespace")]
#[case("//ns:ele:ment", "Multiple colons")]
#[case("//123:element", "Numeric namespace prefix")]
#[case("//ns:123", "Numeric local name")]
#[case("//-ns:element", "Invalid namespace prefix")]
#[case("//ns:.element", "Dot in local name")]
fn test_invalid_namespace_syntax(#[case] xpath: &str, #[case] description: &str) {
    let result = XPathParser::parse_xpath(xpath);
    assert!(
        result.is_err(),
        "Expected {} to fail parsing: '{}'",
        description,
        xpath
    );
}
