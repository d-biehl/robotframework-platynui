//! Tests for additional stream-based function implementations:
//! reverse(), unordered(), string-length()

use platynui_xpath::*;

fn build_doc_with_items(n: usize) -> SimpleNode {
    let mut root_builder = elem("root");
    for _i in 0..n {
        root_builder = root_builder.child(elem("item"));
    }
    simple_doc().child(root_builder).build()
}

// ===== reverse() tests =====

#[test]
fn reverse_empty_sequence() {
    let doc = simple_doc().build();
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let result = evaluate_expr::<SimpleNode>("reverse(//item)", &ctx).unwrap();
    assert_eq!(result.len(), 0);
}

#[test]
fn reverse_single_item() {
    let doc = build_doc_with_items(1);
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let result = evaluate_expr::<SimpleNode>("reverse(//item)", &ctx).unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn reverse_multiple_items() {
    let doc = simple_doc().child(elem("root").child(elem("a")).child(elem("b")).child(elem("c"))).build();

    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let result = evaluate_expr::<SimpleNode>("reverse(//a | //b | //c)", &ctx).unwrap();

    // Should be reversed: 3 items
    assert_eq!(result.len(), 3);
}

#[test]
fn reverse_large_sequence() {
    let doc = build_doc_with_items(1000);
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let result = evaluate_expr::<SimpleNode>("count(reverse(//item))", &ctx).unwrap();
    assert_eq!(result.len(), 1);

    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(count)) => assert_eq!(*count, 1000),
        _ => panic!("Expected integer count"),
    }
}

// ===== unordered() tests =====

#[test]
fn unordered_empty_sequence() {
    let doc = simple_doc().build();
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let result = evaluate_expr::<SimpleNode>("unordered(//item)", &ctx).unwrap();
    assert_eq!(result.len(), 0);
}

#[test]
fn unordered_single_item() {
    let doc = build_doc_with_items(1);
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let result = evaluate_expr::<SimpleNode>("unordered(//item)", &ctx).unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn unordered_multiple_items() {
    let doc = build_doc_with_items(5);
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let result = evaluate_expr::<SimpleNode>("count(unordered(//item))", &ctx).unwrap();

    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(count)) => assert_eq!(*count, 5),
        _ => panic!("Expected integer count"),
    }
}

#[test]
fn unordered_passthrough_zero_copy() {
    // unordered() should be a cheap passthrough (just clones the stream cursor)
    let doc = build_doc_with_items(10000);
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let result = evaluate_expr::<SimpleNode>("exists(unordered(//item))", &ctx).unwrap();

    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(exists)) => assert!(*exists),
        _ => panic!("Expected boolean"),
    }
}

// ===== string-length() tests =====

#[test]
fn string_length_empty_string() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("string-length('')", &ctx).unwrap();

    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(len)) => assert_eq!(*len, 0),
        _ => panic!("Expected integer"),
    }
}

#[test]
fn string_length_simple_string() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("string-length('hello')", &ctx).unwrap();

    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(len)) => assert_eq!(*len, 5),
        _ => panic!("Expected integer"),
    }
}

#[test]
fn string_length_unicode() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("string-length('Ä€📚')", &ctx).unwrap();

    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(len)) => assert_eq!(*len, 3), // 3 Unicode characters
        _ => panic!("Expected integer"),
    }
}

#[test]
fn string_length_from_number() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("string-length(string(42))", &ctx).unwrap();

    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(len)) => assert_eq!(*len, 2), // "42" has 2 characters
        _ => panic!("Expected integer"),
    }
}

// ===== Combined usage tests =====

#[test]
fn combined_reverse_count() {
    let doc = build_doc_with_items(7);
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let result = evaluate_expr::<SimpleNode>("count(reverse(//item))", &ctx).unwrap();

    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(count)) => assert_eq!(*count, 7),
        _ => panic!("Expected integer count"),
    }
}

#[test]
fn combined_unordered_exists() {
    let doc = build_doc_with_items(3);
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let result = evaluate_expr::<SimpleNode>("if (exists(unordered(//item))) then 'yes' else 'no'", &ctx).unwrap();

    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert_eq!(s, "yes"),
        _ => panic!("Expected string"),
    }
}

#[test]
fn combined_string_length_concat() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("string-length(concat('hello', ' ', 'world'))", &ctx).unwrap();

    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(len)) => assert_eq!(*len, 11), // "hello world"
        _ => panic!("Expected integer"),
    }
}
