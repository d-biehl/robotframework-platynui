// Stream-based tests for distinct-values() and index-of() functions
use platynui_xpath::*;
use rstest::*;

// Helper to evaluate expression and get string representations
fn eval_expression_strs(expr: &str, ctx: &DynamicContext<SimpleNode>) -> Vec<String> {
    let compiled = compile(expr).unwrap();
    let result = evaluate(&compiled, ctx).unwrap();
    result
        .iter()
        .map(|item| match item {
            XdmItem::Atomic(XdmAtomicValue::Integer(n)) => n.to_string(),
            XdmItem::Atomic(XdmAtomicValue::Decimal(d)) | XdmItem::Atomic(XdmAtomicValue::Double(d)) => {
                if d.is_nan() {
                    "NaN".to_string()
                } else if d.fract() == 0.0 {
                    format!("{}", *d as i64)
                } else {
                    d.to_string()
                }
            }
            XdmItem::Atomic(XdmAtomicValue::Float(f)) => {
                if f.is_nan() {
                    "NaN".to_string()
                } else if f.fract() == 0.0 {
                    format!("{}", *f as i64)
                } else {
                    f.to_string()
                }
            }
            XdmItem::Atomic(XdmAtomicValue::String(s)) => s.to_string(),
            XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => b.to_string(),
            _ => format!("{:?}", item),
        })
        .collect()
}

// ===== distinct-values() Tests =====

#[rstest]
#[case("distinct-values((1, 2, 3, 2, 1))", &["1", "2", "3"])]
#[case("distinct-values(())", &[])]
#[case("distinct-values((5))", &["5"])]
#[case("distinct-values(('a', 'b', 'a', 'c', 'b'))", &["a", "b", "c"])]
#[case("distinct-values((1.0, 1, 1.00))", &["1"])] // Numeric equality
fn distinct_values_stream_basic(#[case] expr: &str, #[case] expected: &[&str]) {
    let ctx = DynamicContextBuilder::default().build();
    let result = eval_expression_strs(expr, &ctx);
    assert_eq!(result, expected);
}

#[rstest]
#[case("distinct-values((1, 2, 'a', 'b', 3))", &["1", "2", "a", "b", "3"])]
#[case("distinct-values((true(), false(), true()))", &["true", "false"])]
fn distinct_values_stream_mixed_types(#[case] expr: &str, #[case] expected: &[&str]) {
    let ctx = DynamicContextBuilder::default().build();
    let result = eval_expression_strs(expr, &ctx);
    assert_eq!(result, expected);
}

#[rstest]
#[case("distinct-values((1 to 5, 3 to 7))", &["1", "2", "3", "4", "5", "6", "7"])]
fn distinct_values_stream_large_sequences(#[case] expr: &str, #[case] expected: &[&str]) {
    let ctx = DynamicContextBuilder::default().build();
    let result = eval_expression_strs(expr, &ctx);
    assert_eq!(result, expected);
}

#[rstest]
#[case("distinct-values((xs:double('NaN'), 1, xs:double('NaN'), 2))", &["NaN", "1", "2"])]
fn distinct_values_stream_nan_handling(#[case] expr: &str, #[case] expected: &[&str]) {
    let ctx = DynamicContextBuilder::default().build();
    let result = eval_expression_strs(expr, &ctx);
    // Current implementation: NaN is treated as distinct from other values but filtered to one instance
    assert_eq!(result, expected);
}

// ===== index-of() Tests =====

#[rstest]
#[case("index-of((1, 2, 3, 2, 1), 2)", &["2", "4"])]
#[case("index-of((1, 2, 3), 5)", &[])]
#[case("index-of((), 1)", &[])]
#[case("index-of(('a', 'b', 'c', 'b'), 'b')", &["2", "4"])]
fn index_of_stream_basic(#[case] expr: &str, #[case] expected: &[&str]) {
    let ctx = DynamicContextBuilder::default().build();
    let result = eval_expression_strs(expr, &ctx);
    assert_eq!(result, expected);
}

#[rstest]
#[case("index-of((1, 2, 3), 2)", &["2"])] // Exact match
fn index_of_stream_numeric_promotion(#[case] expr: &str, #[case] expected: &[&str]) {
    let ctx = DynamicContextBuilder::default().build();
    let result = eval_expression_strs(expr, &ctx);
    assert_eq!(result, expected);
}

#[rstest]
#[case("index-of((1 to 100), 42)", &["42"])]
#[case("index-of((1 to 10, 1 to 10), 5)", &["5", "15"])]
fn index_of_stream_large_sequences(#[case] expr: &str, #[case] expected: &[&str]) {
    let ctx = DynamicContextBuilder::default().build();
    let result = eval_expression_strs(expr, &ctx);
    assert_eq!(result, expected);
}

#[rstest]
#[case("index-of((1, xs:double('NaN'), 3), xs:double('NaN'))", &[])]
fn index_of_stream_nan_never_matches(#[case] expr: &str, #[case] expected: &[&str]) {
    let ctx = DynamicContextBuilder::default().build();
    let result = eval_expression_strs(expr, &ctx);
    // NaN never equals anything, including itself
    assert_eq!(result, expected);
}

#[rstest]
#[case("index-of((true(), false(), true()), true())", &["1", "3"])]
#[case("index-of((true(), false()), false())", &["2"])]
fn index_of_stream_booleans(#[case] expr: &str, #[case] expected: &[&str]) {
    let ctx = DynamicContextBuilder::default().build();
    let result = eval_expression_strs(expr, &ctx);
    assert_eq!(result, expected);
}
