use super::*;
use rstest::rstest;

// Reserved function names (XPath 2.0 A.3) cannot be used unprefixed as function_qname

#[rstest]
#[case::element("element()")] 
#[case::empty_sequence("empty-sequence()")] 
#[case::document_node("document-node()")] 
#[case::node("node()")] 
#[case::typeswitch("typeswitch()")] 
fn test_reserved_function_names_unprefixed_should_fail(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(result.is_err(), "Expected '{}' to fail as reserved function name", xpath);
}

#[rstest]
#[case::element_prefixed("f:element()")]
#[case::empty_sequence_prefixed("x:empty-sequence()")]
#[case::document_node_prefixed("q:document-node()")]
#[case::node_prefixed("p:node()")]
#[case::typeswitch_prefixed("ns:typeswitch()")]
fn test_reserved_function_names_prefixed_should_pass(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(result.is_ok(), "Expected '{}' to parse as prefixed function name", xpath);
}
