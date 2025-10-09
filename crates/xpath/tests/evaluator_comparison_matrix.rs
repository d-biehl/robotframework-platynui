use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmAtomicValue as A, xdm::XdmItem as I};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;
fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

// Helper to evaluate an expression expected to yield a single boolean.
fn eval_bool(expr: &str) -> bool {
    let out = evaluate_expr::<N>(expr, &ctx()).unwrap_or_else(|e| panic!("{expr} => error: {e}"));
    match out.as_slice() {
        [I::Atomic(A::Boolean(b))] => *b,
        _ => panic!("expected single boolean: {expr:?} => {out:?}"),
    }
}

// General comparison matrix samples (not exhaustive numeric promotion tests; those live elsewhere).
// Spec: For general comparisons A OP B returns true if any pair of items is comparable and satisfies value comparison.
// Incomparable pairs must be ignored; if none comparable => false.
#[rstest]
#[case("(1, 'a') = (true(), 2)", false)] // boolean vs numeric incomparable; no successful pair
#[case("(1,2) = (3,4)", false)]
#[case("(1,2,3) = (5,4,3)", true)]
#[case("(xs:date('2024-01-01Z'), xs:date('2024-01-02Z')) = (xs:date('2023-12-31Z'), xs:date('2024-01-02Z'))", true)]
#[case("(xs:date('2024-01-01Z'), xs:date('2024-01-02Z')) = (xs:date('2024-01-03Z'))", false)]
#[case("(xs:time('12:00:00Z'), xs:time('18:00:00Z')) = (xs:time('18:00:00Z'))", true)]
#[case("(xs:time('12:00:00Z')) = (xs:time('18:00:00Z'))", false)]
#[case("(xs:dayTimeDuration('PT1H'), xs:dayTimeDuration('PT2H')) = (xs:dayTimeDuration('PT2H'))", true)]
#[case("(xs:dayTimeDuration('PT1H')) = (xs:dayTimeDuration('PT2H'))", false)]
#[case("(xs:yearMonthDuration('P1Y'), xs:yearMonthDuration('P2Y')) = (xs:yearMonthDuration('P2Y'))", true)]
#[case("(xs:yearMonthDuration('P1Y')) = (xs:yearMonthDuration('P2Y'))", false)]
#[case("(xs:yearMonthDuration('P1Y'), xs:dayTimeDuration('PT1H')) = (xs:yearMonthDuration('P1Y'))", true)]
// durations of different subtypes are incomparable for =; per spec, yearMonthDuration vs dayTimeDuration are not comparable => true only if same subtype match; left has P1Y; right P1Y; comparable so true.
#[case("(xs:yearMonthDuration('P1Y')) = (xs:dayTimeDuration('PT1H'))", false)]
fn general_eq_matrix(#[case] expr: &str, #[case] expect: bool) {
    assert_eq!(eval_bool(expr), expect, "expr={expr}");
}

#[rstest]
#[case("(1,2,3) != (4,5,6)", true)] // 1!=4 true
#[case("(1,2) != (1,2)", true)] // general '!=': any differing pair yields true
#[case("(xs:date('2024-01-01Z')) != (xs:date('2024-01-01Z'))", false)]
#[case("(xs:date('2024-01-01Z')) != (xs:date('2024-01-02Z'))", true)]
#[case("(xs:time('12:00:00Z')) != (xs:time('12:00:00Z'))", false)]
#[case("(xs:time('12:00:00Z')) != (xs:time('13:00:00Z'))", true)]
#[case("(xs:dayTimeDuration('PT1H')) != (xs:dayTimeDuration('PT1H'))", false)]
#[case("(xs:dayTimeDuration('PT1H')) != (xs:dayTimeDuration('PT2H'))", true)]
fn general_ne_matrix(#[case] expr: &str, #[case] expect: bool) {
    assert_eq!(eval_bool(expr), expect);
}

// Ordering comparisons for numbers, dates, times, durations
#[rstest]
#[case("(1,5) < (4,2)", true)] // 1<4
#[case("(5,6) < (1,2)", false)]
#[case("(xs:date('2024-01-01Z'), xs:date('2024-01-03Z')) < (xs:date('2024-01-02Z'))", true)]
#[case("(xs:date('2024-01-03Z')) < (xs:date('2024-01-02Z'))", false)]
#[case("(xs:time('12:00:00Z'), xs:time('18:00:00Z')) < (xs:time('13:00:00Z'))", true)]
#[case("(xs:time('18:00:00Z')) < (xs:time('13:00:00Z'))", false)]
#[case("(xs:dayTimeDuration('PT1H'), xs:dayTimeDuration('PT3H')) < (xs:dayTimeDuration('PT2H'))", true)]
#[case("(xs:dayTimeDuration('PT3H')) < (xs:dayTimeDuration('PT2H'))", false)]
#[case("(xs:yearMonthDuration('P1Y'), xs:yearMonthDuration('P3Y')) < (xs:yearMonthDuration('P2Y'))", true)]
#[case("(xs:yearMonthDuration('P3Y')) < (xs:yearMonthDuration('P2Y'))", false)]
fn general_lt_matrix(#[case] expr: &str, #[case] expect: bool) {
    assert_eq!(eval_bool(expr), expect);
}

// Incomparable general comparisons => false (e.g., string vs date)
#[rstest]
#[case("('a','b') = (xs:date('2024-01-01Z'))", false)]
#[case("('a') < (xs:date('2024-01-01Z'))", false)]
#[case("(xs:yearMonthDuration('P1Y')) = (xs:dayTimeDuration('PT1H'))", false)]
#[case("(true()) = (xs:date('2024-01-01Z'))", false)]
fn incomparable_general_eq(#[case] expr: &str, #[case] expect: bool) {
    assert_eq!(eval_bool(expr), expect);
}

// Value comparisons (singletons only) should error on empty or arity >1 before comparison; here we just test positive paths. For incomparable values we currently expect false.
#[rstest]
#[case("1 eq 1", true)]
#[case("1 eq 2", false)]
#[case("1 lt 2", true)]
#[case("2 lt 1", false)]
#[case("xs:date('2024-01-01Z') eq xs:date('2024-01-01Z')", true)]
#[case("xs:date('2024-01-01Z') lt xs:date('2024-01-02Z')", true)]
#[case("xs:date('2024-01-02Z') lt xs:date('2024-01-01Z')", false)]
#[case("xs:time('12:00:00Z') lt xs:time('13:00:00Z')", true)]
#[case("xs:dayTimeDuration('PT1H') lt xs:dayTimeDuration('PT2H')", true)]
#[case("xs:yearMonthDuration('P1Y') lt xs:yearMonthDuration('P2Y')", true)]
fn value_comparisons(#[case] expr: &str, #[case] expect: bool) {
    assert_eq!(eval_bool(expr), expect);
}
