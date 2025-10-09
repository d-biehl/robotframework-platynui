//! Tests for stream-based sequence transformation functions:
//! insert-before(), remove()

use platynui_xpath::*;

// ===== insert-before() tests =====

#[test]
fn insert_before_at_start() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("insert-before((2, 3, 4), 1, (1))", &ctx).unwrap();

    // Should be (1, 2, 3, 4)
    assert_eq!(result.len(), 4);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) => assert_eq!(*n, 1),
        _ => panic!("Expected integer"),
    }
}

#[test]
fn insert_before_in_middle() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("insert-before((1, 3, 4), 2, (2))", &ctx).unwrap();

    // Should be (1, 2, 3, 4)
    assert_eq!(result.len(), 4);
    match (&result[0], &result[1], &result[2], &result[3]) {
        (
            XdmItem::Atomic(XdmAtomicValue::Integer(a)),
            XdmItem::Atomic(XdmAtomicValue::Integer(b)),
            XdmItem::Atomic(XdmAtomicValue::Integer(c)),
            XdmItem::Atomic(XdmAtomicValue::Integer(d)),
        ) => {
            assert_eq!(*a, 1);
            assert_eq!(*b, 2);
            assert_eq!(*c, 3);
            assert_eq!(*d, 4);
        }
        _ => panic!("Expected integers"),
    }
}

#[test]
fn insert_before_at_end() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("insert-before((1, 2, 3), 10, (4, 5))", &ctx).unwrap();

    // Insert position beyond sequence length appends at end
    assert_eq!(result.len(), 5);
}

#[test]
fn insert_before_multiple_items() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("insert-before((1, 4), 2, (2, 3))", &ctx).unwrap();

    // Should be (1, 2, 3, 4)
    assert_eq!(result.len(), 4);
}

#[test]
fn insert_before_empty_sequence() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("insert-before((), 1, (1, 2, 3))", &ctx).unwrap();

    // Insert into empty sequence
    assert_eq!(result.len(), 3);
}

#[test]
fn insert_before_empty_insert() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("insert-before((1, 2, 3), 2, ())", &ctx).unwrap();

    // No change
    assert_eq!(result.len(), 3);
}

#[test]
fn insert_before_negative_position() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("insert-before((1, 2, 3), -1, (0))", &ctx).unwrap();

    // Negative position treated as 1 (start)
    assert_eq!(result.len(), 4);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) => assert_eq!(*n, 0),
        _ => panic!("Expected integer"),
    }
}

// ===== remove() tests =====

#[test]
fn remove_first_item() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("remove((1, 2, 3, 4), 1)", &ctx).unwrap();

    // Should be (2, 3, 4)
    assert_eq!(result.len(), 3);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) => assert_eq!(*n, 2),
        _ => panic!("Expected integer"),
    }
}

#[test]
fn remove_middle_item() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("remove((1, 2, 3, 4), 2)", &ctx).unwrap();

    // Should be (1, 3, 4)
    assert_eq!(result.len(), 3);
    match (&result[0], &result[1], &result[2]) {
        (
            XdmItem::Atomic(XdmAtomicValue::Integer(a)),
            XdmItem::Atomic(XdmAtomicValue::Integer(b)),
            XdmItem::Atomic(XdmAtomicValue::Integer(c)),
        ) => {
            assert_eq!(*a, 1);
            assert_eq!(*b, 3);
            assert_eq!(*c, 4);
        }
        _ => panic!("Expected integers"),
    }
}

#[test]
fn remove_last_item() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("remove((1, 2, 3), 3)", &ctx).unwrap();

    // Should be (1, 2)
    assert_eq!(result.len(), 2);
}

#[test]
fn remove_out_of_bounds() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("remove((1, 2, 3), 10)", &ctx).unwrap();

    // Position beyond sequence length - no change
    assert_eq!(result.len(), 3);
}

#[test]
fn remove_negative_position() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("remove((1, 2, 3), -1)", &ctx).unwrap();

    // Negative position treated as 1
    assert_eq!(result.len(), 2);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) => assert_eq!(*n, 2),
        _ => panic!("Expected integer"),
    }
}

#[test]
fn remove_from_single_item() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("remove((42), 1)", &ctx).unwrap();

    // Remove only item - empty sequence
    assert_eq!(result.len(), 0);
}

#[test]
fn remove_from_empty_sequence() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("remove((), 1)", &ctx).unwrap();

    // No change
    assert_eq!(result.len(), 0);
}

// ===== Combined usage tests =====

#[test]
fn combined_insert_remove() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("remove(insert-before((1, 3), 2, (2)), 4)", &ctx).unwrap();

    // insert-before gives (1, 2, 3), remove position 4 (out of bounds) → (1, 2, 3)
    assert_eq!(result.len(), 3);
}

#[test]
fn combined_remove_insert() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("insert-before(remove((1, 2, 3, 4), 2), 2, (2))", &ctx).unwrap();

    // remove gives (1, 3, 4), insert-before at pos 2 → (1, 2, 3, 4)
    assert_eq!(result.len(), 4);
}

#[test]
fn combined_count_insert_before() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("count(insert-before(1 to 100, 50, (999)))", &ctx).unwrap();

    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(count)) => assert_eq!(*count, 101),
        _ => panic!("Expected integer"),
    }
}

#[test]
fn combined_count_remove() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("count(remove(1 to 100, 50))", &ctx).unwrap();

    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(count)) => assert_eq!(*count, 99),
        _ => panic!("Expected integer"),
    }
}
