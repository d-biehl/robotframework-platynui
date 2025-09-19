use platynui_xpath::parser::{ast, parse as parse_expr};
use rstest::rstest;

fn parse(expr: &str) -> ast::Expr {
    parse_expr(expr).expect("parse failed")
}

#[rstest]
#[case(". is .", ast::NodeComp::Is)]
#[case(". << .", ast::NodeComp::Precedes)]
#[case(". >> .", ast::NodeComp::Follows)]
fn node_comparisons(#[case] input: &str, #[case] comp: ast::NodeComp) {
    match parse(input) {
        ast::Expr::NodeComparison { op, .. } => assert_eq!(op, comp),
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
#[case("--1")]
#[case("+-+1")]
fn unary_chains(#[case] input: &str) {
    match parse(input) {
        ast::Expr::Unary { .. } => {}
        x => panic!("unexpected: {:?}", x),
    }
}
