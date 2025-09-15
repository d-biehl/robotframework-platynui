use platynui_xpath::{
    evaluate_expr,
    runtime::{DynamicContext, ErrorCode},
    xdm::{XdmAtomicValue, XdmItem},
};
use rstest::rstest;

#[derive(Clone, Debug, PartialEq, Eq)]
struct DummyNode;
impl platynui_xpath::model::XdmNode for DummyNode {
    fn kind(&self) -> platynui_xpath::model::NodeKind {
        platynui_xpath::model::NodeKind::Document
    }
    fn string_value(&self) -> String {
        String::new()
    }
    fn children(&self) -> Vec<Self> {
        vec![]
    }
    fn parent(&self) -> Option<Self> {
        None
    }
    fn attributes(&self) -> Vec<Self> {
        vec![]
    }
    fn compare_document_order(
        &self,
        _other: &Self,
    ) -> Result<std::cmp::Ordering, platynui_xpath::engine::runtime::Error> {
        Ok(std::cmp::Ordering::Equal)
    }
    fn name(&self) -> Option<platynui_xpath::QName> {
        None
    }
}

fn eval_atomic(expr: &str) -> XdmAtomicValue {
    let ctx: DynamicContext<DummyNode> = DynamicContext::default();
    let seq = evaluate_expr(expr, &ctx).expect("eval ok");
    match seq.first() {
        Some(XdmItem::Atomic(a)) => a.clone(),
        _ => panic!("expected atomic"),
    }
}

#[rstest]
fn int_add_overflow_promotes_to_decimal() {
    // Build a very large sum that exceeds i64 range in our integer-exact path → decimal promotion
    // Choose (i64::MAX - 1) + (i64::MAX - 1) which must overflow i64
    let a = eval_atomic(&format!("{} + {}", i64::MAX - 1, i64::MAX - 1));
    match a {
        XdmAtomicValue::Decimal(_) => {}
        other => panic!("expected decimal, got {:?}", other),
    }
}

#[rstest]
fn int_mul_overflow_promotes_to_decimal() {
    // i64::MAX / 2 * 3 overflows i64; should become decimal
    let a = eval_atomic(&format!("{} * 3", i64::MAX / 2));
    match a {
        XdmAtomicValue::Decimal(d) => assert!(d > i64::MAX as f64 / 2.0),
        other => panic!("expected decimal, got {:?}", other),
    }
}

#[rstest]
fn idiv_extreme_overflow_errors_foar0002() {
    let ctx: DynamicContext<DummyNode> = DynamicContext::default();
    // Construct numerator via multiplication: (i64::MAX * 3) idiv 1 → integer path computes product in i128,
    // which cannot fit into i64 for the final xs:integer, triggering FOAR0002.
    let expr = format!("({} * 3) idiv 1", i64::MAX);
    let err = evaluate_expr(&expr, &ctx).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::FOAR0002);
}
