use platynui_xpath::parser::{ast, parse as parse_expr};
use rstest::rstest;

fn parse(expr: &str) -> ast::Expr {
    parse_expr(expr).expect("parse failed")
}

#[rstest]
#[case("@id", ast::Axis::Attribute)]
#[case("child::a", ast::Axis::Child)]
#[case("self::node()", ast::Axis::SelfAxis)]
#[case("descendant-or-self::node()", ast::Axis::DescendantOrSelf)]
fn axis_single_step(#[case] input: &str, #[case] axis: ast::Axis) {
    match parse(input) {
        ast::Expr::Path(p) => {
            assert_eq!(p.steps.len(), 1);
            match &p.steps[0] {
                ast::Step::Axis { axis: actual, .. } => assert_eq!(actual, &axis),
                other => panic!("unexpected step: {:?}", other),
            }
        }
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
#[case("..", ast::Axis::Parent)]
fn reverse_abbrev(#[case] input: &str, #[case] axis: ast::Axis) {
    match parse(input) {
        ast::Expr::Path(p) => {
            assert_eq!(p.steps.len(), 1);
            match &p.steps[0] {
                ast::Step::Axis { axis: actual, .. } => assert_eq!(actual, &axis),
                other => panic!("unexpected step: {:?}", other),
            }
        }
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
#[case("a//b", 3)] // a/descendant-or-self::node()/b
#[case("//node()", 2)] // implicit descendant-or-self + node()
fn double_slash_inserts_desc_or_self(#[case] input: &str, #[case] steps: usize) {
    match parse(input) {
        ast::Expr::Path(p) => assert_eq!(p.steps.len(), steps),
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
#[case("/.", 1)] // context item as the only step below the root
#[case("/(a|b)", 1)] // parenthesized expression as first step
#[case("/./root", 2)]
#[case("//.", 2)] // implicit descendant-or-self + context item
fn absolute_path_may_start_with_primary_expr(#[case] input: &str, #[case] steps: usize) {
    match parse(input) {
        ast::Expr::Path(p) => {
            assert!(matches!(p.start, ast::PathStart::Root));
            assert_eq!(p.steps.len(), steps);
            assert!(p.steps.iter().any(|s| matches!(s, ast::Step::FilterExpr(_))));
        }
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
#[case("processing-instruction()")]
#[case("processing-instruction('xml-stylesheet')")]
fn pi_tests_parse(#[case] input: &str) {
    match parse(input) {
        ast::Expr::Path(p) => match &p.steps[0] {
            ast::Step::Axis { test, .. } => match test {
                ast::NodeTest::Kind(ast::KindTest::ProcessingInstruction(_)) => {}
                x => panic!("unexpected: {:?}", x),
            },
            other => panic!("unexpected step: {:?}", other),
        },
        x => panic!("unexpected: {:?}", x),
    }
}
