use platynui_xpath::engine::runtime::ErrorCode;
use platynui_xpath::parser::{ast, parse as parse_expr};
use rstest::rstest;

fn parse(expr: &str) -> ast::Expr {
    parse_expr(expr).expect("parse failed")
}

#[rstest]
#[case("fn:empty()", ("fn", "empty", 0))]
#[case("local:foo(1)", ("local", "foo", 1))]
#[case("foo(1, 2, 3)", ("", "foo", 3))]
fn function_calls_ok(#[case] input: &str, #[case] expect: (&str, &str, usize)) {
    match parse(input) {
        ast::Expr::FunctionCall { name, args } => {
            assert_eq!(name.prefix.as_deref().unwrap_or(""), expect.0);
            assert_eq!(name.local, expect.1);
            assert_eq!(args.len(), expect.2);
        }
        x => panic!("unexpected: {:?}", x),
    }
}

#[rstest]
fn reserved_function_name_rejected() {
    // reserved names like 'if' cannot appear as function_qname
    let err = parse_expr("if()").expect_err("expected error");
    assert_eq!(err.code_enum(), ErrorCode::XPST0003);
}

#[rstest]
fn function_call_trailing_comma() {
    let err = parse_expr("foo(1,)").expect_err("expected error");
    assert_eq!(err.code_enum(), ErrorCode::XPST0003);
}

#[rstest]
fn function_call_missing_arg_between_commas() {
    let err = parse_expr("foo(,1)").expect_err("expected error");
    assert_eq!(err.code_enum(), ErrorCode::XPST0003);
}
