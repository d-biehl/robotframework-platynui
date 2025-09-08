use super::*;
use rstest::rstest;

// Reserved function names (XPath 2.0 A.3) cannot be used unprefixed as function_qname

#[rstest]
#[case("element()", "reserved 'element' unprefixed should fail")]
#[case("empty-sequence()", "reserved 'empty-sequence' unprefixed should fail")]
#[case("document-node()", "reserved 'document-node' unprefixed should fail")]
#[case("node()", "reserved 'node' unprefixed should fail")]
#[case("typeswitch()", "reserved 'typeswitch' unprefixed should fail")]
fn test_reserved_function_names_unprefixed_should_fail(#[case] xpath: &str, #[case] _desc: &str) {
    let result = parse_xpath(xpath);
    assert!(result.is_err(), "Expected '{}' to fail as reserved function name", xpath);
}

#[rstest]
#[case("f:element()", "prefixed element allowed")]
#[case("x:empty-sequence()", "prefixed empty-sequence allowed")]
#[case("q:document-node()", "prefixed document-node allowed")]
#[case("p:node()", "prefixed node allowed")]
#[case("ns:typeswitch()", "prefixed typeswitch allowed")]
fn test_reserved_function_names_prefixed_should_pass(#[case] xpath: &str, #[case] _desc: &str) {
    let result = parse_xpath(xpath);
    assert!(result.is_ok(), "Expected '{}' to parse as prefixed function name", xpath);
}

