use platynui_xpath::parser::{ast, parse as parse_expr};
use rstest::rstest;

fn parse(expr: &str) -> ast::Expr {
    parse_expr(expr).expect("parse failed")
}

#[rstest]
#[case("element(a)")]
#[case("element(*, xs:string)")]
#[case("element(a, xs:integer?)")]
fn element_kind_tests(#[case] input: &str) {
    match parse(input) {
        ast::Expr::Path(p) => match &p.steps[0] {
            ast::Step::Axis { test, .. } => match test {
                ast::NodeTest::Kind(ast::KindTest::Element { .. }) => {}
                x => panic!("unexpected: {:?}", x),
            },
            other => panic!("unexpected step: {:?}", other),
        },
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
#[case("attribute(a)")]
#[case("attribute(*, xs:string)")]
fn attribute_kind_tests(#[case] input: &str) {
    match parse(input) {
        ast::Expr::Path(p) => match &p.steps[0] {
            ast::Step::Axis { test, .. } => match test {
                ast::NodeTest::Kind(ast::KindTest::Attribute { .. }) => {}
                x => panic!("unexpected: {:?}", x),
            },
            other => panic!("unexpected step: {:?}", other),
        },
        x => panic!("unexpected: {:?}", x),
    }
}
