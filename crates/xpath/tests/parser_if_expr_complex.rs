use platynui_xpath::parser::{ast, parse as parse_expr};
use rstest::rstest;

fn parse(expr: &str) -> ast::Expr {
    parse_expr(expr).expect("parse failed")
}

#[rstest]
fn complex_if_then_else_nesting() {
    let src = r#"
if (fn:count($c) eq 0) then
    $zero
else if (fn:count($c) eq 1) then
    $c[1]
else
    $c[1] + fn:sum(subsequence($c, 2))
"#;
    match parse(src) {
        ast::Expr::IfThenElse { cond, then_expr, else_expr } => {
            // top-level cond is a value comparison
            assert!(matches!(*cond, ast::Expr::ValueComparison { .. }));
            // then_expr is a var ref
            assert!(matches!(*then_expr, ast::Expr::VarRef(_)));
            // else_expr should be another IfThenElse
            match *else_expr {
                ast::Expr::IfThenElse { cond: c2, then_expr: t2, else_expr: e2 } => {
                    assert!(matches!(*c2, ast::Expr::ValueComparison { .. }));
                    // t2 corresponds to $c[1]; depending on builder it can be a Filter over VarRef
                    assert!(
                        matches!(*t2, ast::Expr::Filter { .. } | ast::Expr::Path(_)),
                        "unexpected then-branch shape: {:?}",
                        *t2
                    );
                    // e2 is addition of path and function call
                    match *e2 {
                        ast::Expr::Binary { op, .. } => {
                            assert_eq!(op, ast::BinaryOp::Add);
                        }
                        x => panic!("unexpected else2: {:?}", x),
                    }
                }
                x => panic!("unexpected else: {:?}", x),
            }
        }
        x => panic!("unexpected: {:?}", x),
    }
}
