use platynui_xpath::parser::{ast, parse as parse_expr};
use rstest::rstest;

fn parse(expr: &str) -> ast::Expr {
    parse_expr(expr).expect("parse failed")
}

#[rstest]
#[case("some $x in (1,2), $y in (3,4) satisfies $x lt $y", ast::Quantifier::Some, 2)]
#[case("every $a in (1,2), $b in (1,2) satisfies $a ge $b", ast::Quantifier::Every, 2)]
fn quantified_multi_bindings(#[case] input: &str, #[case] kind: ast::Quantifier, #[case] n: usize) {
    match parse(input) {
        ast::Expr::Quantified { kind: k, bindings, .. } => {
            assert_eq!(k, kind);
            assert_eq!(bindings.len(), n);
        }
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
fn for_multi_bindings() {
    let e = parse("for $x in (1,2), $y in (3,4) return $x + $y");
    match e {
        ast::Expr::ForExpr { bindings, .. } => assert_eq!(bindings.len(), 2),
        x => panic!("unexpected: {:?}", x),
    }
}
