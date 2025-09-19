use platynui_xpath::parser::{ast, parse as parse_expr};
use rstest::rstest;

fn parse(expr: &str) -> ast::Expr {
    parse_expr(expr).expect("parse failed")
}

#[rstest]
#[case("child::a", ast::Axis::Child)]
#[case("descendant::a", ast::Axis::Descendant)]
#[case("attribute::a", ast::Axis::Attribute)]
#[case("self::node()", ast::Axis::SelfAxis)]
#[case("descendant-or-self::node()", ast::Axis::DescendantOrSelf)]
#[case("following-sibling::a", ast::Axis::FollowingSibling)]
#[case("following::a", ast::Axis::Following)]
#[case("namespace::*", ast::Axis::Namespace)]
fn forward_axes(#[case] input: &str, #[case] axis: ast::Axis) {
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
#[case("parent::node()", ast::Axis::Parent)]
#[case("ancestor::a", ast::Axis::Ancestor)]
#[case("preceding-sibling::a", ast::Axis::PrecedingSibling)]
#[case("preceding::a", ast::Axis::Preceding)]
#[case("ancestor-or-self::node()", ast::Axis::AncestorOrSelf)]
fn reverse_axes(#[case] input: &str, #[case] axis: ast::Axis) {
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
