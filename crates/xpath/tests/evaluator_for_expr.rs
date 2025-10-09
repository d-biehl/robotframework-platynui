use platynui_xpath::engine::runtime::DynamicContext;
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmAtomicValue as A, xdm::XdmItem as I};
use rstest::rstest;

fn ctx() -> DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContext::default()
}

#[rstest]
fn for_simple_numeric() {
    let out =
        evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("for $x in (1,2,3) return $x + 1", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Integer(2)), I::Atomic(A::Integer(3)), I::Atomic(A::Integer(4))]);
}

#[rstest]
fn for_empty_input() {
    let out = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("for $x in () return $x", &ctx()).unwrap();
    assert!(out.is_empty());
}

#[rstest]
fn for_position_last() {
    let out = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("for $x in (10,20) return position()", &ctx())
        .unwrap();
    assert_eq!(out, vec![I::Atomic(A::Integer(1)), I::Atomic(A::Integer(2))]);
    let out =
        evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("for $x in (10,20) return last()", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Integer(2)), I::Atomic(A::Integer(2))]);
}

#[rstest]
fn for_nested() {
    let out = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(
        "for $x in (1,2) return for $y in (3,4) return $x + $y",
        &ctx(),
    )
    .unwrap();
    assert_eq!(
        out,
        vec![I::Atomic(A::Integer(4)), I::Atomic(A::Integer(5)), I::Atomic(A::Integer(5)), I::Atomic(A::Integer(6))]
    );
}
