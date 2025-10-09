use platynui_xpath::engine::runtime::DynamicContext;
use platynui_xpath::evaluate_expr;
use platynui_xpath::xdm::XdmAtomicValue as A;
use platynui_xpath::xdm::XdmItem as I;
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;

fn ctx() -> DynamicContext<N> {
    DynamicContext::default()
}

#[rstest]
fn let_single_binding_returns_sequence() {
    let out = evaluate_expr::<N>("let $x := (1,2) return $x", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Integer(1)), I::Atomic(A::Integer(2))]);
}

#[rstest]
fn let_bindings_can_reference_previous() {
    let out = evaluate_expr::<N>("let $x := (1,2), $y := $x[1] return ($x, $y)", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Integer(1)), I::Atomic(A::Integer(2)), I::Atomic(A::Integer(1)),]);
}

#[rstest]
fn nested_let_scopes_restore_outer_binding() {
    let out = evaluate_expr::<N>("let $x := 1 return (let $x := 2 return $x, $x)", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Integer(2)), I::Atomic(A::Integer(1))]);
}

#[rstest]
fn let_binding_expression_can_use_prior_variable() {
    let out = evaluate_expr::<N>("let $x := 1, $y := $x + 1 return $y", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Integer(2))]);
}
