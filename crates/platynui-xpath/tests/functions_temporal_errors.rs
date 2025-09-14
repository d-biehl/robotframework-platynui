use platynui_xpath::engine::runtime::ErrorCode;
use platynui_xpath::{engine::evaluator::evaluate_expr, runtime::DynamicContextBuilder};

fn err_code(expr: &str) -> ErrorCode {
    let ctx = DynamicContextBuilder::new().build();
    evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx)
        .unwrap_err()
        .code_enum()
}

#[test]
fn date_constructor_lexical_vs_range() {
    // Lexically malformed
    let e = err_code("xs:date('2025-13-40')");
    assert_eq!(e, ErrorCode::FORG0001);

    // Proper lexical but invalid month
    let e = err_code("xs:date('2025-00-10')");
    assert_eq!(e, ErrorCode::FORG0001);

    // Lexically malformed string
    let e = err_code("xs:date('not-a-date')");
    assert_eq!(e, ErrorCode::FORG0001);
}

#[test]
fn time_constructor_lexical_vs_range() {
    let e = err_code("xs:time('25:00:00')");
    assert_eq!(e, ErrorCode::FORG0001);

    let e = err_code("xs:time('23:60:00')");
    assert_eq!(e, ErrorCode::FORG0001);

    let e = err_code("xs:time('23:00:60')");
    assert_eq!(e, ErrorCode::FORG0001);

    let e = err_code("xs:time('bad')");
    assert_eq!(e, ErrorCode::FORG0001);
}

#[test]
fn datetime_constructor_lexical_vs_range() {
    let e = err_code("xs:dateTime('2025-09-13T23:59:60')");
    assert_eq!(e, ErrorCode::FORG0001);

    let e = err_code("xs:dateTime('2025-09-13Tnope')");
    assert_eq!(e, ErrorCode::FORG0001);
}
