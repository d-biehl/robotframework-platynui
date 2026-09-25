use platynui_xpath::engine::runtime::ErrorCode;
use platynui_xpath::{engine::evaluator::evaluate_expr, runtime::DynamicContextBuilder};
use rstest::rstest;

fn err_code(expr: &str) -> ErrorCode {
    let ctx = DynamicContextBuilder::new().build();
    evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx).unwrap_err().code_enum()
}

#[rstest]
#[case("xs:date('2025-13-40')")] // Lexically malformed
#[case("xs:date('2025-00-10')")] // Proper lexical but invalid month
#[case("xs:date('not-a-date')")] // Lexically malformed string
fn date_constructor_lexical_vs_range(#[case] expr: &str) {
    let e = err_code(expr);
    assert_eq!(e, ErrorCode::FORG0001);
}

#[rstest]
#[case("xs:time('25:00:00')")]
#[case("xs:time('23:60:00')")]
#[case("xs:time('23:00:60')")]
#[case("xs:time('bad')")]
fn time_constructor_lexical_vs_range(#[case] expr: &str) {
    let e = err_code(expr);
    assert_eq!(e, ErrorCode::FORG0001);
}

#[rstest]
#[case("xs:dateTime('2025-09-13T23:59:60')")]
#[case("xs:dateTime('2025-09-13Tnope')")]
fn datetime_constructor_lexical_vs_range(#[case] expr: &str) {
    let e = err_code(expr);
    assert_eq!(e, ErrorCode::FORG0001);
}
