use platynui_xpath::functions::deep_equal_with_collation;
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use rstest::rstest;

// Simple arbitrary atomic subset for now.
#[rstest]
fn deep_equal_reflexive() {
    use XdmAtomicValue as A;
    // Curated representative sequences
    let cases: Vec<Vec<XdmItem<platynui_xpath::model::simple::SimpleNode>>> = vec![
        vec![],
        vec![XdmItem::Atomic(A::Integer(1))],
        vec![XdmItem::Atomic(A::String("a".into())), XdmItem::Atomic(A::String("a".into()))],
        vec![XdmItem::Atomic(A::Double(1.5)), XdmItem::Atomic(A::Double(-0.0))],
        vec![XdmItem::Atomic(A::Boolean(true)), XdmItem::Atomic(A::Boolean(false))],
        vec![XdmItem::Atomic(A::UntypedAtomic("42".into())), XdmItem::Atomic(A::Integer(42))],
    ];
    for seq in cases {
        let lhs = seq.clone();
        let rhs = seq.clone();
        let res = deep_equal_with_collation(&lhs, &rhs, None).unwrap();
        assert!(res, "sequence not reflexive: {:?}", lhs);
    }
}

fn eval_distinct_values(
    seq: &[XdmItem<platynui_xpath::model::simple::SimpleNode>],
) -> Vec<XdmItem<platynui_xpath::model::simple::SimpleNode>> {
    use platynui_xpath::compile_with_context;
    use platynui_xpath::engine::evaluator::evaluate;
    use platynui_xpath::engine::runtime::{DynamicContext, StaticContextBuilder};
    use platynui_xpath::xdm::ExpandedName;
    let mut ctx: DynamicContext<platynui_xpath::model::simple::SimpleNode> = DynamicContext::default();
    ctx.variables.insert(ExpandedName::new(None, "s"), seq.to_vec());
    let static_ctx = StaticContextBuilder::new().with_variable(ExpandedName::new(None, "s")).build();
    let compiled = compile_with_context("distinct-values($s)", &static_ctx).expect("compile distinct-values");
    evaluate(&compiled, &ctx).expect("evaluation distinct-values").into_iter().collect()
}

#[rstest]
fn distinct_values_idempotent() {
    use XdmAtomicValue as A;
    let cases: Vec<Vec<XdmItem<platynui_xpath::model::simple::SimpleNode>>> = vec![
        vec![],
        vec![XdmItem::Atomic(A::Integer(1)), XdmItem::Atomic(A::Integer(1)), XdmItem::Atomic(A::Integer(2))],
        vec![
            XdmItem::Atomic(A::String("a".into())),
            XdmItem::Atomic(A::String("A".into())),
            XdmItem::Atomic(A::String("a".into())),
        ],
        vec![XdmItem::Atomic(A::Double(f64::NAN)), XdmItem::Atomic(A::Double(f64::NAN))],
    ];
    for seq in cases {
        let once = eval_distinct_values(&seq);
        let twice = eval_distinct_values(&once);
        // Use XDM deep-equal semantics instead of Rust's PartialEq to handle NaN correctly
        let eq = deep_equal_with_collation(&once, &twice, None).expect("deep-equal evaluation");
        assert!(eq, "distinct-values not idempotent: once={:?} twice={:?}", once, twice);
    }
}
