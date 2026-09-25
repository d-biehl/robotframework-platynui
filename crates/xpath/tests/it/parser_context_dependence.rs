use platynui_xpath::parser::parse;
use rstest::rstest;

/// Absolute paths, filtered/parenthesized absolute paths and unions of absolutes do not select
/// nodes relative to the context node. A predicate has its own focus, so `//x[.='y']` stays
/// absolute, and a compound form is independent when every produced branch is absolute.
#[rstest]
#[case("//control:Button")]
#[case("/Window")]
#[case("(//control:Button)[1]")]
#[case("//x[.='y']")]
#[case("/a | //b")]
#[case("if (true()) then //x else //y")]
#[case("for $i in //x return //y")]
#[case("(//x, //y)")]
fn context_independent(#[case] expr: &str) {
    let parsed = parse(expr).expect("expression should parse");
    assert!(!parsed.is_context_dependent(), "{expr} should be context-independent");
}

/// Relative paths, the context item, unions with a relative operand, and compound forms whose
/// produced branch is relative all select nodes relative to the context node.
#[rstest]
#[case(".")]
#[case(".//x")]
#[case("./x")]
#[case("child::x")]
#[case(".//a | //b")]
#[case("if (true()) then .//x else //y")]
#[case("for $i in //x return .//y")]
#[case("let $a := //x return .//y")]
#[case("(.//x, //y)")]
fn context_dependent(#[case] expr: &str) {
    let parsed = parse(expr).expect("expression should parse");
    assert!(parsed.is_context_dependent(), "{expr} should be context-dependent");
}
