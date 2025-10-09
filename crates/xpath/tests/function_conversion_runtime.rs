use platynui_xpath::compiler::compile;
use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::simple_node::{attr, doc as simple_doc, elem, text};
use platynui_xpath::xdm::{XdmAtomicValue as A, XdmItem as I};
use platynui_xpath::{SimpleNode, XdmNode, evaluate};
use rstest::rstest;

fn build_doc() -> SimpleNode {
    simple_doc()
        .child(
            elem("root")
                .child(elem("item").attr(attr("value", "Sample")))
                .child(elem("item").attr(attr("value", "Data")))
                .child(elem("item").child(text("2"))),
        )
        .build()
}

fn make_context(doc: &SimpleNode) -> platynui_xpath::DynamicContext<SimpleNode> {
    DynamicContextBuilder::default().with_context_item(I::Node(doc.clone())).build()
}

fn empty_context() -> platynui_xpath::DynamicContext<SimpleNode> {
    DynamicContextBuilder::default().build()
}

fn eval_expression(expr: &str) -> Vec<I<SimpleNode>> {
    let compiled = compile(expr).expect("compile");
    let ctx = empty_context();
    evaluate::<SimpleNode>(&compiled, &ctx).expect("eval")
}

fn eval_expression_err(expr: &str) -> platynui_xpath::engine::runtime::Error {
    let compiled = compile(expr).expect("compile");
    let ctx = empty_context();
    evaluate::<SimpleNode>(&compiled, &ctx).expect_err("expected error")
}

