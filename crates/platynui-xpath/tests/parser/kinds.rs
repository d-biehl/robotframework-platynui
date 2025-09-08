use super::*;
use super::ast;
use rstest::rstest;

// XPath 2.0 Kind tests and wildcard NameTests

fn first_step(ast: &ast::Expr) -> &ast::Step {
    let path = match ast {
        ast::Expr::Path(p) => p,
        other => panic!("Expected Path expr, got: {:?}", other),
    };
    path.steps.first().expect("at least one step")
}

#[rstest]
fn test_processing_instruction_with_target_string() {
    let ast = parse_ast("processing-instruction('xml-stylesheet')");
    let step = first_step(&ast);
    match &step.test {
        ast::NodeTest::Kind(ast::KindTest::ProcessingInstruction(Some(t))) => {
            assert_eq!(t, "xml-stylesheet");
        }
        other => panic!("Expected PI kind test with target, got: {:?}", other),
    }
}

#[rstest]
fn test_document_node_kind_test() {
    let ast = parse_ast("document-node()");
    let step = first_step(&ast);
    match &step.test {
        ast::NodeTest::Kind(ast::KindTest::Document(None)) => {}
        other => panic!("Expected document-node() kind test, got: {:?}", other),
    }
}

#[rstest]
fn test_processing_instruction_without_target() {
    let ast = parse_ast("processing-instruction()");
    let step = first_step(&ast);
    match &step.test {
        ast::NodeTest::Kind(ast::KindTest::ProcessingInstruction(None)) => {}
        other => panic!("Expected PI kind test without target, got: {:?}", other),
    }
}

#[rstest]
fn test_processing_instruction_with_ncname_target() {
    let ast = parse_ast("processing-instruction(piTarget)");
    let step = first_step(&ast);
    match &step.test {
        ast::NodeTest::Kind(ast::KindTest::ProcessingInstruction(Some(t))) => {
            assert_eq!(t, "piTarget");
        }
        other => panic!("Expected PI kind test with ncname target, got: {:?}", other),
    }
}

#[rstest]
fn test_schema_element_attribute_grammar_only() {
    // Grammar acceptance only; AST builder may not capture arguments.
    assert!(parse_xpath("schema-element(ns:Name)").is_ok());
    assert!(parse_xpath("schema-attribute(ns:Name)").is_ok());
}

#[rstest]
fn test_element_attribute_with_params_grammar_only() {
    // element(*, xs:string?) and attribute(*, xs:string)
    assert!(parse_xpath("element(*, xs:string?)").is_ok());
    assert!(parse_xpath("attribute(*, xs:string)").is_ok());
}

#[rstest]
fn test_wildcard_ns_and_local() {
    // ns:*
    let ast = parse_ast("ns:*");
    let step = first_step(&ast);
    match &step.test {
        ast::NodeTest::Name(ast::NameTest::Wildcard(ast::WildcardName::NsWildcard(p))) => {
            assert_eq!(p, "ns");
        }
        other => panic!("Expected ns:* wildcard, got: {:?}", other),
    }

    // *:local
    let ast = parse_ast("*:local");
    let step = first_step(&ast);
    match &step.test {
        ast::NodeTest::Name(ast::NameTest::Wildcard(ast::WildcardName::LocalWildcard(l))) => {
            assert_eq!(l, "local");
        }
        other => panic!("Expected *:local wildcard, got: {:?}", other),
    }
}
