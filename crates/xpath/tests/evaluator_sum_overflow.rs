// Tests for sum() overflow → FOAR0002 error.
// Before the fix, `sum_default` silently truncated i128→i64 with `as i64`.
// Now it uses `try_into()` and raises FOAR0002 on overflow.

use platynui_xpath::{
    SimpleNode, evaluate_expr,
    runtime::{DynamicContext, ErrorCode},
    xdm::{XdmAtomicValue as A, XdmItem as I},
};
use rstest::rstest;

type N = SimpleNode;

fn ctx() -> DynamicContext<N> {
    DynamicContext::default()
}

#[rstest]
fn sum_large_integers_overflows_foar0002() {
    let c = ctx();
    // Create a sequence of values whose sum exceeds i64::MAX.
    // i64::MAX = 9_223_372_036_854_775_807
    // Two values of ~5e18 each → sum ~1e19 > i64::MAX
    let expr = format!("fn:sum(({0}, {0}))", i64::MAX / 2 + 1);
    let err = evaluate_expr::<N>(&expr, &c).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::FOAR0002, "expected FOAR0002 for integer overflow in sum, got: {err}");
}

#[rstest]
fn sum_large_negative_integers_overflows_foar0002() {
    let c = ctx();
    // Sum of very negative values exceeding i64::MIN
    let expr = format!("fn:sum(({0}, {0}))", i64::MIN / 2 - 1);
    let err = evaluate_expr::<N>(&expr, &c).unwrap_err();
    assert_eq!(
        err.code_enum(),
        ErrorCode::FOAR0002,
        "expected FOAR0002 for negative integer overflow in sum, got: {err}"
    );
}

#[rstest]
fn sum_within_range_succeeds() {
    let c = ctx();
    // Sum that stays within i64 range should work fine
    let expr = "fn:sum((1000000, 2000000, 3000000))";
    let seq = evaluate_expr::<N>(expr, &c).unwrap();
    match seq.first() {
        Some(I::Atomic(A::Integer(v))) => assert_eq!(*v, 6_000_000),
        other => panic!("expected Integer(6000000), got {other:?}"),
    }
}

#[rstest]
fn sum_max_boundary_succeeds() {
    let c = ctx();
    // Exactly i64::MAX should be representable
    let expr = format!("fn:sum(({}, 0))", i64::MAX);
    let seq = evaluate_expr::<N>(&expr, &c).unwrap();
    match seq.first() {
        Some(I::Atomic(A::Integer(v))) => assert_eq!(*v, i64::MAX),
        other => panic!("expected Integer({}), got {other:?}", i64::MAX),
    }
}

#[rstest]
fn sum_subtype_integers_succeeds() {
    let c = ctx();
    // Sum of integer subtypes should work
    let expr = "fn:sum((xs:long(10), xs:short(20), xs:byte(30)))";
    let seq = evaluate_expr::<N>(expr, &c).unwrap();
    match seq.first() {
        Some(I::Atomic(a)) => {
            let v = a.as_i128().expect("integer");
            assert_eq!(v, 60);
        }
        other => panic!("expected integer 60, got {other:?}"),
    }
}