#[test]
fn string_length_atomizes_attributes() {
    let doc = build_doc();
    let ctx = make_context(&doc);
    let compiled = compile("string-length(//item[1]/@value)").expect("compile");
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::Integer(len)) => assert_eq!(*len, 6),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn contains_converts_strings() {
    let doc = build_doc();
    let ctx = make_context(&doc);
    let compiled = compile("contains(//item[1]/@value, //item[2]/@value)").expect("compile");
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::Boolean(value)) => assert!(!value),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn substring_casts_numeric_arguments() {
    let doc = build_doc();
    let ctx = make_context(&doc);
    let compiled = compile("substring(//item[1]/@value, //item[3]/text(), 3)").expect("compile");
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::String(s)) => assert_eq!(s, "amp"),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn number_handles_untyped_and_string_values() {
    let doc = build_doc();
    let ctx = make_context(&doc);
    let compiled = compile("number(//item[1]/@value)").expect("compile");
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::Double(v)) => assert!(v.is_nan()),
        other => panic!("unexpected result: {other:?}"),
    }

    let compiled_digits = compile("number(//item[3]/text())").expect("compile");
    let result_digits = evaluate::<SimpleNode>(&compiled_digits, &ctx).expect("eval");
    assert_eq!(result_digits.len(), 1);
    match &result_digits[0] {
        I::Atomic(A::Double(v)) => assert_eq!(*v, 2.0),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn string_length_reports_cardinality_errors() {
    let doc = build_doc();
    let ctx = make_context(&doc);
    let compiled = compile("string-length((//item/@value)[position() <= 2])").expect("compile");
    let err = evaluate::<SimpleNode>(&compiled, &ctx).expect_err("expected error");
    assert_eq!(err.code.local, "XPTY0004");
}

#[test]
fn substring_invalid_numeric_argument_raises_forg0001() {
    let doc = build_doc();
    let ctx = make_context(&doc);
    let compiled = compile("substring(//item[1]/@value, //item[2]/@value)").expect("compile");
    let err = evaluate::<SimpleNode>(&compiled, &ctx).expect_err("expected error");
    assert_eq!(err.code.local, "FORG0001");
}

#[test]
fn not_casts_boolean_arguments() {
    let doc = build_doc();
    let ctx = make_context(&doc);

    let compiled_true = compile("not('true')").expect("compile");
    let result_true = evaluate::<SimpleNode>(&compiled_true, &ctx).expect("eval");
    assert_eq!(result_true.len(), 1);
    match &result_true[0] {
        I::Atomic(A::Boolean(v)) => assert!(!v),
        other => panic!("unexpected result: {other:?}"),
    }

    let compiled_maybe = compile("not('maybe')").expect("compile");
    let result_maybe = evaluate::<SimpleNode>(&compiled_maybe, &ctx).expect("eval");
    assert_eq!(result_maybe.len(), 1);
    match &result_maybe[0] {
        I::Atomic(A::Boolean(v)) => assert!(!v),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn contains_accepts_anyuri_arguments() {
    let doc = build_doc();
    let ctx = make_context(&doc);
    let compiled = compile("contains('Sample', xs:anyURI('Sam'))").expect("compile");
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::Boolean(v)) => assert!(*v),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn compare_casts_string_arguments() {
    let doc = build_doc();
    let ctx = make_context(&doc);
    let compiled = compile("compare(//item[1]/@value, xs:anyURI('Sample'))").expect("compile");
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::Integer(v)) => assert_eq!(*v, 0),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[rstest]
fn min_rejects_non_string_collation() {
    let err = eval_expression_err("min(('a','b'), 1)");
    assert_eq!(err.code.local, "XPTY0004");
}

#[rstest]
fn max_accepts_string_collation() {
    let result = eval_expression("max(('a','b'), 'http://www.w3.org/2005/xpath-functions/collation/codepoint')");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::String(s)) => assert_eq!(s, "b"),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[rstest]
fn node_name_rejects_atomic() {
    let err = eval_expression_err("node-name(1)");
    assert_eq!(err.code.local, "XPTY0004");
}

#[rstest]
fn base_uri_rejects_atomic() {
    let err = eval_expression_err("base-uri(1)");
    assert_eq!(err.code.local, "XPTY0004");
}

#[rstest]
fn lang_rejects_non_node_context() {
    let err = eval_expression_err("lang('en', 'text')");
    assert_eq!(err.code.local, "XPTY0004");
}

#[rstest]
fn nilled_rejects_non_node() {
    let err = eval_expression_err("nilled('string')");
    assert_eq!(err.code.local, "XPTY0004");
}

#[rstest]
fn id_rejects_non_node_context() {
    let err = eval_expression_err("id(('a'), 1)");
    assert_eq!(err.code.local, "XPTY0004");
}

#[test]
fn matches_atomizes_nodes() {
    let doc = build_doc();
    let ctx = make_context(&doc);
    let compiled = compile("matches(//item[1]/@value, 'Sam.*')").expect("compile");
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::Boolean(v)) => assert!(*v),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn replace_handles_untyped_atomic() {
    let doc = build_doc();
    let ctx = make_context(&doc);
    let compiled = compile("replace(//item[1]/@value, 'S', 'X')").expect("compile");
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::String(s)) => assert_eq!(s, "Xample"),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn tokenize_casts_any_uri_flags_optional() {
    let doc = build_doc();
    let ctx = make_context(&doc);
    let compiled = compile("tokenize(xs:anyURI('a,b'), ',')").expect("compile");
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval");
    assert_eq!(result.len(), 2);
    assert!(matches!(&result[0], I::Atomic(A::String(s)) if s == "a"));
    assert!(matches!(&result[1], I::Atomic(A::String(s)) if s == "b"));
}

#[test]
fn matches_reports_invalid_regex() {
    let doc = build_doc();
    let ctx = make_context(&doc);
    let compiled = compile("matches('abc', '[')").expect("compile");
    let err = evaluate::<SimpleNode>(&compiled, &ctx).expect_err("expected error");
    assert_eq!(err.code.local, "FORX0002");
}

#[test]
fn subsequence_converts_numeric_arguments() {
    let doc = build_doc();
    let ctx = make_context(&doc);
    let compiled = compile("subsequence(//item/@value, //item[3]/text())").expect("compile");
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::String(s)) => assert_eq!(s, "Data"),
        I::Atomic(A::UntypedAtomic(s)) => assert_eq!(s, "Data"),
        I::Node(n) => assert_eq!(n.string_value(), "Data"),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn distinct_values_handles_untyped_atomic() {
    let doc = build_doc();
    let ctx = make_context(&doc);
    let compiled = compile("distinct-values(//item/@value)").expect("compile");
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval");
    assert_eq!(result.len(), 2);
    let mut values: Vec<String> = result
        .iter()
        .map(|i| match i {
            I::Atomic(A::String(s)) => s.clone(),
            I::Atomic(A::UntypedAtomic(s)) => s.clone(),
            other => panic!("unexpected result: {other:?}"),
        })
        .collect();
    values.sort_unstable();
    assert_eq!(values, vec!["Data".to_string(), "Sample".to_string()]);
}

#[test]
fn index_of_casts_arguments() {
    let doc = build_doc();
    let ctx = make_context(&doc);
    let compiled = compile("index-of(//item/@value, xs:anyURI('Sample'))").expect("compile");
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::Integer(i)) => assert_eq!(*i, 1),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn insert_before_casts_position() {
    let doc = build_doc();
    let ctx = make_context(&doc);
    let compiled = compile("insert-before(//item/@value, 2, 'Inserted')").expect("compile");
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval");
    assert_eq!(result.len(), 3);
    assert!(matches!(&result[1], I::Atomic(A::String(s)) if s == "Inserted"));
}

#[rstest]
#[case("insert-before(('a','b'), '2', 'x')", "XPTY0004")]
#[case("insert-before(('a','b'), 2.5, 'x')", "FOCA0001")]
fn insert_before_rejects_non_integer_position(#[case] expr: &str, #[case] code: &str) {
    let err = eval_expression_err(expr);
    assert_eq!(err.code.local, code);
}

#[rstest]
#[case("remove(('a','b'), '1')", "XPTY0004")]
#[case("remove(('a','b'), 1.2)", "FOCA0001")]
fn remove_rejects_non_integer_position(#[case] expr: &str, #[case] code: &str) {
    let err = eval_expression_err(expr);
    assert_eq!(err.code.local, code);
}

#[rstest]
#[case("sum((xs:yearMonthDuration('P1Y'), xs:yearMonthDuration('P2M')))", A::YearMonthDuration(14))]
#[case("sum((xs:dayTimeDuration('PT3S'), xs:dayTimeDuration('PT5S')))", A::DayTimeDuration(8))]
fn sum_accepts_duration_sequences(#[case] expr: &str, #[case] expected: A) {
    let compiled = compile(expr).expect("compile");
    let ctx = empty_context();
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(value) => assert_eq!(value, &expected),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[rstest]
#[case("avg((xs:yearMonthDuration('P2Y'), xs:yearMonthDuration('P4Y')))", A::YearMonthDuration(36))]
#[case("avg((xs:dayTimeDuration('PT10S'), xs:dayTimeDuration('PT20S')))", A::DayTimeDuration(15))]
fn avg_accepts_duration_sequences(#[case] expr: &str, #[case] expected: A) {
    let compiled = compile(expr).expect("compile");
    let ctx = empty_context();
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(value) => assert_eq!(value, &expected),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[rstest]
fn sum_uses_duration_zero_parameter() {
    let compiled = compile("sum((), xs:dayTimeDuration('PT5S'))").expect("compile");
    let ctx = empty_context();
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::DayTimeDuration(secs)) => assert_eq!(*secs, 5),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[rstest]
fn sum_rejects_string_sequence() {
    let err = eval_expression_err("sum(('1','2'))");
    assert_eq!(err.code.local, "XPTY0004");
}

#[rstest]
fn sum_accepts_untyped_atomic_literals() {
    let result = eval_expression("sum((xs:untypedAtomic('1'), xs:untypedAtomic('2')))");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::Double(v)) => assert_eq!(*v, 3.0),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[rstest]
fn round_accepts_integer_precision() {
    let result = eval_expression("round(xs:untypedAtomic('2.45'), 1)");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::Double(v)) => assert_eq!(*v, 2.5),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[rstest]
fn round_half_to_even_accepts_integer_precision() {
    let result = eval_expression("round-half-to-even(xs:double(2.55), 1)");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::Double(v)) => assert_eq!(*v, 2.6),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[rstest]
fn round_rejects_string_value() {
    let err = eval_expression_err("round('2.45')");
    assert_eq!(err.code.local, "XPTY0004");
}

#[rstest]
#[case("round(xs:double(2.45), 1.5)")]
#[case("round-half-to-even(xs:double(2.45), 1.5)")]
fn rounding_precision_rejects_non_integral(#[case] expr: &str) {
    let err = eval_expression_err(expr);
    assert_eq!(err.code.local, "FOCA0001");
}

#[rstest]
#[case("round(xs:double(2.45), '1')")]
#[case("round-half-to-even(xs:double(2.45), '1')")]
fn rounding_precision_rejects_string(#[case] expr: &str) {
    let err = eval_expression_err(expr);
    assert_eq!(err.code.local, "XPTY0004");
}

#[rstest]
#[case("round(2.75, ())", "round(2.75)")]
#[case("round-half-to-even(2.05, ())", "round-half-to-even(2.05)")]
fn rounding_precision_empty_sequence_defaults_to_zero(#[case] with_precision: &str, #[case] baseline: &str) {
    let with_precision_result = eval_expression(with_precision);
    let baseline_result = eval_expression(baseline);
    assert_eq!(with_precision_result, baseline_result);
}

#[rstest]
fn round_precision_cardinality_error() {
    let expr = "round(2.3, (1, 2))";
    let err = eval_expression_err(expr);
    assert_eq!(err.code.local, "XPTY0004");
}

#[test]
fn encode_for_uri_casts_anyuri_arguments() {
    let compiled = compile("encode-for-uri(xs:anyURI('a b'))").expect("compile");
    let ctx = make_context(&build_doc());
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::String(s)) => assert_eq!(s, "a%20b"),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn encode_for_uri_reports_cardinality_errors() {
    let ctx = make_context(&build_doc());
    let compiled = compile("encode-for-uri(('a','b'))").expect("compile");
    let err = evaluate::<SimpleNode>(&compiled, &ctx).expect_err("expected error");
    assert_eq!(err.code.local, "XPTY0004");
}

#[rstest]
#[case("string-length(1)")]
#[case("contains(1, 'a')")]
#[case("encode-for-uri(1)")]
fn string_functions_reject_numeric_arguments(#[case] expr: &str) {
    let err = eval_expression_err(expr);
    assert_eq!(err.code.local, "XPTY0004");
}

#[rstest]
#[case("contains(true(), 'a')")]
#[case("substring-before(true(), 'a')")]
fn string_functions_reject_boolean_arguments(#[case] expr: &str) {
    let err = eval_expression_err(expr);
    assert_eq!(err.code.local, "XPTY0004");
}

#[rstest]
fn codepoints_to_string_rejects_non_integer() {
    let err = eval_expression_err("codepoints-to-string(('a'))");
    assert_eq!(err.code.local, "XPTY0004");
}

#[rstest]
fn codepoints_to_string_accepts_integers() {
    let result = eval_expression("codepoints-to-string((97, 98))");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::String(s)) => assert_eq!(s, "ab"),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[rstest]
fn xs_boolean_multi_item_cardinality_error() {
    let err = eval_expression_err("xs:boolean((1,2))");
    assert_eq!(err.code.local, "XPTY0004");
}

#[rstest]
fn xs_boolean_accepts_numeric() {
    let result = eval_expression("xs:boolean(1)");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::Boolean(v)) => assert!(*v),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[rstest]
fn xs_qname_rejects_numeric_argument() {
    let err = eval_expression_err("xs:QName(1)");
    assert_eq!(err.code.local, "XPTY0004");
}

#[rstest]
fn xs_datetime_accepts_empty_sequence() {
    let result = eval_expression("xs:dateTime(())");
    assert!(result.is_empty());
}

#[rstest]
fn trace_rejects_non_string_label() {
    let err = eval_expression_err("trace((), 1)");
    assert_eq!(err.code.local, "XPTY0004");
}

#[rstest]
fn error_rejects_non_qname_code() {
    let err = eval_expression_err("error('not:a:qname')");
    assert_eq!(err.code.local, "FORG0001");
}

#[rstest]
fn error_rejects_non_string_description() {
    let err = eval_expression_err("error(xs:QName('err:oops'), 1)");
    assert_eq!(err.code.local, "FORG0001");
}

#[test]
fn resolve_uri_allows_anyuri_arguments() {
    let compiled =
        compile("resolve-uri(xs:anyURI('docs/page'), xs:anyURI('http://example.com/base/'))").expect("compile");
    let ctx = make_context(&build_doc());
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::AnyUri(uri)) => {
            assert_eq!(uri, "http://example.com/base/docs/page")
        }
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn years_from_duration_converts_string_argument() {
    let compiled = compile("years-from-duration('P2Y6M')").expect("compile");
    let ctx = make_context(&build_doc());
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::Integer(v)) => assert_eq!(*v, 2),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn seconds_from_duration_converts_untyped_atomic() {
    let compiled = compile("seconds-from-duration(xs:untypedAtomic('PT12S'))").expect("compile");
    let ctx = make_context(&build_doc());
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::Decimal(v)) => assert_eq!(*v, 12.0),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn months_from_duration_cardinality_error() {
    let compiled = compile("months-from-duration(('P1Y', 'P2Y'))").expect("compile");
    let ctx = make_context(&build_doc());
    let err = evaluate::<SimpleNode>(&compiled, &ctx).expect_err("expected error");
    assert_eq!(err.code.local, "XPTY0004");
}

#[test]
fn years_from_duration_invalid_lexical_reports_forg0001() {
    let compiled = compile("years-from-duration('not-a-duration')").expect("compile");
    let ctx = make_context(&build_doc());
    let err = evaluate::<SimpleNode>(&compiled, &ctx).expect_err("expected error");
    assert_eq!(err.code.local, "FORG0001");
}

#[rstest]
fn year_from_datetime_rejects_non_datetime() {
    let err = eval_expression_err("year-from-dateTime(1)");
    assert_eq!(err.code.local, "XPTY0004");
}

#[rstest]
fn hours_from_time_rejects_non_time() {
    let err = eval_expression_err("hours-from-time(1)");
    assert_eq!(err.code.local, "XPTY0004");
}

#[rstest]
fn adjust_date_to_timezone_rejects_non_duration() {
    let err = eval_expression_err("adjust-date-to-timezone(xs:date('2020-01-01'), 1)");
    assert_eq!(err.code.local, "XPTY0004");
}

#[test]
fn namespace_uri_from_qname_converts_untyped_atomic() {
    let compiled = compile("namespace-uri-from-QName(xs:untypedAtomic('xml:lang'))").expect("compile");
    let ctx = make_context(&build_doc());
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::AnyUri(uri)) => {
            assert_eq!(uri, "http://www.w3.org/XML/1998/namespace")
        }
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn namespace_uri_from_qname_reports_unknown_prefix() {
    let compiled = compile("namespace-uri-from-QName(xs:untypedAtomic('u:item'))").expect("compile");
    let ctx = make_context(&build_doc());
    let err = evaluate::<SimpleNode>(&compiled, &ctx).expect_err("expected error");
    assert_eq!(err.code.local, "FONS0004");
}

#[rstest]
fn qname_accepts_empty_namespace() {
    let result = eval_expression("QName((), 'local')");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::QName { ns_uri, prefix, local }) => {
            assert!(ns_uri.is_none());
            assert!(prefix.is_none());
            assert_eq!(local, "local");
        }
        other => panic!("unexpected result: {other:?}"),
    }
}

#[rstest]
fn qname_rejects_non_string_namespace() {
    let err = eval_expression_err("QName(1, 'local')");
    assert_eq!(err.code.local, "XPTY0004");
}

#[rstest]
fn resolve_qname_rejects_non_element_context() {
    let err = eval_expression_err("resolve-QName('local', 1)");
    assert_eq!(err.code.local, "XPTY0004");
}

#[rstest]
fn namespace_uri_for_prefix_rejects_non_element() {
    let err = eval_expression_err("namespace-uri-for-prefix('p', 1)");
    assert_eq!(err.code.local, "XPTY0004");
}

#[rstest]
fn in_scope_prefixes_rejects_non_element() {
    let err = eval_expression_err("in-scope-prefixes(1)");
    assert_eq!(err.code.local, "XPTY0004");
}

#[test]
fn local_name_from_qname_converts_string() {
    let compiled = compile("local-name-from-QName('plain')").expect("compile");
    let ctx = make_context(&build_doc());
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::NCName(s)) => assert_eq!(s, "plain"),
        other => panic!("unexpected result: {other:?}"),
    }
}
