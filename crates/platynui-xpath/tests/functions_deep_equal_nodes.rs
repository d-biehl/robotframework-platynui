use platynui_xpath::compile_xpath;
use platynui_xpath::model::{NodeKind, QName, XdmNode};
use platynui_xpath::runtime::StaticContext;
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use rstest::rstest;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Node {
    value: String,
}
impl XdmNode for Node {
    fn kind(&self) -> NodeKind {
        NodeKind::Text
    }
    fn name(&self) -> Option<QName> {
        None
    }
    fn string_value(&self) -> String {
        self.value.clone()
    }
    fn parent(&self) -> Option<Self> {
        None
    }
    fn children(&self) -> Vec<Self> {
        vec![]
    }
    fn attributes(&self) -> Vec<Self> {
        vec![]
    }
}

fn as_bool<N>(items: &Vec<XdmItem<N>>) -> bool {
    match &items[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => *b,
        _ => panic!("bool expected"),
    }
}

#[rstest]
fn deep_equal_over_nodes_uses_collation() {
    let sc = StaticContext::default();
    // Build a sequence of two nodes as context variables and compare via deep-equal
    let ex = compile_xpath(
        "deep-equal($a, $b, 'urn:platynui:collation:simple-case-accent')",
        &sc,
    )
    .unwrap();
    let a = vec![
        XdmItem::Node(Node {
            value: "CAFÉ".into(),
        }),
        XdmItem::Node(Node { value: "Ä".into() }),
    ];
    let b = vec![
        XdmItem::Node(Node {
            value: "café".into(),
        }),
        XdmItem::Node(Node { value: "ä".into() }),
    ];
    let ctx = platynui_xpath::runtime::DynamicContextBuilder::new()
        .with_variable(platynui_xpath::xdm::ExpandedName::new(None, "a"), a)
        .with_variable(platynui_xpath::xdm::ExpandedName::new(None, "b"), b)
        .build();
    let out: Vec<XdmItem<Node>> = ex.evaluate(&ctx).unwrap();
    assert!(as_bool(&out));
}

#[rstest]
fn deep_equal_mismatch_lengths_false() {
    let sc = StaticContext::default();
    let ex = compile_xpath("deep-equal($a, $b)", &sc).unwrap();
    let a = vec![XdmItem::Node(Node { value: "x".into() })];
    let b = vec![
        XdmItem::Node(Node { value: "x".into() }),
        XdmItem::Node(Node { value: "y".into() }),
    ];
    let ctx = platynui_xpath::runtime::DynamicContextBuilder::new()
        .with_variable(platynui_xpath::xdm::ExpandedName::new(None, "a"), a)
        .with_variable(platynui_xpath::xdm::ExpandedName::new(None, "b"), b)
        .build();
    let out: Vec<XdmItem<Node>> = ex.evaluate(&ctx).unwrap();
    assert!(!as_bool(&out));
}
