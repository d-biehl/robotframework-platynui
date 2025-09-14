use platynui_xpath::functions::deep_equal_with_collation;
use platynui_xpath::simple_node::SimpleNode;
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use proptest::prelude::*;
use rstest::rstest;

// Simple arbitrary atomic subset for now.
prop_compose! {
    fn arb_atomic()(s in "[a-zA-Z0-9]{0,8}", i in any::<i32>(), b in any::<bool>()) -> XdmAtomicValue {
        // Mix selection for variability
        match (i % 5).abs() { // deterministic branching
            0 => XdmAtomicValue::String(s),
            1 => XdmAtomicValue::Integer(i as i64),
            2 => XdmAtomicValue::Double(i as f64 / 3.0),
            3 => XdmAtomicValue::Boolean(b),
            _ => XdmAtomicValue::UntypedAtomic(i.to_string()),
        }
    }
}

prop_compose! {
    fn arb_sequence()(vec in prop::collection::vec(arb_atomic(), 0..6)) -> Vec<XdmItem<SimpleNode>> {
        vec.into_iter().map(XdmItem::Atomic).collect()
    }
}

proptest! {
    #[rstest]
    fn deep_equal_reflexive(seq in arb_sequence()) {
        let lhs = seq.clone();
        let rhs = seq.clone();
        let res = deep_equal_with_collation(&lhs, &rhs, None).unwrap();
        prop_assert!(res, "sequence not reflexive: {:?}", lhs);
    }
}

fn eval_distinct_values(seq: &[XdmItem<SimpleNode>]) -> Vec<XdmItem<SimpleNode>> {
    use platynui_xpath::evaluator::evaluate_expr;
    use platynui_xpath::runtime::DynamicContext;
    use platynui_xpath::xdm::ExpandedName;
    let mut ctx: DynamicContext<SimpleNode> = DynamicContext::default();
    ctx.variables
        .insert(ExpandedName::new(None, "s"), seq.to_vec());
    evaluate_expr("distinct-values($s)", &ctx)
        .expect("evaluation distinct-values")
        .into_iter()
        .collect()
}

proptest! {
    #[rstest]
    fn distinct_values_idempotent(seq in arb_sequence()) {
    let once = eval_distinct_values(&seq);
    let twice = eval_distinct_values(&once);
        prop_assert_eq!(once, twice);
    }
}
