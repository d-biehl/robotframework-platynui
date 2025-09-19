use platynui_xpath::parser::{ast, parse as parse_expr};
use rstest::rstest;

fn parse(expr: &str) -> ast::Expr {
    parse_expr(expr).expect("parse failed")
}

#[rstest]
#[case("ns:elem", (Some("ns"), "elem"))]
#[case("elem", (None, "elem"))]
#[case("@ns:attr", (Some("ns"), "attr"))]
#[case("😀", (None, "😀"))] // astral NameStartChar allowed in XML 1.1
#[case("ns:😀", (Some("ns"), "😀"))] // astral local with ASCII prefix
#[case("foo.bar", (None, "foo.bar"))] // '.' allowed as NameChar (not first)
#[case("ns:foo.bar", (Some("ns"), "foo.bar"))]
fn prefixes_in_qnames(#[case] input: &str, #[case] expect: (Option<&str>, &str)) {
    match parse(input) {
        ast::Expr::Path(p) => match &p.steps[0] {
            ast::Step::Axis { test, .. } => match test {
                ast::NodeTest::Name(ast::NameTest::QName(q)) => {
                    assert_eq!(q.prefix.as_deref(), expect.0);
                    assert_eq!(q.local, expect.1);
                }
                x => panic!("unexpected: {:?}", x),
            },
            other => panic!("unexpected step: {:?}", other),
        },
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
#[case("*:elem")]
#[case("ns:*")]
fn wildcard_with_prefixes(#[case] input: &str) {
    match parse(input) {
        ast::Expr::Path(p) => match &p.steps[0] {
            ast::Step::Axis { test, .. } => match test {
                ast::NodeTest::Name(ast::NameTest::Wildcard(_)) => {}
                x => panic!("unexpected: {:?}", x),
            },
            other => panic!("unexpected step: {:?}", other),
        },
        x => panic!("unexpected: {:?}", x),
    }
}
