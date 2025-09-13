use platynui_xpath::evaluator::evaluate_expr;
use platynui_xpath::runtime::DynamicContext;
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
    ) -> Result<std::cmp::Ordering, platynui_xpath::runtime::Error> {
        Ok(std::cmp::Ordering::Equal)
    }
    fn name(&self) -> Option<platynui_xpath::QName> {
        None
    }
}

fn eval_bool(expr: &str) -> bool {
    let ctx: DynamicContext<DummyNode> = DynamicContext::default();
    let seq = evaluate_expr(expr, &ctx).expect("eval");
    match seq.get(0) {
        Some(platynui_xpath::xdm::XdmItem::Atomic(
            platynui_xpath::xdm::XdmAtomicValue::Boolean(b),
        )) => *b,
        _ => false,
    }
}

#[rstest]
fn incomparable_boolean_general_eq_false() {
    // General comparison: boolean vs numeric values are incomparable; overall result false.
    // Using sequence to force general comparison semantics.
    assert!(!eval_bool("(true(), false()) = (1, 2)"));
}

#[rstest]
fn incomparable_string_vs_boolean_eq_false() {
    assert!(!eval_bool("'a' = true()"));
}

#[rstest]
fn incomparable_numeric_vs_string_lt_false() {
    assert!(!eval_bool("1 < 'x'"));
}
