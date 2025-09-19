use platynui_xpath::parser::{ast, parse as parse_expr};
use rstest::rstest;

fn parse(expr: &str) -> ast::Expr {
    parse_expr(expr).expect("parse failed")
}

#[rstest]
fn nested_function_in_args() {
    match parse("foo(bar(1), baz())") {
        ast::Expr::FunctionCall { args, .. } => {
            assert_eq!(args.len(), 2);
            assert!(matches!(args[0], ast::Expr::FunctionCall { .. }));
            assert!(matches!(args[1], ast::Expr::FunctionCall { .. }));
        }
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
fn sequence_as_argument() {
    match parse("foo((1,2,3))") {
        ast::Expr::FunctionCall { args, .. } => {
            assert!(matches!(args[0], ast::Expr::Sequence(_)));
        }
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
fn if_expr_in_arguments() {
    match parse("f(if (1) then 2 else 3)") {
        ast::Expr::FunctionCall { args, .. } => {
            assert!(matches!(args[0], ast::Expr::IfThenElse { .. }));
        }
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
fn complex_path_and_comparison_in_arguments() {
    // nested path with predicates and comparison
    match parse("f(//a/b[@id = 3 and position() lt 10])") {
        ast::Expr::FunctionCall { args, .. } => {
            assert!(matches!(args[0], ast::Expr::Path(_)));
        }
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
fn multi_level_nested_calls_and_sequences() {
    match parse("g(h(i(1), (2, j(3))), k(l(m(4)), 5))") {
        ast::Expr::FunctionCall { args, .. } => {
            assert_eq!(args.len(), 2);
            // first arg is h(...)
            match &args[0] {
                ast::Expr::FunctionCall { name, args: a } => {
                    assert_eq!(name.local, "h");
                    assert_eq!(a.len(), 2);
                    assert!(matches!(a[0], ast::Expr::FunctionCall { .. }));
                    assert!(matches!(a[1], ast::Expr::Sequence(_)));
                }
                x => panic!("unexpected: {:?}", x),
            }
            // second arg is k(...)
            match &args[1] {
                ast::Expr::FunctionCall { name, args: a } => {
                    assert_eq!(name.local, "k");
                    assert_eq!(a.len(), 2);
                    assert!(matches!(a[0], ast::Expr::FunctionCall { .. }));
                    assert!(matches!(a[1], ast::Expr::Literal(_)));
                }
                x => panic!("unexpected: {:?}", x),
            }
        }
        x => panic!("unexpected: {:?}", x),
    }
}
