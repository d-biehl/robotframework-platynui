use platynui_xpath::parser::{ast, parse_xpath};

fn parse(expr: &str) -> ast::Expr {
    parse_xpath(expr).expect("parse failed")
}

#[test]
fn schema_element_and_attribute() {
    match parse("schema-element(a)") {
        ast::Expr::Path(p) => match &p.steps[0].test {
            ast::NodeTest::Kind(ast::KindTest::SchemaElement(_)) => {}
            x => panic!("unexpected: {:?}", x),
        },
        x => panic!("unexpected: {:?}", x),
    }
    match parse("schema-attribute(a)") {
        ast::Expr::Path(p) => match &p.steps[0].test {
            ast::NodeTest::Kind(ast::KindTest::SchemaAttribute(_)) => {}
            x => panic!("unexpected: {:?}", x),
        },
        x => panic!("unexpected: {:?}", x),
    }
}

#[test]
fn document_node_with_schema_element() {
    match parse("document-node(schema-element(a))") {
        ast::Expr::Path(p) => match &p.steps[0].test {
            ast::NodeTest::Kind(ast::KindTest::Document(Some(inner))) => match inner.as_ref() {
                ast::KindTest::SchemaElement(_) => {}
                x => panic!("unexpected inner: {:?}", x),
            },
            x => panic!("unexpected: {:?}", x),
        },
        x => panic!("unexpected: {:?}", x),
    }
}
