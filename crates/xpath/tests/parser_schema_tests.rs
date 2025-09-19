use platynui_xpath::parser::{ast, parse as parse_expr};
use rstest::rstest;

fn parse(expr: &str) -> ast::Expr {
    parse_expr(expr).expect("parse failed")
}

#[rstest]
fn schema_element_and_attribute() {
    match parse("schema-element(a)") {
        ast::Expr::Path(p) => match &p.steps[0] {
            ast::Step::Axis { test, .. } => match test {
                ast::NodeTest::Kind(ast::KindTest::SchemaElement(_)) => {}
                x => panic!("unexpected: {:?}", x),
            },
            other => panic!("unexpected step: {:?}", other),
        },
        x => panic!("unexpected: {:?}", x),
    }
    match parse("schema-attribute(a)") {
        ast::Expr::Path(p) => match &p.steps[0] {
            ast::Step::Axis { test, .. } => match test {
                ast::NodeTest::Kind(ast::KindTest::SchemaAttribute(_)) => {}
                x => panic!("unexpected: {:?}", x),
            },
            other => panic!("unexpected step: {:?}", other),
        },
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
fn document_node_with_schema_element() {
    match parse("document-node(schema-element(a))") {
        ast::Expr::Path(p) => match &p.steps[0] {
            ast::Step::Axis { test, .. } => match test {
                ast::NodeTest::Kind(ast::KindTest::Document(Some(inner))) => match inner.as_ref() {
                    ast::KindTest::SchemaElement(_) => {}
                    x => panic!("unexpected inner: {:?}", x),
                },
                x => panic!("unexpected: {:?}", x),
            },
            other => panic!("unexpected step: {:?}", other),
        },
        x => panic!("unexpected: {:?}", x),
    }
}
