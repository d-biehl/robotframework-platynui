// Tests for arithmetic on all XSD integer subtypes.
// Before the `classify()` fix, operations like `xs:long(5) + xs:short(3)` would fail
// with "non-numeric operand" because `classify()` only handled four variants.

use platynui_xpath::xdm::{XdmAtomicValue as A, XdmItem as I};
use platynui_xpath::{SimpleNode, evaluate_expr, runtime::DynamicContext};
use rstest::rstest;

type N = SimpleNode;

fn ctx() -> DynamicContext<N> {
    DynamicContext::default()
}

fn eval_i64(expr: &str) -> i64 {
    let c = ctx();
    let seq = evaluate_expr::<N>(expr, &c).expect("eval ok");
    match seq.first().expect("non-empty") {
        I::Atomic(a) => a.as_i128().expect("integer") as i64,
        other => panic!("expected atomic, got {other:?}"),
    }
}

fn eval_atomic(expr: &str) -> A {
    let c = ctx();
    let seq = evaluate_expr::<N>(expr, &c).expect("eval ok");
    match seq.first().expect("non-empty") {
        I::Atomic(a) => a.clone(),
        other => panic!("expected atomic, got {other:?}"),
    }
}

// --- Same-subtype arithmetic ---

#[rstest]
fn long_plus_long() {
    assert_eq!(eval_i64("xs:long(100) + xs:long(200)"), 300);
}

#[rstest]
fn short_plus_short() {
    assert_eq!(eval_i64("xs:short(10) + xs:short(20)"), 30);
}

#[rstest]
fn byte_plus_byte() {
    assert_eq!(eval_i64("xs:byte(5) + xs:byte(7)"), 12);
}

#[rstest]
fn unsigned_long_plus_unsigned_long() {
    assert_eq!(eval_i64("xs:unsignedLong(100) + xs:unsignedLong(200)"), 300);
}

#[rstest]
fn unsigned_int_plus_unsigned_int() {
    assert_eq!(eval_i64("xs:unsignedInt(50) + xs:unsignedInt(25)"), 75);
}

#[rstest]
fn unsigned_short_plus_unsigned_short() {
    assert_eq!(eval_i64("xs:unsignedShort(100) + xs:unsignedShort(55)"), 155);
}

#[rstest]
fn unsigned_byte_plus_unsigned_byte() {
    assert_eq!(eval_i64("xs:unsignedByte(10) + xs:unsignedByte(20)"), 30);
}

#[rstest]
fn non_negative_integer_plus_positive_integer() {
    assert_eq!(
        eval_i64("xs:nonNegativeInteger(3) + xs:positiveInteger(7)"),
        10
    );
}

#[rstest]
fn non_positive_integer_plus_negative_integer() {
    assert_eq!(
        eval_i64("xs:nonPositiveInteger(-5) + xs:negativeInteger(-3)"),
        -8
    );
}

// --- Cross-subtype arithmetic ---

#[rstest]
fn long_plus_short() {
    assert_eq!(eval_i64("xs:long(1000) + xs:short(5)"), 1005);
}

#[rstest]
fn byte_times_unsigned_int() {
    assert_eq!(eval_i64("xs:byte(10) * xs:unsignedInt(5)"), 50);
}

#[rstest]
fn integer_plus_long() {
    assert_eq!(eval_i64("xs:integer(100) + xs:long(200)"), 300);
}

#[rstest]
fn short_minus_byte() {
    assert_eq!(eval_i64("xs:short(30) - xs:byte(5)"), 25);
}

#[rstest]
fn unsigned_short_times_unsigned_byte() {
    assert_eq!(eval_i64("xs:unsignedShort(100) * xs:unsignedByte(3)"), 300);
}

// --- Subtype with decimal promotion ---

#[rstest]
fn long_plus_decimal_promotes() {
    let a = eval_atomic("xs:long(5) + xs:decimal('2.5')");
    assert!(matches!(a, A::Decimal(_)), "expected Decimal, got {a:?}");
}

#[rstest]
fn short_plus_float_promotes() {
    let a = eval_atomic("xs:short(3) + xs:float('1.5')");
    assert!(matches!(a, A::Float(_)), "expected Float, got {a:?}");
}

#[rstest]
fn byte_plus_double_promotes() {
    let a = eval_atomic("xs:byte(2) + xs:double('3.0')");
    assert!(matches!(a, A::Double(_)), "expected Double, got {a:?}");
}

#[rstest]
fn unsigned_byte_div_integer() {
    // Division of two integer subtypes should produce Decimal per XPath 2.0 spec
    let a = eval_atomic("xs:unsignedByte(10) div xs:integer(4)");
    assert!(matches!(a, A::Decimal(_)), "expected Decimal, got {a:?}");
}

// --- idiv and mod with subtypes ---

#[rstest]
fn long_idiv_short() {
    assert_eq!(eval_i64("xs:long(17) idiv xs:short(5)"), 3);
}

#[rstest]
fn unsigned_int_mod_byte() {
    assert_eq!(eval_i64("xs:unsignedInt(17) mod xs:byte(5)"), 2);
}

// --- Unary minus on subtypes ---

#[rstest]
fn negate_long() {
    assert_eq!(eval_i64("-xs:long(42)"), -42);
}

#[rstest]
fn negate_unsigned_byte() {
    assert_eq!(eval_i64("-xs:unsignedByte(10)"), -10);
}

// --- Comparisons with subtypes ---

#[rstest]
fn long_eq_short() {
    let c = ctx();
    let seq = evaluate_expr::<N>("xs:long(5) = xs:short(5)", &c).unwrap();
    assert!(matches!(seq.first(), Some(I::Atomic(A::Boolean(true)))));
}

#[rstest]
fn unsigned_int_lt_byte() {
    let c = ctx();
    let seq = evaluate_expr::<N>("xs:unsignedInt(3) < xs:byte(10)", &c).unwrap();
    assert!(matches!(seq.first(), Some(I::Atomic(A::Boolean(true)))));
}

#[rstest]
fn negative_integer_le_non_positive_integer() {
    let c = ctx();
    let seq = evaluate_expr::<N>("xs:negativeInteger(-5) <= xs:nonPositiveInteger(0)", &c).unwrap();
    assert!(matches!(seq.first(), Some(I::Atomic(A::Boolean(true)))));
}
