use platynui_xpath::engine::evaluator::evaluate_expr; // if public helper not exposed, adjust to compile+evaluate
use platynui_xpath::engine::runtime::DynamicContext;
use platynui_xpath::model::XdmNode;
use rstest::rstest; // placeholder trait

// Provide a minimal dummy node implementation for atomic-only tests (if real test infra already has one, this can be replaced).
#[derive(Clone, Debug, PartialEq, Eq)]
struct DummyNode;
impl XdmNode for DummyNode {
    type Children<'a> = std::iter::Empty<Self> where Self: 'a;
    type Attributes<'a> = std::iter::Empty<Self> where Self: 'a;
    type Namespaces<'a> = std::iter::Empty<Self> where Self: 'a;

    fn kind(&self) -> platynui_xpath::model::NodeKind {
        platynui_xpath::model::NodeKind::Document
    }
    fn string_value(&self) -> String {
        String::new()
    }
    fn parent(&self) -> Option<Self> {
        None
    }
    fn children(&self) -> Self::Children<'_> {
        std::iter::empty()
    }
    fn attributes(&self) -> Self::Attributes<'_> {
        std::iter::empty()
    }
    fn namespaces(&self) -> Self::Namespaces<'_> {
        std::iter::empty()
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

fn eval(expr: &str) -> bool {
    let ctx: DynamicContext<DummyNode> = DynamicContext::default();
    let seq = evaluate_expr(expr, &ctx).expect("evaluation");
    match seq.first() {
        Some(platynui_xpath::xdm::XdmItem::Atomic(
            platynui_xpath::xdm::XdmAtomicValue::Boolean(b),
        )) => *b,
        _ => panic!("expected boolean result"),
    }
}

#[rstest]
fn int_vs_decimal_eq() {
    assert!(eval("1 eq 1.0"));
}
#[rstest]
fn int_vs_float_lt() {
    assert!(eval("1 lt 1.5e0"));
}
#[rstest]
fn decimal_vs_float_gt() {
    assert!(!eval("1.25 gt 1.3e0"));
}
#[rstest]
fn float_vs_double_ne() {
    assert!(eval("1.0e0 ne 2.0E0"));
}
#[rstest]
fn chain_mixed_le() {
    assert!(eval("1 le 1.0"));
}
