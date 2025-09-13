use platynui_xpath::evaluator::evaluate_expr;
use platynui_xpath::runtime::DynamicContextBuilder;
use platynui_xpath::simple_node::SimpleNode;
use rstest::rstest;

fn ctx() -> platynui_xpath::runtime::DynamicContext<SimpleNode> {
    DynamicContextBuilder::new().build()
}

// Helper: extract atomic debug tail for numeric variant discrimination

#[rstest]
fn sum_empty_returns_integer_zero() {
    let c = ctx();
    let r = evaluate_expr::<SimpleNode>("fn:sum(())", &c).unwrap();
    assert_eq!(r[0].to_string(), "Integer(0)");
}

#[rstest]
fn sum_integers_stays_integer() {
    let c = ctx();
    let r = evaluate_expr::<SimpleNode>("fn:sum((1,2,3))", &c).unwrap();
    assert_eq!(r[0].to_string(), "Integer(6)");
}

#[rstest]
fn sum_integer_decimal_promotes_decimal() {
    let c = ctx();
    let r = evaluate_expr::<SimpleNode>("fn:sum((1, xs:decimal('2.5')))", &c).unwrap();
    assert!(r[0].to_string().starts_with("Decimal("));
}

#[rstest]
fn sum_with_float_promotes_float() {
    let c = ctx();
    let r = evaluate_expr::<SimpleNode>("fn:sum((1, xs:float('2')))", &c).unwrap();
    assert!(r[0].to_string().starts_with("Float("));
}

#[rstest]
fn sum_with_double_promotes_double() {
    let c = ctx();
    let r = evaluate_expr::<SimpleNode>("fn:sum((1, xs:double('2')))", &c).unwrap();
    assert!(r[0].to_string().starts_with("Double("));
}

#[rstest]
fn sum_seed_used_when_empty() {
    let c = ctx();
    let r = evaluate_expr::<SimpleNode>("fn:sum((), 42)", &c).unwrap();
    assert_eq!(r[0].to_string(), "Integer(42)");
}

#[rstest]
fn avg_integer_sequence_decimal_result() {
    let c = ctx();
    let r = evaluate_expr::<SimpleNode>("fn:avg((1,2,3,4))", &c).unwrap();
    assert!(r[0].to_string().starts_with("Decimal("));
}

#[rstest]
fn avg_float_sequence_float_result() {
    let c = ctx();
    let r = evaluate_expr::<SimpleNode>("fn:avg((xs:float('1'), xs:float('3')))", &c).unwrap();
    assert!(r[0].to_string().starts_with("Float("));
}

#[rstest]
fn min_numeric_preserves_kind_integer() {
    let c = ctx();
    let r = evaluate_expr::<SimpleNode>("fn:min((3,1,2))", &c).unwrap();
    assert_eq!(r[0].to_string(), "Integer(1)");
}

#[rstest]
fn max_numeric_promotes_decimal() {
    let c = ctx();
    let r = evaluate_expr::<SimpleNode>("fn:max((1, xs:decimal('2.1')))", &c).unwrap();
    assert!(r[0].to_string().starts_with("Decimal("));
}

#[rstest]
fn min_with_float_promotes_float() {
    let c = ctx();
    let r = evaluate_expr::<SimpleNode>("fn:min((xs:float('2'), 1))", &c).unwrap();
    assert!(r[0].to_string().starts_with("Float("));
}

#[rstest]
fn max_with_double_promotes_double() {
    let c = ctx();
    let r = evaluate_expr::<SimpleNode>("fn:max((xs:double('2'), 1))", &c).unwrap();
    assert!(r[0].to_string().starts_with("Double("));
}
