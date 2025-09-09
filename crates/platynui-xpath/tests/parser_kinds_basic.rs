use platynui_xpath::parser::{ast, parse_xpath};
use rstest::rstest;

fn parse(expr: &str) -> ast::Expr {
    parse_xpath(expr).expect("parse failed")
}

#[rstest]
#[case("node()", ast::KindTest::AnyKind)]
#[case("text()", ast::KindTest::Text)]
#[case("comment()", ast::KindTest::Comment)]
#[case("processing-instruction()", ast::KindTest::ProcessingInstruction(None))]
fn basic_kind_tests(#[case] input: &str, #[case] kind: ast::KindTest) {
    match parse(input) {
        ast::Expr::Path(p) => match &p.steps[0].test {
            ast::NodeTest::Kind(k) => assert_eq!(k, &kind),
            x => panic!("unexpected: {:?}", x),
        },
        x => panic!("unexpected: {:?}", x),
    }
}

#[test]
fn document_node_wrapped_element() {
    match parse("document-node(element(*))") {
        ast::Expr::Path(p) => match &p.steps[0].test {
            ast::NodeTest::Kind(ast::KindTest::Document(Some(inner))) => match inner.as_ref() {
                ast::KindTest::Element { .. } => {}
                x => panic!("unexpected inner: {:?}", x),
            },
            x => panic!("unexpected: {:?}", x),
        },
        x => panic!("unexpected: {:?}", x),
    }
}
