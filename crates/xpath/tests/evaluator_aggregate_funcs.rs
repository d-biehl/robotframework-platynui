use platynui_xpath::{
    evaluate_expr,
    runtime::DynamicContext,
    xdm::{XdmAtomicValue, XdmItem},
};
use rstest::rstest;

fn ctx() -> DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContext::default()
}

fn d(expr: &str) -> f64 {
    let seq = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx()).unwrap();
    match seq.first() {
        Some(XdmItem::Atomic(XdmAtomicValue::Double(v))) => *v,
        Some(XdmItem::Atomic(XdmAtomicValue::Float(v))) => *v as f64,
        Some(XdmItem::Atomic(XdmAtomicValue::Decimal(v))) => *v,
        Some(XdmItem::Atomic(XdmAtomicValue::Integer(v))) => *v as f64,
        _ => panic!("expected numeric atomic"),
    }
}

#[rstest]
#[case("sum((1,2,3))", 6.0)]
#[case("sum(())", 0.0)] // default empty
#[case("sum((), 42)", 42.0)] // seed
fn sum_basic_and_empty(#[case] expr: &str, #[case] expected: f64) {
    assert_eq!(d(expr), expected);
}

#[rstest]
#[case("sum((1, untypedAtomic('2'), untypedAtomic('3')))", 6.0)]
fn sum_with_untyped_and_string_numbers(#[case] expr: &str, #[case] expected: f64) {
    assert_eq!(d(expr), expected);
}

#[rstest]
#[case("avg((2,4,6))", Some(4.0))]
#[case("avg(())", None)]
fn avg_basic(#[case] expr: &str, #[case] expected: Option<f64>) {
    let seq = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx()).unwrap();
    match expected {
        Some(val) => {
            let got = match seq.first().expect("value") {
                XdmItem::Atomic(XdmAtomicValue::Double(v)) => *v,
                XdmItem::Atomic(XdmAtomicValue::Float(v)) => *v as f64,
                XdmItem::Atomic(XdmAtomicValue::Decimal(v)) => *v,
                XdmItem::Atomic(XdmAtomicValue::Integer(v)) => *v as f64,
                _ => panic!("expected numeric atomic"),
            };
            assert!((got - val).abs() < 1e-9, "avg mismatch: got {got} expected {val}");
        }
        None => assert!(seq.is_empty()),
    }
}

#[rstest]
#[case("min((3,1,10,2))", 1.0)]
#[case("max((3,1,10,2))", 10.0)]
fn min_max_numeric(#[case] expr: &str, #[case] expected: f64) {
    assert_eq!(d(expr), expected);
}

#[rstest]
#[case("min(('b','aa','c'))", "aa")]
#[case("max(('b','aa','c'))", "c")]
fn min_max_string_collation_fallback(#[case] expr: &str, #[case] expected: &str) {
    // Mixed non-numeric leads to string ordering branch; expect lexicographic min/max
    let seq = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx()).unwrap();
    if let Some(XdmItem::Atomic(XdmAtomicValue::String(s))) = seq.first() {
        assert_eq!(s, expected);
    } else {
        panic!("expected string")
    }
}

#[rstest]
#[case("min((number('abc'), 5))")]
fn min_max_nan_propagation(#[case] expr: &str) {
    let seq = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx()).unwrap();
    if let Some(XdmItem::Atomic(XdmAtomicValue::Double(dv))) = seq.first() {
        assert!(dv.is_nan());
    } else {
        panic!("expected double NaN")
    }
}
