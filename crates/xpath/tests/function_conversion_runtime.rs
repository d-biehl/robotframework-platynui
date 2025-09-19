use platynui_xpath::compiler::compile;
use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::simple_node::{attr, doc as simple_doc, elem, text};
use platynui_xpath::xdm::{XdmAtomicValue as A, XdmItem as I};
use platynui_xpath::{SimpleNode, XdmNode, evaluate};

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
    DynamicContextBuilder::default()
        .with_context_item(I::Node(doc.clone()))
        .build()
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
    let compiled = compile("insert-before(//item/@value, '2', 'Inserted')").expect("compile");
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval");
    assert_eq!(result.len(), 3);
    assert!(matches!(&result[1], I::Atomic(A::String(s)) if s == "Inserted"));
}

#[test]
fn round_accepts_string_precision() {
    let compiled = compile("round(xs:untypedAtomic('2.45'), '1')").expect("compile");
    let ctx = make_context(&build_doc());
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval");
    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Atomic(A::Double(v)) => assert_eq!(*v, 2.5),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn round_precision_cardinality_error() {
    let doc = build_doc();
    let ctx = make_context(&doc);
    let compiled = compile("round(//item/@value, ('1','2'))").expect("compile");
    let err = evaluate::<SimpleNode>(&compiled, &ctx).expect_err("expected error");
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

#[test]
fn resolve_uri_allows_anyuri_arguments() {
    let compiled =
        compile("resolve-uri(xs:anyURI('docs/page'), xs:anyURI('http://example.com/base/'))")
            .expect("compile");
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

#[test]
fn namespace_uri_from_qname_converts_untyped_atomic() {
    let compiled =
        compile("namespace-uri-from-QName(xs:untypedAtomic('xml:lang'))").expect("compile");
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
    let compiled =
        compile("namespace-uri-from-QName(xs:untypedAtomic('u:item'))").expect("compile");
    let ctx = make_context(&build_doc());
    let err = evaluate::<SimpleNode>(&compiled, &ctx).expect_err("expected error");
    assert_eq!(err.code.local, "FONS0004");
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
