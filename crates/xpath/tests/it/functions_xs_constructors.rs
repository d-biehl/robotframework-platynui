use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder};
use platynui_xpath::runtime::ErrorCode;
use platynui_xpath::{
    engine::evaluator::evaluate_expr, model::simple::SimpleNode, xdm::XdmAtomicValue as A, xdm::XdmItem as I,
};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;

fn empty_ctx() -> DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

#[rstest]
fn xs_string_basic_and_empty() {
    let out = evaluate_expr::<N>("xs:string('abc')", &empty_ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::String("abc".into()))]);

    let out = evaluate_expr::<N>("xs:string(())", &empty_ctx()).unwrap();
    assert!(out.is_empty());
}

#[rstest]
fn xs_string_on_node_atomizes() {
    // Context item is a text node with value "Hello"
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(SimpleNode::text("Hello"))).build();
    let out = evaluate_expr::<N>("xs:string(.)", &ctx).unwrap();
    assert_eq!(out, vec![I::Atomic(A::String("Hello".into()))]);
}

#[rstest]
fn xs_boolean_lexical_forms_and_empty() {
    let c = empty_ctx();
    let t1 = evaluate_expr::<N>("xs:boolean('true')", &c).unwrap();
    assert_eq!(t1, vec![I::Atomic(A::Boolean(true))]);
    let t2 = evaluate_expr::<N>("xs:boolean('1')", &c).unwrap();
    assert_eq!(t2, vec![I::Atomic(A::Boolean(true))]);

    let f1 = evaluate_expr::<N>("xs:boolean('false')", &c).unwrap();
    assert_eq!(f1, vec![I::Atomic(A::Boolean(false))]);
    let f2 = evaluate_expr::<N>("xs:boolean('0')", &c).unwrap();
    assert_eq!(f2, vec![I::Atomic(A::Boolean(false))]);

    // empty sequence yields empty
    let e = evaluate_expr::<N>("xs:boolean(())", &c).unwrap();
    assert!(e.is_empty());
}

#[rstest]
fn xs_boolean_invalid_raises_forg0001() {
    let err = evaluate_expr::<N>("xs:boolean('yes')", &empty_ctx()).expect_err("expected error");
    assert!(err.code_enum() == ErrorCode::FORG0001, "unexpected error code: {:?}", err.code_qname());
}

#[rstest]
fn xs_integer_valid_and_empty() {
    let c = empty_ctx();
    let a = evaluate_expr::<N>("xs:integer('42')", &c).unwrap();
    assert_eq!(a, vec![I::Atomic(A::Integer(42))]);
    let b = evaluate_expr::<N>("xs:integer('-3')", &c).unwrap();
    assert_eq!(b, vec![I::Atomic(A::Integer(-3))]);
    // leading plus and whitespace should be tolerated per minimal impl (trim + parse)
    let d = evaluate_expr::<N>("xs:integer('  +7  ')", &c).unwrap();
    assert_eq!(d, vec![I::Atomic(A::Integer(7))]);

    // empty sequence -> empty
    let e = evaluate_expr::<N>("xs:integer(())", &c).unwrap();
    assert!(e.is_empty());
}

#[rstest]
fn xs_integer_on_node_atomizes() {
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(SimpleNode::text("7"))).build();
    let out = evaluate_expr::<N>("xs:integer(.)", &ctx).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Integer(7))]);
}

#[rstest]
fn xs_integer_invalid_raises_forg0001() {
    // Fractional numeric literal → FOCA0001 (fractional part) per updated constructor logic.
    let err_frac = evaluate_expr::<N>("xs:integer('3.14')", &empty_ctx()).expect_err("expected error");
    assert!(err_frac.code_enum() == ErrorCode::FOCA0001, "3.14 expected FOCA0001, got {:?}", err_frac.code_qname());
    // Non-numeric lexical → FORG0001
    let err_lex = evaluate_expr::<N>("xs:integer('abc')", &empty_ctx()).expect_err("expected error");
    assert!(err_lex.code_enum() == ErrorCode::FORG0001, "abc expected FORG0001, got {:?}", err_lex.code_qname());
}
