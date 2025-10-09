use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder};
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmAtomicValue, xdm::XdmItem};
use rstest::{fixture, rstest};

type N = platynui_xpath::model::simple::SimpleNode;

#[fixture]
fn ctx() -> DynamicContext<N> {
    DynamicContextBuilder::<N>::default().build()
}

#[rstest]
#[case("for $x in (1,2), $y in (10,20) return $x + $y", vec![11, 21, 12, 22])]
#[case("for $y in (10,20), $x in (1,2) return $x + $y", vec![11, 12, 21, 22])]
#[case("for $x in (), $y in (10,20) return $x + $y", vec![])]
#[case("for $x in (1,2), $y in () return $x + $y", vec![])]
fn for_bindings_cartesian_and_order(ctx: DynamicContext<N>, #[case] expr: &str, #[case] expected: Vec<i64>) {
    let out = evaluate_expr::<N>(expr, &ctx).unwrap();
    let nums: Vec<i64> = out
        .into_iter()
        .map(|it| match it {
            XdmItem::Atomic(XdmAtomicValue::Integer(i)) => i,
            _ => panic!("int"),
        })
        .collect();
    assert_eq!(nums, expected);
}
