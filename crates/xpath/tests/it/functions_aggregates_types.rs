use platynui_xpath::engine::evaluator::evaluate_expr;
use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::xdm::{XdmAtomicValue as A, XdmItem as I};
use rstest::rstest;

fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContextBuilder::new().build()
}

// Helper: extract atomic debug tail for numeric variant discrimination

#[rstest]
fn sum_empty_returns_integer_zero() {
    let c = ctx();
    let r = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("fn:sum(())", &c).unwrap();
    match &r[0] {
        I::Atomic(A::Integer(v)) => assert_eq!(*v, 0),
        other => panic!("expected Integer(0), got {other:?}"),
    }
}

#[rstest]
fn sum_integers_stays_integer() {
    let c = ctx();
    let r = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("fn:sum((1,2,3))", &c).unwrap();
    match &r[0] {
        I::Atomic(A::Integer(v)) => assert_eq!(*v, 6),
        other => panic!("expected Integer(6), got {other:?}"),
    }
}

#[rstest]
fn sum_integer_decimal_promotes_decimal() {
    let c = ctx();
    let r = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("fn:sum((1, xs:decimal('2.5')))", &c).unwrap();
    assert!(matches!(r[0], I::Atomic(A::Decimal(_))));
}

#[rstest]
fn sum_with_float_promotes_float() {
    let c = ctx();
    let r = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("fn:sum((1, xs:float('2')))", &c).unwrap();
    assert!(matches!(r[0], I::Atomic(A::Float(_))));
}

#[rstest]
fn sum_with_double_promotes_double() {
    let c = ctx();
    let r = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("fn:sum((1, xs:double('2'))) ", &c).unwrap();
    assert!(matches!(r[0], I::Atomic(A::Double(_))));
}

#[rstest]
fn sum_seed_used_when_empty() {
    let c = ctx();
    let r = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("fn:sum((), 42)", &c).unwrap();
    match &r[0] {
        I::Atomic(A::Integer(v)) => assert_eq!(*v, 42),
        other => panic!("expected Integer(42), got {other:?}"),
    }
}

#[rstest]
fn avg_integer_sequence_decimal_result() {
    let c = ctx();
    let r = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("fn:avg((1,2,3,4))", &c).unwrap();
    assert!(matches!(r[0], I::Atomic(A::Decimal(_))));
}

#[rstest]
fn avg_float_sequence_float_result() {
    let c = ctx();
    let r = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("fn:avg((xs:float('1'), xs:float('3'))) ", &c)
        .unwrap();
    assert!(matches!(r[0], I::Atomic(A::Float(_))));
}

#[rstest]
fn min_numeric_preserves_kind_integer() {
    let c = ctx();
    let r = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("fn:min((3,1,2))", &c).unwrap();
    match &r[0] {
        I::Atomic(A::Integer(v)) => assert_eq!(*v, 1),
        other => panic!("expected Integer(1), got {other:?}"),
    }
}

#[rstest]
fn max_numeric_promotes_decimal() {
    let c = ctx();
    let r = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("fn:max((1, xs:decimal('2.1')))", &c).unwrap();
    assert!(matches!(r[0], I::Atomic(A::Decimal(_))));
}

#[rstest]
fn min_with_float_promotes_float() {
    let c = ctx();
    let r = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("fn:min((xs:float('2'), 1))", &c).unwrap();
    assert!(matches!(r[0], I::Atomic(A::Float(_))));
}

#[rstest]
fn max_with_double_promotes_double() {
    let c = ctx();
    let r = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("fn:max((xs:double('2'), 1))", &c).unwrap();
    assert!(matches!(r[0], I::Atomic(A::Double(_))));
}
