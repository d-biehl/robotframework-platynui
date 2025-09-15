use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder};
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmItem};
use rstest::{fixture, rstest};

type N = platynui_xpath::model::simple::SimpleNode;

#[fixture]
fn ctx() -> DynamicContext<N> {
    DynamicContextBuilder::<N>::default().build()
}

#[rstest]
#[case("some $x in (1,2), $y in (3) satisfies $x + $y = 5", true)]
#[case("every $x in (1,2), $y in (3) satisfies $x lt 3", true)]
fn quantifiers_two_bindings(#[case] expr: &str, #[case] expected: bool, ctx: DynamicContext<N>) {
    let out = evaluate_expr::<N>(expr, &ctx).unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => {
            assert_eq!(*b, expected)
        }
        _ => panic!("expected boolean"),
    }
}
