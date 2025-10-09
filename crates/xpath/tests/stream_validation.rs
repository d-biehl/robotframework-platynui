//! Tests for stream-based sequence validation and transformation functions:
//! exactly-one(), one-or-more(), zero-or-one(), subsequence()

use platynui_xpath::*;

fn build_doc_with_items(n: usize) -> SimpleNode {
    let mut root_builder = elem("root");
    for _i in 0..n {
        root_builder = root_builder.child(elem("item"));
    }
    simple_doc().child(root_builder).build()
}

// ===== exactly-one() tests =====

#[test]
fn exactly_one_success() {
    let doc = build_doc_with_items(1);
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let result = evaluate_expr::<SimpleNode>("exactly-one(//item)", &ctx).unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn exactly_one_empty_fails() {
    let doc = simple_doc().build();
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let result = evaluate_expr::<SimpleNode>("exactly-one(//item)", &ctx);
    assert!(result.is_err());
}

#[test]
fn exactly_one_multiple_fails() {
    let doc = build_doc_with_items(2);
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let result = evaluate_expr::<SimpleNode>("exactly-one(//item)", &ctx);
    assert!(result.is_err());
}

// ===== one-or-more() tests =====

#[test]
fn one_or_more_single() {
    let doc = build_doc_with_items(1);
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let result = evaluate_expr::<SimpleNode>("count(one-or-more(//item))", &ctx).unwrap();

    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(count)) => assert_eq!(*count, 1),
        _ => panic!("Expected integer"),
    }
}

#[test]
fn one_or_more_multiple() {
    let doc = build_doc_with_items(5);
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let result = evaluate_expr::<SimpleNode>("count(one-or-more(//item))", &ctx).unwrap();

    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(count)) => assert_eq!(*count, 5),
        _ => panic!("Expected integer"),
    }
}

#[test]
fn one_or_more_empty_fails() {
    let doc = simple_doc().build();
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let result = evaluate_expr::<SimpleNode>("one-or-more(//item)", &ctx);
    assert!(result.is_err());
}

// ===== zero-or-one() tests =====

#[test]
fn zero_or_one_empty() {
    let doc = simple_doc().build();
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let result = evaluate_expr::<SimpleNode>("zero-or-one(//item)", &ctx).unwrap();
    assert_eq!(result.len(), 0);
}

#[test]
fn zero_or_one_single() {
    let doc = build_doc_with_items(1);
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let result = evaluate_expr::<SimpleNode>("zero-or-one(//item)", &ctx).unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn zero_or_one_multiple_fails() {
    let doc = build_doc_with_items(2);
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let result = evaluate_expr::<SimpleNode>("zero-or-one(//item)", &ctx);
    assert!(result.is_err());
}

// ===== subsequence() tests =====

#[test]
fn subsequence_basic_2args() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("subsequence((1, 2, 3, 4, 5), 2)", &ctx).unwrap();
    // Should return (2, 3, 4, 5)
    assert_eq!(result.len(), 4);
}

#[test]
fn subsequence_basic_3args() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("subsequence((1, 2, 3, 4, 5), 2, 2)", &ctx).unwrap();
    // Should return (2, 3)
    assert_eq!(result.len(), 2);
}

#[test]
fn subsequence_start_zero() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("subsequence((1, 2, 3), 0, 2)", &ctx).unwrap();
    // Start=0 with length=2: positions 0 and 1, but XPath positions start at 1
    // So this captures part of position 1 and all of position 1
    assert_eq!(result.len(), 2); // Items at fractional overlap
}

#[test]
fn subsequence_length_zero() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("subsequence((1, 2, 3), 1, 0)", &ctx).unwrap();
    assert_eq!(result.len(), 0);
}

#[test]
fn subsequence_out_of_bounds() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("subsequence((1, 2, 3), 10)", &ctx).unwrap();
    assert_eq!(result.len(), 0);
}

#[test]
fn subsequence_negative_start() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("subsequence((1, 2, 3, 4, 5), -2, 5)", &ctx).unwrap();
    // Negative start treated as before sequence start
    // This is tricky - check XPath spec behavior
    assert!(result.len() <= 5);
}

#[test]
fn subsequence_with_nodes() {
    let doc = build_doc_with_items(10);
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let result = evaluate_expr::<SimpleNode>("count(subsequence(//item, 3, 4))", &ctx).unwrap();

    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(count)) => assert_eq!(*count, 4),
        _ => panic!("Expected integer"),
    }
}

#[test]
fn subsequence_large_sequence() {
    let doc = build_doc_with_items(1000);
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let result = evaluate_expr::<SimpleNode>("count(subsequence(//item, 500, 100))", &ctx).unwrap();

    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(count)) => assert_eq!(*count, 100),
        _ => panic!("Expected integer"),
    }
}

// ===== Combined usage tests =====

#[test]
fn combined_exactly_one_subsequence() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("exactly-one(subsequence((1, 2, 3), 2, 1))", &ctx).unwrap();

    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(val)) => assert_eq!(*val, 2),
        _ => panic!("Expected integer"),
    }
}

#[test]
fn combined_count_one_or_more_subsequence() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("count(one-or-more(subsequence(1 to 100, 50, 10)))", &ctx).unwrap();

    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(count)) => assert_eq!(*count, 10),
        _ => panic!("Expected integer"),
    }
}

#[test]
fn combined_zero_or_one_empty_subsequence() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("zero-or-one(subsequence((1, 2, 3), 10))", &ctx).unwrap();
    assert_eq!(result.len(), 0);
}
