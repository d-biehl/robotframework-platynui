use platynui_xpath::parser::{ast, parse as parse_expr};
use rstest::rstest;

fn parse(expr: &str) -> ast::Expr {
    parse_expr(expr).expect("parse failed")
}

#[rstest]
#[case("42", ast::Expr::Literal(ast::Literal::Integer(42)))]
#[case("'a'", ast::Expr::Literal(ast::Literal::String("a".into())))]
#[case("\"b\"", ast::Expr::Literal(ast::Literal::String("b".into())))]
fn literals(#[case] input: &str, #[case] expected: ast::Expr) {
    assert_eq!(parse(input), expected);
}

#[rstest]
#[case("1 + 2 * 3", |e: &ast::Expr| matches!(e, ast::Expr::Binary { op: ast::BinaryOp::Add, .. }))]
#[case("1 to 3", |e: &ast::Expr| matches!(e, ast::Expr::Range { .. }))]
#[case("true() and false() or true()", |e: &ast::Expr| matches!(e, ast::Expr::Binary { op: ast::BinaryOp::Or, .. }))]
fn arithmetic_logic(#[case] input: &str, #[case] predicate: fn(&ast::Expr) -> bool) {
    let e = parse(input);
    assert!(predicate(&e), "unexpected AST: {:?}", e);
}

#[rstest]
#[case("1 = 1", |e: &ast::Expr| matches!(e, ast::Expr::GeneralComparison { .. }))]
#[case("1 eq 1", |e: &ast::Expr| matches!(e, ast::Expr::ValueComparison { .. }))]
fn comparisons_cases(#[case] input: &str, #[case] predicate: fn(&ast::Expr) -> bool) {
    let e = parse(input);
    assert!(predicate(&e), "unexpected AST: {:?}", e);
}

#[rstest]
#[case("(1, 2, 3)", 3)]
#[case("()", 0)]
fn sequences(#[case] input: &str, #[case] len: usize) {
    match parse(input) {
        ast::Expr::Sequence(v) => assert_eq!(v.len(), len),
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
#[case("$x", |e: &ast::Expr| matches!(e, ast::Expr::VarRef(_)))]
#[case("concat('a','b')", |e: &ast::Expr| match e { ast::Expr::FunctionCall { name, args } => name.local == "concat" && args.len() == 2, _ => false })]
fn vars_and_funcs(#[case] input: &str, #[case] predicate: fn(&ast::Expr) -> bool) {
    let e = parse(input);
    assert!(predicate(&e), "unexpected AST: {:?}", e);
}

#[rstest]
#[case("/a/b", |p: &ast::PathExpr| matches!(p.start, ast::PathStart::Root) && p.steps.len() == 2)]
#[case("//book/title[1]", |p: &ast::PathExpr| matches!(p.start, ast::PathStart::Root) && p.steps.len() >= 2)]
fn paths(#[case] input: &str, #[case] predicate: fn(&ast::PathExpr) -> bool) {
    match parse(input) {
        ast::Expr::Path(p) => assert!(predicate(&p)),
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
#[case(". treat as item()*", |e: &ast::Expr| match e { ast::Expr::TreatAs { ty, .. } => matches!(ty, ast::SequenceType::Typed { .. }), _ => false })]
#[case(". instance of empty-sequence()", |e: &ast::Expr| match e { ast::Expr::InstanceOf { ty, .. } => matches!(ty, ast::SequenceType::EmptySequence), _ => false })]
fn types_cases(#[case] input: &str, #[case] predicate: fn(&ast::Expr) -> bool) {
    let e = parse(input);
    assert!(predicate(&e), "unexpected AST: {:?}", e);
}

#[rstest]
#[case("if (1) then 2 else 3", |e: &ast::Expr| matches!(e, ast::Expr::IfThenElse { .. }))]
#[case("some $x in (1,2) satisfies $x gt 1", |e: &ast::Expr| matches!(e, ast::Expr::Quantified { kind: ast::Quantifier::Some, .. }))]
#[case("every $x in (1,2) satisfies $x ge 1", |e: &ast::Expr| matches!(e, ast::Expr::Quantified { kind: ast::Quantifier::Every, .. }))]
#[case("for $x in (1,2) return $x", |e: &ast::Expr| match e { ast::Expr::ForExpr { bindings, .. } => bindings.len() == 1, _ => false })]
fn control_flow(#[case] input: &str, #[case] predicate: fn(&ast::Expr) -> bool) {
    let e = parse(input);
    assert!(predicate(&e), "unexpected AST: {:?}", e);
}
