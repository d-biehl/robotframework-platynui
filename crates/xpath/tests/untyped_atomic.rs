use platynui_xpath::{
    evaluate_expr,
    runtime::DynamicContext,
    xdm::{XdmAtomicValue, XdmItem},
};
use rstest::rstest;

#[derive(Clone, Debug, PartialEq, Eq)]
struct DummyNode {
    val: String,
}
impl platynui_xpath::model::XdmNode for DummyNode {
    type Children<'a>
        = std::iter::Empty<Self>
    where
        Self: 'a;
    type Attributes<'a>
        = std::iter::Empty<Self>
    where
        Self: 'a;
    type Namespaces<'a>
        = std::iter::Empty<Self>
    where
        Self: 'a;

    fn kind(&self) -> platynui_xpath::model::NodeKind {
        platynui_xpath::model::NodeKind::Element
    }
    fn typed_value(&self) -> Vec<XdmAtomicValue> {
        vec![XdmAtomicValue::UntypedAtomic(self.val.clone())]
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

fn eval_with(val: &str, expr: &str) -> Result<Vec<XdmItem<DummyNode>>, platynui_xpath::engine::runtime::Error> {
    let ctx: DynamicContext<DummyNode> = DynamicContext::<DummyNode> {
        context_item: Some(XdmItem::Node(DummyNode { val: val.to_string() })),
        ..Default::default()
    };
    evaluate_expr(expr, &ctx)
}

fn eval_atomic(val: &str, expr: &str) -> Result<XdmAtomicValue, platynui_xpath::engine::runtime::Error> {
    let seq = eval_with(val, expr)?;
    Ok(match seq.first() {
        Some(XdmItem::Atomic(a)) => a.clone(),
        _ => panic!("expected atomic"),
    })
}

#[rstest]
fn arithmetic_untyped_numeric() {
    match eval_atomic("42", ". + 1") {
        Ok(XdmAtomicValue::Double(d)) if (d - 43.0).abs() < 1e-9 => {}
        Ok(XdmAtomicValue::Decimal(d)) if (d - 43.0).abs() < 1e-9 => {}
        Ok(a) => panic!("wrong result {a:?}"),
        Err(e) => panic!("err {e:?}"),
    }
}

#[rstest]
fn arithmetic_untyped_invalid_error() {
    let err = eval_with("xyz", ". + 1").unwrap_err();
    // runtime::Error stores code as 'err:CODE'; compare via enum
    assert_eq!(err.code_enum(), platynui_xpath::engine::runtime::ErrorCode::FORG0001);
}

#[rstest]
fn comparison_untyped_numeric_eq() {
    let b = eval_with("10", ". = 10").unwrap();
    match b[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(true)) => {}
        _ => panic!("expected true boolean"),
    }
}

#[rstest]
fn comparison_untyped_vs_string() {
    // untyped vs string -> string comparison
    let r = eval_with("abc", ". = 'abc'").unwrap();
    match r[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(true)) => {}
        _ => panic!("expected true"),
    }
}

#[rstest]
fn comparison_untyped_invalid_numeric_eq_false() {
    // invalid numeric text compared to number should be false (after failed cast triggers FORG0001? We'll decide spec mapping soon)
    // placeholder expectation: current implementation may error; adjusted after logic change
    let res = eval_with("abc", ". = 5");
    assert!(res.is_err(), "expecting error until implemented");
}
