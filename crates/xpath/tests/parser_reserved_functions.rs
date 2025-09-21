use platynui_xpath::engine::runtime::ErrorCode;
use platynui_xpath::parser::{ast, parse};
use rstest::rstest;

// Annex A.3 reserved function names
// Node tests should parse as Path with KindTest, not as function calls.
#[rstest]
#[case("node()", ast::KindTest::AnyKind)]
#[case("text()", ast::KindTest::Text)]
#[case("processing-instruction()", ast::KindTest::ProcessingInstruction(None))]
#[case("comment()", ast::KindTest::Comment)]
#[case("document-node()", ast::KindTest::Document(None))]
#[case(
    "element()",
    ast::KindTest::Element { name: None, ty: None, nillable: false }
)]
#[case(
    "attribute()",
    ast::KindTest::Attribute { name: None, ty: None }
)]
fn reserved_names_as_node_tests(#[case] input: &str, #[case] expect: ast::KindTest) {
    match parse(input).expect("parse failed") {
        ast::Expr::Path(p) => match &p.steps[0] {
            ast::Step::Axis { test, .. } => match test {
                ast::NodeTest::Kind(k) => assert_eq!(k, &expect),
                x => panic!("unexpected: {:?}", x),
            },
            other => panic!("unexpected step: {:?}", other),
        },
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
fn reserved_names_schema_variants() {
    // schema-element/attribute should also parse as node tests
    let q = |local: &str| ast::QName { prefix: None, local: local.into(), ns_uri: None };
    match parse("schema-element(a)").expect("parse failed") {
        ast::Expr::Path(p) => match &p.steps[0] {
            ast::Step::Axis { test, .. } => match test {
                ast::NodeTest::Kind(ast::KindTest::SchemaElement(qn)) => {
                    assert_eq!(qn, &q("a"))
                }
                x => panic!("unexpected: {:?}", x),
            },
            other => panic!("unexpected step: {:?}", other),
        },
        x => panic!("unexpected: {:?}", x),
    }
    match parse("schema-attribute(a)").expect("parse failed") {
        ast::Expr::Path(p) => match &p.steps[0] {
            ast::Step::Axis { test, .. } => match test {
                ast::NodeTest::Kind(ast::KindTest::SchemaAttribute(qn)) => {
                    assert_eq!(qn, &q("a"))
                }
                x => panic!("unexpected: {:?}", x),
            },
            other => panic!("unexpected step: {:?}", other),
        },
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
fn pi_with_target_is_node_test() {
    match parse("processing-instruction('xml-stylesheet')").expect("parse failed") {
        ast::Expr::Path(p) => match &p.steps[0] {
            ast::Step::Axis { test, .. } => match test {
                ast::NodeTest::Kind(ast::KindTest::ProcessingInstruction(Some(t))) => {
                    assert_eq!(t, "xml-stylesheet");
                }
                x => panic!("unexpected: {:?}", x),
            },
            other => panic!("unexpected step: {:?}", other),
        },
        x => panic!("unexpected: {:?}", x),
    }
}

// SequenceType reserved names should not be parsed as function calls in expression context.
#[rstest]
#[case("item()")]
#[case("empty-sequence()")]
fn reserved_sequence_type_names_rejected(#[case] input: &str) {
    let err = parse(input).expect_err("expected error");
    assert_eq!(err.code_enum(), ErrorCode::XPST0003);
}
