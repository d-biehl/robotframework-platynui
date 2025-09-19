use platynui_xpath::parser::{ast, parse as parse_expr};
use rstest::rstest;

fn parse(expr: &str) -> ast::Expr {
    parse_expr(expr).expect("parse failed")
}

#[rstest]
#[case("a union b")]
#[case("a intersect b")]
#[case("a except b")]
fn set_ops(#[case] input: &str) {
    match parse(input) {
        ast::Expr::SetOp { .. } => {}
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
#[case(". cast as xs:string?")]
#[case(". castable as xs:integer")]
fn casts(#[case] input: &str) {
    match parse(input) {
        ast::Expr::CastAs { .. } | ast::Expr::CastableAs { .. } => {}
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
#[case(". instance of item()*")]
fn instance_of(#[case] input: &str) {
    match parse(input) {
        ast::Expr::InstanceOf { .. } => {}
        x => panic!("unexpected: {:?}", x),
    }
}
