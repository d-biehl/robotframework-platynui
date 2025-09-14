use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::{xdm::XdmItem, engine::evaluator::evaluate_expr, xdm::XdmAtomicValue};

type N = platynui_xpath::model::simple::SimpleNode;

#[test]
fn for_two_bindings_cartesian_order() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    // for $x in (1,2), $y in (10,20) return $x + $y
    let expr = "for $x in (1,2), $y in (10,20) return $x + $y";
    let out = evaluate_expr::<N>(expr, &ctx).unwrap();
    let nums: Vec<i64> = out
        .into_iter()
        .map(|it| match it {
            XdmItem::Atomic(XdmAtomicValue::Integer(i)) => i,
            _ => panic!("int"),
        })
        .collect();
    assert_eq!(nums, vec![11, 21, 12, 22]);
}
