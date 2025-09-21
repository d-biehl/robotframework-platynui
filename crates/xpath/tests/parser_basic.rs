use platynui_xpath::parser::{ast, parse as parse_expr};
use rstest::rstest;

fn parse(expr: &str) -> ast::Expr {
    parse_expr(expr).expect("parse failed")
}

#[rstest]
fn literal_numbers_strings() {
    assert_eq!(parse("42"), ast::Expr::Literal(ast::Literal::Integer(42)));
    match parse("3.14") {
        ast::Expr::Literal(ast::Literal::Decimal(_)) => {}
        _ => panic!(),
    }
    match parse("1e3") {
        ast::Expr::Literal(ast::Literal::Double(_)) => {}
        _ => panic!(),
    }
    assert_eq!(parse("'a'"), ast::Expr::Literal(ast::Literal::String("a".into())));
    assert_eq!(parse("\"a\""), ast::Expr::Literal(ast::Literal::String("a".into())));
}

#[rstest]
fn arithmetic_and_logic() {
    let e = parse("1 + 2 * 3");
    // Expect (1 + (2 * 3))
    match e {
        ast::Expr::Binary { op: ast::BinaryOp::Add, .. } => {}
        _ => panic!("wrong shape"),
    }
    let e = parse("1 to 3");
    match e {
        ast::Expr::Range { .. } => {}
        _ => panic!(),
    }
    let e = parse("true() and false() or true()");
    match e {
        ast::Expr::Binary { op: ast::BinaryOp::Or, .. } => {}
        _ => panic!(),
    }
}

#[rstest]
fn comparisons() {
    let e = parse("1 = 1");
    match e {
        ast::Expr::GeneralComparison { .. } => {}
        _ => panic!(),
    }
    let e = parse("1 eq 1");
    match e {
        ast::Expr::ValueComparison { .. } => {}
        _ => panic!(),
    }
}

#[rstest]
fn sequences_and_parentheses() {
    let e = parse("(1, 2, 3)");
    match e {
        ast::Expr::Sequence(v) => assert_eq!(v.len(), 3),
        _ => panic!(),
    }
    let e = parse("()");
    match e {
        ast::Expr::Sequence(v) => assert!(v.is_empty()),
        _ => panic!(),
    }
}

#[rstest]
fn variables_and_functions() {
    let e = parse("$x");
    match e {
        ast::Expr::VarRef(q) => assert_eq!(q.local, "x"),
        _ => panic!(),
    }
    let e = parse("concat('a','b')");
    match e {
        ast::Expr::FunctionCall { name, args } => {
            assert_eq!(name.local, "concat");
            assert_eq!(args.len(), 2);
        }
        _ => panic!(),
    }
}

#[rstest]
fn paths_and_predicates() {
    let e = parse("/a/b");
    match e {
        ast::Expr::Path(p) => {
            assert!(matches!(p.start, ast::PathStart::Root));
            assert_eq!(p.steps.len(), 2);
        }
        _ => panic!(),
    }
    let e = parse("//book/title[1]");
    match e {
        ast::Expr::Path(p) => {
            assert!(matches!(p.start, ast::PathStart::Root));
            assert!(p.steps.len() >= 2);
        }
        _ => panic!(),
    }
    let e = parse("$n/title");
    match e {
        ast::Expr::PathFrom { .. } => {}
        _ => panic!(),
    }
}

#[rstest]
fn kind_tests_and_types() {
    let e = parse(". treat as item()*");
    match e {
        ast::Expr::TreatAs { ty, .. } => match ty {
            ast::SequenceType::Typed { .. } => {}
            _ => panic!(),
        },
        _ => panic!(),
    }
    let e = parse(". instance of empty-sequence()");
    match e {
        ast::Expr::InstanceOf { ty, .. } => match ty {
            ast::SequenceType::EmptySequence => {}
            _ => panic!(),
        },
        _ => panic!(),
    }
}

#[rstest]
fn if_some_every_for() {
    let e = parse("if (1) then 2 else 3");
    match e {
        ast::Expr::IfThenElse { .. } => {}
        _ => panic!(),
    }
    let e = parse("some $x in (1,2) satisfies $x gt 1");
    match e {
        ast::Expr::Quantified { kind: ast::Quantifier::Some, .. } => {}
        _ => panic!(),
    }
    let e = parse("every $x in (1,2) satisfies $x ge 1");
    match e {
        ast::Expr::Quantified { kind: ast::Quantifier::Every, .. } => {}
        _ => panic!(),
    }
    let e = parse("for $x in (1,2) return $x");
    match e {
        ast::Expr::ForExpr { bindings, .. } => {
            assert_eq!(bindings.len(), 1);
        }
        _ => panic!(),
    }
}
