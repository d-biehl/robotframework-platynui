/// Test that count() uses the new stream-based implementation.
use platynui_xpath::*;

// ===== count() tests =====

#[test]
fn count_empty_sequence() {
    let doc = simple_doc().build();
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let result = evaluate_expr::<SimpleNode>("count(())", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) => assert_eq!(*n, 0),
        _ => panic!("Expected integer"),
    }
}

#[test]
fn count_single_item() {
    let doc = simple_doc().child(elem("root")).build();
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc.clone())).build();
    let result = evaluate_expr::<SimpleNode>("count(/root)", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) => assert_eq!(*n, 1),
        _ => panic!("Expected integer"),
    }
}

#[test]
fn count_multiple_children() {
    let doc = simple_doc().child(elem("root").child(elem("item")).child(elem("item")).child(elem("item"))).build();
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc.clone())).build();
    let result = evaluate_expr::<SimpleNode>("count(//item)", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) => assert_eq!(*n, 3),
        _ => panic!("Expected integer"),
    }
}

#[test]
fn count_large_sequence() {
    // Test streaming works for large sequences
    let result = evaluate_expr::<SimpleNode>("count(1 to 10000)", &DynamicContextBuilder::default().build()).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) => assert_eq!(*n, 10000),
        _ => panic!("Expected integer"),
    }
}

// ===== exists() tests =====

#[test]
fn exists_empty_sequence() {
    let doc = simple_doc().build();
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let result = evaluate_expr::<SimpleNode>("exists(())", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => assert!(!*b),
        _ => panic!("Expected boolean"),
    }
}

#[test]
fn exists_single_item() {
    let doc = simple_doc().child(elem("root")).build();
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc.clone())).build();
    let result = evaluate_expr::<SimpleNode>("exists(/root)", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("Expected boolean"),
    }
}

#[test]
fn exists_early_termination() {
    // exists() should stop after first item (O(1) not O(n))
    let result =
        evaluate_expr::<SimpleNode>("exists(1 to 1000000)", &DynamicContextBuilder::default().build()).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("Expected boolean"),
    }
}

// ===== empty() tests =====

#[test]
fn empty_empty_sequence() {
    let doc = simple_doc().build();
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let result = evaluate_expr::<SimpleNode>("empty(())", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("Expected boolean"),
    }
}

#[test]
fn empty_non_empty_sequence() {
    let doc = simple_doc().child(elem("root")).build();
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc.clone())).build();
    let result = evaluate_expr::<SimpleNode>("empty(/root)", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => assert!(!*b),
        _ => panic!("Expected boolean"),
    }
}

#[test]
fn empty_early_termination() {
    // empty() should stop after first item (O(1) not O(n))
    let result = evaluate_expr::<SimpleNode>("empty(1 to 1000000)", &DynamicContextBuilder::default().build()).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => assert!(!*b),
        _ => panic!("Expected boolean"),
    }
}

// ===== Combined usage test =====

#[test]
fn combined_sequence_predicates() {
    let doc = simple_doc().child(elem("root").child(elem("item")).child(elem("item"))).build();
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();

    // All three stream functions working together
    let result = evaluate_expr::<SimpleNode>("if (exists(//item)) then count(//item) else 0", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) => assert_eq!(*n, 2),
        _ => panic!("Expected integer"),
    }
}
