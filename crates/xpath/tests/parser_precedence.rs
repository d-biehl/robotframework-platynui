use platynui_xpath::parser::{ast, parse as parse_expr};
use rstest::rstest;

fn parse(expr: &str) -> ast::Expr {
    parse_expr(expr).expect("parse failed")
}

#[rstest]
fn union_vs_intersect_except() {
    // a union b intersect c -> (a union (b intersect c)) due to precedence
    match parse("a union b intersect c") {
        ast::Expr::SetOp { op, right, .. } => {
            assert!(matches!(op, ast::SetOp::Union));
            assert!(matches!(*right, ast::Expr::SetOp { op: ast::SetOp::Intersect, .. }));
        }
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
fn arithmetic_precedence_shape() {
    // 1 + 2 * 3 should group as (1 + (2 * 3))
    match parse("1 + 2 * 3") {
        ast::Expr::Binary { op: ast::BinaryOp::Add, right, .. } => {
            assert!(matches!(*right, ast::Expr::Binary { op: ast::BinaryOp::Mul, .. }));
        }
        x => panic!("unexpected: {:?}", x),
    }
}
