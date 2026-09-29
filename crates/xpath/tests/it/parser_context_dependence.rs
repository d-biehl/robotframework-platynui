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

/// An expression that computes a value reads the context wherever an operand, a condition, a
/// binding or a function argument does, and so does a function that reads the context item, its
/// position or its size when an argument is left out.
#[rstest]
#[case("count(.//x)")]
#[case("string-join(.//x/@Name, ', ')")]
#[case("exists(.//x)")]
#[case("1 + count(.//x)")]
#[case("-count(.//x)")]
#[case("count(.//x) > 0")]
#[case("1 to count(.//x)")]
#[case("count(.//x) instance of xs:integer")]
#[case("count(.//x) eq 0")]
#[case(". is //x")]
#[case(". treat as node()")]
#[case("string(.) castable as xs:integer")]
#[case("string(.) cast as xs:integer")]
#[case("if (exists(.//x)) then //a else //b")]
#[case("for $i in .//x return //y")]
#[case("let $a := .//x return $a")]
#[case("some $x in .//y satisfies true()")]
#[case("every $x in //y satisfies . = $x")]
#[case("root()/x")]
#[case("name()")]
#[case("fn:name()")]
#[case("local-name()")]
#[case("namespace-uri()")]
#[case("string()")]
#[case("string-length()")]
#[case("normalize-space()")]
#[case("number()")]
#[case("data()")]
#[case("root()")]
#[case("base-uri()")]
#[case("document-uri()")]
#[case("position()")]
#[case("last()")]
#[case("lang('en')")]
#[case("id('a')")]
#[case("element-with-id('a')")]
#[case("idref('a')")]
fn a_computed_value_reads_the_context_inside_it(#[case] expr: &str) {
    let parsed = parse(expr).expect("expression should parse");
    assert!(parsed.is_context_dependent(), "{expr} should be context-dependent");
}

/// The context read inside a predicate or a later step of a path is that step's own focus, and a
/// function that gets every argument, or reads no context at all, does not read it.
#[rstest]
#[case("count(//x)")]
#[case("count(//x) eq 0")]
#[case("string-join(//x/@Name, ', ')")]
#[case("//x[count(.//y) > 0]")]
#[case("(//x)[last()]")]
#[case("(//x)[. = 'y']")]
#[case("//x/string()")]
#[case("(//x)/name()")]
#[case("name(//x)")]
#[case("lang('en', //x)")]
#[case("id('a', //x)")]
#[case("true()")]
#[case("current-date()")]
#[case("for $i in //x return $i/y")]
#[case("some $x in //y satisfies $x/@IsEnabled")]
#[case("$v")]
#[case("'text'")]
fn a_computed_value_without_a_context_read_is_independent(#[case] expr: &str) {
    let parsed = parse(expr).expect("expression should parse");
    assert!(!parsed.is_context_dependent(), "{expr} should be context-independent");
}
