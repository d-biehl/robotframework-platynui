//! Tests for stream-based string functions:
//! concat(), string-join()

use platynui_xpath::*;

// ===== concat() tests =====

#[test]
fn concat_two_strings() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("concat('Hello', ' World')", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert_eq!(s, "Hello World"),
        _ => panic!("Expected string"),
    }
}

#[test]
fn concat_multiple_strings() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("concat('a', 'b', 'c', 'd', 'e')", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert_eq!(s, "abcde"),
        _ => panic!("Expected string 'abcde'"),
    }
}

#[test]
fn concat_mixed_types() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("concat('Count: ', 42, ', Total: ', 100)", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert_eq!(s, "Count: 42, Total: 100"),
        _ => panic!("Expected concatenated string"),
    }
}

#[test]
fn concat_empty_strings() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("concat('', 'hello', '')", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert_eq!(s, "hello"),
        _ => panic!("Expected 'hello'"),
    }
}

#[test]
fn concat_unicode() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("concat('Hällö', ' ', 'Wörld', '! 🌍')", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert_eq!(s, "Hällö Wörld! 🌍"),
        _ => panic!("Expected Unicode string"),
    }
}

// ===== string-join() tests =====

#[test]
fn string_join_empty_sequence() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("string-join((), ', ')", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert_eq!(s, ""),
        _ => panic!("Expected empty string"),
    }
}

#[test]
fn string_join_single_item() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("string-join('hello', ', ')", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert_eq!(s, "hello"),
        _ => panic!("Expected 'hello'"),
    }
}

#[test]
fn string_join_multiple_items() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("string-join(('apple', 'banana', 'cherry'), ', ')", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert_eq!(s, "apple, banana, cherry"),
        _ => panic!("Expected joined string"),
    }
}

#[test]
fn string_join_empty_separator() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("string-join(('a', 'b', 'c'), '')", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert_eq!(s, "abc"),
        _ => panic!("Expected 'abc'"),
    }
}

#[test]
fn string_join_newline_separator() {
    let ctx = DynamicContextBuilder::default().build();
    // Note: Character reference &#10; is preserved literally in XPath string
    let result = evaluate_expr::<SimpleNode>("string-join(('line1', 'line2', 'line3'), '&#10;')", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert_eq!(s, "line1&#10;line2&#10;line3"),
        _ => panic!("Expected character-reference-separated string"),
    }
}

#[test]
fn string_join_mixed_types() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("string-join((1, 2, 3, 4, 5), '-')", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert_eq!(s, "1-2-3-4-5"),
        _ => panic!("Expected '1-2-3-4-5'"),
    }
}

#[test]
fn string_join_large_sequence() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("string-join((1 to 10), ',')", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert_eq!(s, "1,2,3,4,5,6,7,8,9,10"),
        _ => panic!("Expected comma-separated sequence"),
    }
}

#[test]
fn string_join_unicode() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("string-join(('🍎', '🍌', '🍒'), ' | ')", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert_eq!(s, "🍎 | 🍌 | 🍒"),
        _ => panic!("Expected Unicode emoji string"),
    }
}

// ===== Combined usage tests =====

#[test]
fn combined_concat_string_join() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("concat('List: ', string-join(('a', 'b', 'c'), ', '))", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert_eq!(s, "List: a, b, c"),
        _ => panic!("Expected combined string"),
    }
}

#[test]
fn combined_nested_string_join() {
    let ctx = DynamicContextBuilder::default().build();
    let result =
        evaluate_expr::<SimpleNode>("string-join((concat('Hello', ' ', 'World'), 'Rust', 'XPath'), ' - ')", &ctx)
            .unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert_eq!(s, "Hello World - Rust - XPath"),
        _ => panic!("Expected nested concatenation"),
    }
}

#[test]
fn combined_count_concat() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("concat('Total items: ', count((1, 2, 3, 4, 5)))", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert_eq!(s, "Total items: 5"),
        _ => panic!("Expected 'Total items: 5'"),
    }
}
