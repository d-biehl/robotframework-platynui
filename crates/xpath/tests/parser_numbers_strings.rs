#![allow(clippy::approx_constant)]

use platynui_xpath::parser::{ast, parse as parse_expr};
use rstest::rstest;

fn parse(expr: &str) -> ast::Expr {
    parse_expr(expr).expect("parse failed")
}

#[rstest]
#[allow(clippy::approx_constant)]
#[case("0", ast::Literal::Integer(0))]
#[case("3.14", ast::Literal::Decimal(3.14))]
#[case("1e3", ast::Literal::Double(1000.0))]
fn numeric_literals(#[case] input: &str, #[case] lit: ast::Literal) {
    match parse(input) {
        ast::Expr::Literal(v) => assert_eq!(v, lit),
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
fn numeric_unary_minus() {
    match parse("-42") {
        ast::Expr::Unary { sign: ast::UnarySign::Minus, expr } => match *expr {
            ast::Expr::Literal(ast::Literal::Integer(42)) => {}
            x => panic!("unexpected inner: {:?}", x),
        },
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
#[case("'a'", "a")]
#[case("'it''s'", "it's")] // apostrophe doubled inside single quotes per XPath
#[case("\"a\"", "a")]
#[case("\"x\"\"y\"", "x\"y")] // doubled quotes inside strings
fn string_literals(#[case] input: &str, #[case] txt: &str) {
    match parse(input) {
        ast::Expr::Literal(ast::Literal::String(s)) => assert_eq!(s, txt),
        x => panic!("unexpected: {:?}", x),
    }
}
