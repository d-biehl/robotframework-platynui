use super::*;

#[rstest]
#[case::ns_element("ns:book")]
#[case::ns_element_with_attr(
    "//ns:product[@ns:id='123']",
)]
#[case::multiple_ns("html:div/html:span[@xml:lang='en']")]
#[case::soap("//soap:Envelope/soap:Body/m:GetPrice")]
#[case::rdf("//rdf:Description[@rdf:about]")]
#[case::xs_schema("xs:element[@name='person']")]
#[case::opendocument("//office:document/office:body//text:p")]
#[case::ns_functions(
    "local-name() = 'element' and namespace-uri() = 'http://example.com'",
)]
fn test_namespace_expressions(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}': {:?}", xpath, result.err()
    );
}

#[rstest]
#[case::prefix_without_local("//ns:")]
#[case::just_colon("//:")]
#[case::colon_at_start("//:element")]
#[case::double_colon_in_ns("//ns::element")]
#[case::multiple_colons("//ns:ele:ment")]
#[case::numeric_prefix("//123:element")]
#[case::numeric_local("//ns:123")]
#[case::invalid_prefix("//-ns:element")]
#[case::dot_in_local("//ns:.element")]
fn test_invalid_namespace_syntax(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_err(),
        "Expected to fail parsing: '{}'", xpath
    );
}
