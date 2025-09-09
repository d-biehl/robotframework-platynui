use platynui_xpath::parser::{ast, parse_xpath};

fn parse(expr: &str) -> ast::Expr {
    parse_xpath(expr).expect("parse failed")
}

#[test]
fn fn_min_simple_sequence() {
    match parse("fn:min((3,4,5))") {
        ast::Expr::FunctionCall { name, args } => {
            assert_eq!(name.local, "min");
            assert_eq!(name.prefix.as_deref().unwrap_or("fn"), "fn");
            assert_eq!(args.len(), 1);
            match &args[0] {
                ast::Expr::Sequence(items) => {
                    assert_eq!(items.len(), 3);
                }
                x => panic!("unexpected: {:?}", x),
            }
        }
        x => panic!("unexpected: {:?}", x),
    }
}

#[test]
fn fn_min_dates_sequence() {
    match parse("fn:min((fn:current-date(), xs:date(\"2001-01-01\")))") {
        ast::Expr::FunctionCall { name, args } => {
            assert_eq!(name.local, "min");
            assert_eq!(args.len(), 1);
            match &args[0] {
                ast::Expr::Sequence(items) => {
                    assert_eq!(items.len(), 2);
                    assert!(matches!(items[0], ast::Expr::FunctionCall { .. }));
                    assert!(matches!(items[1], ast::Expr::FunctionCall { .. }));
                }
                x => panic!("unexpected: {:?}", x),
            }
        }
        x => panic!("unexpected: {:?}", x),
    }
}
