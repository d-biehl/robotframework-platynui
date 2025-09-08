use platynui_xpath::compile_xpath;
use platynui_xpath::model::{NodeKind, QName, XdmNode};
use platynui_xpath::runtime::StaticContext;
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use rstest::rstest;

#[derive(Debug, Clone, PartialEq, Eq)]
struct DummyNode;

impl XdmNode for DummyNode {
    fn kind(&self) -> NodeKind {
        NodeKind::Text
    }
    fn name(&self) -> Option<QName> {
        None
    }
    fn string_value(&self) -> String {
        String::new()
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
    fn compare_document_order(&self, _other: &Self) -> Result<std::cmp::Ordering, platynui_xpath::runtime::Error> {
        Ok(std::cmp::Ordering::Equal)
    }
}

fn atomics<N>(items: &Vec<XdmItem<N>>) -> Vec<XdmAtomicValue> {
    items
        .iter()
        .filter_map(|it| {
            if let XdmItem::Atomic(a) = it {
                Some(a.clone())
            } else {
                None
            }
        })
        .collect()
}

#[rstest]
#[case::upward("1 to 3", vec![XdmAtomicValue::Integer(1), XdmAtomicValue::Integer(2), XdmAtomicValue::Integer(3)])]
#[case::downward_empty("3 to 1", vec![])]
fn range_to_operator(#[case] expr: &str, #[case] expected: Vec<XdmAtomicValue>) {
    let exec = compile_xpath(expr, &StaticContext::default()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec
        .evaluate(&platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build())
        .unwrap();
    let vals = atomics(&out);
    assert_eq!(vals, expected);
}
