use platynui_xpath::{
    evaluate_expr,
    runtime::DynamicContext,
    xdm::{XdmAtomicValue, XdmItem},
};
use rstest::rstest;

// Property: For any string S that is not a lexical double, number(S) is NaN and hence number(S) = number(S) is false and != is true.
// We restrict S to strings containing at least one alphabetic char to avoid accidental numeric lexicals.
#[rstest]
#[case("a")]
#[case("ABC")]
#[case("hello")]
#[case("zzz")]
#[case("OnlyAlpha")]
#[case("xXx")]
fn number_alpha_nan_equality_behavior(#[case] s: &str) {
    let ctx: DynamicContext<platynui_xpath::model::simple::SimpleNode> = DynamicContext::default();
    let expr_eq = format!("number('{s}') = number('{s}')");
    let expr_ne = format!("number('{s}') != number('{s}')");
    let seq_eq = evaluate_expr(&expr_eq, &ctx).unwrap();
    let seq_ne = evaluate_expr(&expr_ne, &ctx).unwrap();
    match (seq_eq.first(), seq_ne.first()) {
        (Some(XdmItem::Atomic(XdmAtomicValue::Boolean(beq))), Some(XdmItem::Atomic(XdmAtomicValue::Boolean(bne)))) => {
            assert!(!beq && *bne, "expected NaN equality false & inequality true for {s}");
        }
        _ => panic!("expected boolean results"),
    }
}
