use platynui_xpath::parser::{ast, parse_xpath};
use rstest::rstest;

fn parse(expr: &str) -> ast::Expr {
    parse_xpath(expr).expect("parse failed")
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
            assert_eq!(p.steps[0].axis, axis);
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
            assert_eq!(p.steps[0].axis, axis);
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
#[case("processing-instruction()")]
#[case("processing-instruction('xml-stylesheet')")]
fn pi_tests_parse(#[case] input: &str) {
    match parse(input) {
        ast::Expr::Path(p) => match &p.steps[0].test {
            ast::NodeTest::Kind(ast::KindTest::ProcessingInstruction(_)) => {}
            x => panic!("unexpected: {:?}", x),
        },
        x => panic!("unexpected: {:?}", x),
    }
}
