use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::runtime::ErrorCode;
use platynui_xpath::{
    evaluate_expr,
    xdm::{XdmAtomicValue as A, XdmItem as I},
};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;
fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

fn eval_bool(expr: &str) -> bool {
    let out = evaluate_expr::<N>(expr, &ctx()).unwrap();
    match out.as_slice() {
        [I::Atomic(A::Boolean(b))] => *b,
        _ => panic!("expected boolean: {expr}"),
    }
}

// ---- General comparisons: > <= >= ----
#[rstest]
#[case("(1,5) > (0,-1)", true)] // 5>0 or 1>0
#[case("(1,2) > (3,4)", false)] // no pair with left>right
#[case("(xs:date('2024-01-03Z'), xs:date('2024-01-01Z')) > (xs:date('2024-01-02Z'))", true)]
#[case("(xs:date('2024-01-01Z')) > (xs:date('2024-01-02Z'))", false)]
#[case("(xs:time('18:00:00Z')) > (xs:time('17:59:59Z'))", true)]
#[case("(xs:time('12:00:00Z')) > (xs:time('18:00:00Z'))", false)]
#[case("(xs:dayTimeDuration('PT3H'), xs:dayTimeDuration('PT1H')) > (xs:dayTimeDuration('PT2H'))", true)]
#[case("(xs:yearMonthDuration('P3Y')) > (xs:yearMonthDuration('P5Y'))", false)]
fn general_gt_matrix(#[case] expr: &str, #[case] expect: bool) {
    assert_eq!(eval_bool(expr), expect, "expr={expr}");
}

#[rstest]
#[case("(1,5) <= (0,5)", true)] // 5<=5
#[case("(2,3) <= (1,2)", true)] // 2<=2
#[case("(4,5) <= (1,3)", false)] // no pair left<=right
#[case("(xs:date('2024-01-02Z')) <= (xs:date('2024-01-02Z'))", true)]
#[case("(xs:time('12:00:00Z')) <= (xs:time('12:00:00Z'))", true)]
#[case("(xs:dayTimeDuration('PT2H')) <= (xs:dayTimeDuration('PT1H'))", false)]
fn general_leq_matrix(#[case] expr: &str, #[case] expect: bool) {
    assert_eq!(eval_bool(expr), expect, "expr={expr}");
}

#[rstest]
#[case("(1,5) >= (5,9)", true)] // 5>=5
#[case("(1,2) >= (3,4)", false)]
#[case("(xs:yearMonthDuration('P2Y')) >= (xs:yearMonthDuration('P2Y'))", true)]
#[case("(xs:yearMonthDuration('P1Y')) >= (xs:yearMonthDuration('P2Y'))", false)]
fn general_geq_matrix(#[case] expr: &str, #[case] expect: bool) {
    assert_eq!(eval_bool(expr), expect, "expr={expr}");
}

// Incomparable general comparisons with new operators => false
#[rstest]
#[case("('a') > (xs:date('2024-01-01Z'))", false)]
#[case("(true()) >= (xs:time('12:00:00Z'))", false)]
fn incomparable_general_extended(#[case] expr: &str, #[case] expect: bool) {
    assert_eq!(eval_bool(expr), expect);
}

// ---- Value comparisons: le / ge ----
#[rstest]
#[case("1 le 1", true)]
#[case("1 le 2", true)]
#[case("2 le 1", false)]
// string lexicographic ordering (codepoint)
#[case("'a' le 'a'", true)]
#[case("'a' le 'b'", true)]
#[case("'b' le 'a'", false)]
#[case("'A' le 'a'", true)] // 'A' (65) < 'a' (97)
#[case("xs:date('2024-01-01Z') le xs:date('2024-01-02Z')", true)]
#[case("xs:date('2024-01-02Z') le xs:date('2024-01-01Z')", false)]
#[case("xs:time('12:00:00Z') le xs:time('12:00:00Z')", true)]
#[case("xs:dayTimeDuration('PT1H') le xs:dayTimeDuration('PT2H')", true)]
#[case("xs:yearMonthDuration('P2Y') le xs:yearMonthDuration('P1Y')", false)]
fn value_le(#[case] expr: &str, #[case] expect: bool) {
    assert_eq!(eval_bool(expr), expect);
}

#[rstest]
#[case("1 ge 1", true)]
#[case("2 ge 1", true)]
#[case("1 ge 2", false)]
// string lexicographic ordering (codepoint)
#[case("'a' ge 'a'", true)]
#[case("'b' ge 'a'", true)]
#[case("'a' ge 'b'", false)]
#[case("'a' ge 'A'", true)] // 'a'(97) > 'A'(65)
#[case("xs:date('2024-01-02Z') ge xs:date('2024-01-02Z')", true)]
#[case("xs:time('13:00:00Z') ge xs:time('12:00:00Z')", true)]
#[case("xs:dayTimeDuration('PT2H') ge xs:dayTimeDuration('PT3H')", false)]
#[case("xs:yearMonthDuration('P2Y') ge xs:yearMonthDuration('P1Y')", true)]
fn value_ge(#[case] expr: &str, #[case] expect: bool) {
    assert_eq!(eval_bool(expr), expect);
}

// ---- Boolean relational errors (value comparisons) ----
// Spec: boolean values are not ordered; using lt/le/gt/ge should raise type error (XPTY0004)
#[rstest]
#[case("true() lt false()")]
#[case("true() le true()")]
#[case("false() gt true()")]
#[case("false() ge false()")]
fn boolean_relational_errors(#[case] expr: &str) {
    let err = evaluate_expr::<N>(expr, &ctx()).expect_err("expected error");
    assert_eq!(err.code_enum(), ErrorCode::XPTY0004, "expected XPTY0004, got {:?}", err.code_qname());
}
