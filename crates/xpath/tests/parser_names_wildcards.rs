use platynui_xpath::parser::{ast, parse as parse_expr};
use rstest::rstest;

fn parse(expr: &str) -> ast::Expr {
    parse_expr(expr).expect("parse failed")
}

#[rstest]
#[case("a", "a")]
#[case("ns:a", "a")]
fn name_test_qname(#[case] input: &str, #[case] local: &str) {
    match parse(input) {
        ast::Expr::Path(p) => match &p.steps[0] {
            ast::Step::Axis { test, .. } => match test {
                ast::NodeTest::Name(ast::NameTest::QName(q)) => assert_eq!(q.local, local),
                x => panic!("unexpected: {:?}", x),
            },
            other => panic!("unexpected step: {:?}", other),
        },
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
#[case("*", ast::WildcardName::Any)]
#[case("*:a", ast::WildcardName::LocalWildcard("a".to_string()))]
#[case("ns:*", ast::WildcardName::NsWildcard("ns".to_string()))]
fn wildcard_name_tests(#[case] input: &str, #[case] expect: ast::WildcardName) {
    match parse(input) {
        ast::Expr::Path(p) => match &p.steps[0] {
            ast::Step::Axis { test, .. } => match test {
                ast::NodeTest::Name(ast::NameTest::Wildcard(w)) => assert_eq!(w, &expect),
                x => panic!("unexpected: {:?}", x),
            },
            other => panic!("unexpected step: {:?}", other),
        },
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
#[case("@a", true)]
#[case("a", false)]
fn wildcard_context_attribute_axis(#[case] input: &str, #[case] is_attr: bool) {
    match parse(input) {
        ast::Expr::Path(p) => match &p.steps[0] {
            ast::Step::Axis { axis, .. } => {
                assert_eq!(matches!(axis, ast::Axis::Attribute), is_attr)
            }
            other => panic!("unexpected step: {:?}", other),
        },
        x => panic!("unexpected: {:?}", x),
    }
}
