//! Tests for stream-based aggregate functions:
//! sum(), avg(), min(), max()

use platynui_xpath::*;

// ===== sum() tests =====

#[test]
fn sum_empty_sequence() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("sum(())", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) => assert_eq!(*n, 0),
        _ => panic!("Expected integer 0"),
    }
}

#[test]
fn sum_with_zero_value() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("sum((), 42)", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) => assert_eq!(*n, 42),
        _ => panic!("Expected integer 42"),
    }
}

#[test]
fn sum_integers() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("sum((1, 2, 3, 4, 5))", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) => assert_eq!(*n, 15),
        _ => panic!("Expected integer"),
    }
}

#[test]
fn sum_decimals() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("sum((1.5, 2.5, 3.0))", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Decimal(d)) => assert!((d - 7.0).abs() < 0.001),
        _ => panic!("Expected decimal"),
    }
}

#[test]
fn sum_mixed_numeric_types() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("sum((1, 2.5, 3))", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Decimal(d)) => assert!((d - 6.5).abs() < 0.001),
        _ => panic!("Expected decimal"),
    }
}

#[test]
fn sum_with_nan() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("sum((1, 2, xs:double('NaN'), 3))", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Double(d)) => assert!(d.is_nan()),
        _ => panic!("Expected NaN"),
    }
}

// ===== avg() tests =====

#[test]
fn avg_empty_sequence() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("avg(())", &ctx).unwrap();
    assert_eq!(result.len(), 0); // Empty sequence returns empty
}

#[test]
fn avg_single_value() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("avg(42)", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Decimal(d)) if (*d as i64) == 42 => {}
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) if *n == 42 => {}
        _ => panic!("Expected 42"),
    }
}

#[test]
fn avg_integers() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("avg((1, 2, 3, 4, 5))", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Decimal(d)) => assert!((d - 3.0).abs() < 0.001),
        _ => panic!("Expected decimal 3.0"),
    }
}

#[test]
fn avg_decimals() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("avg((2.5, 3.5, 4.0))", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Decimal(d)) => {
            assert!((d - 3.333).abs() < 0.01, "Expected ~3.333, got {}", d);
        }
        _ => panic!("Expected decimal"),
    }
}

#[test]
fn avg_with_nan() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("avg((1, xs:double('NaN'), 3))", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Double(d)) => assert!(d.is_nan()),
        _ => panic!("Expected NaN"),
    }
}

// ===== min() tests =====

#[test]
fn min_empty_sequence() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("min(())", &ctx).unwrap();
    assert_eq!(result.len(), 0); // Empty sequence returns empty
}

#[test]
fn min_single_value() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("min(42)", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) => assert_eq!(*n, 42),
        _ => panic!("Expected integer 42"),
    }
}

#[test]
fn min_integers() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("min((5, 2, 8, 1, 9))", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) => assert_eq!(*n, 1),
        _ => panic!("Expected integer 1"),
    }
}

#[test]
fn min_strings() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("min(('apple', 'banana', 'cherry'))", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert_eq!(s, "apple"),
        _ => panic!("Expected 'apple'"),
    }
}

#[test]
fn min_with_nan() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("min((1, 2, xs:double('NaN'), 0))", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Double(d)) => assert!(d.is_nan()),
        _ => panic!("Expected NaN"),
    }
}

// ===== max() tests =====

#[test]
fn max_empty_sequence() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("max(())", &ctx).unwrap();
    assert_eq!(result.len(), 0); // Empty sequence returns empty
}

#[test]
fn max_single_value() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("max(42)", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) => assert_eq!(*n, 42),
        _ => panic!("Expected integer 42"),
    }
}

#[test]
fn max_integers() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("max((5, 2, 8, 1, 9))", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) => assert_eq!(*n, 9),
        _ => panic!("Expected integer 9"),
    }
}

#[test]
fn max_strings() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("max(('apple', 'banana', 'cherry'))", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert_eq!(s, "cherry"),
        _ => panic!("Expected 'cherry'"),
    }
}

#[test]
fn max_with_nan() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("max((1, 2, xs:double('NaN'), 10))", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Double(d)) => assert!(d.is_nan()),
        _ => panic!("Expected NaN"),
    }
}

// ===== Combined usage tests =====

#[test]
fn combined_sum_avg() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("let $sum := sum(1 to 10) return avg(1 to 10)", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Decimal(d)) => assert!((d - 5.5).abs() < 0.001),
        _ => panic!("Expected decimal 5.5"),
    }
}

#[test]
fn combined_min_max() {
    let ctx = DynamicContextBuilder::default().build();
    let result = evaluate_expr::<SimpleNode>("min((1, 2, 3)) + max((1, 2, 3))", &ctx).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) => assert_eq!(*n, 4), // 1 + 3
        _ => panic!("Expected integer 4"),
    }
}

#[test]
fn combined_large_sequence() {
    let ctx = DynamicContextBuilder::default().build();
    let result =
        evaluate_expr::<SimpleNode>("count((sum(1 to 100), avg(1 to 100), min(1 to 100), max(1 to 100)))", &ctx)
            .unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(n)) => assert_eq!(*n, 4),
        _ => panic!("Expected count of 4"),
    }
}
