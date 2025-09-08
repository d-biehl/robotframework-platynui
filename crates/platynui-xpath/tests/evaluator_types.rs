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

fn as_bool<N>(items: &Vec<XdmItem<N>>) -> bool {
    match &items[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => *b,
        _ => panic!("bool expected"),
    }
}

fn atoms<N>(items: &Vec<XdmItem<N>>) -> Vec<XdmAtomicValue> {
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
#[rustfmt::skip]
#[case::int("'5' cast as xs:integer", XdmAtomicValue::Integer(5))]
#[case::str("5 cast as xs:string", XdmAtomicValue::String("5".into()))]
#[case::bool("'true' cast as xs:boolean", XdmAtomicValue::Boolean(true))]
#[case::trunc("'2.7' cast as xs:integer", XdmAtomicValue::Integer(2))]
fn cast_simple(#[case] expr: &str, #[case] expected: XdmAtomicValue) {
    let sc = mk_sc();
    let exec = compile_xpath(expr, &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert_eq!(atoms(&out), vec![expected]);
}

#[rstest]
fn castable_and_optional() {
    let sc = mk_sc();
    // simple castable check
    let exec = compile_xpath("1 castable as xs:string?", &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert!(as_bool(&out));
}

#[rstest]
#[rustfmt::skip]
#[case::t1("'a' instance of xs:string", true)]
#[case::t2("1 instance of xs:string", false)]
#[case::t3("1 instance of xs:integer?", true)]
#[case::t4("1 instance of xs:integer+", true)]
#[case::t5("1 instance of xs:integer", true)]
fn instance_of_cases(#[case] expr: &str, #[case] expected: bool) {
    let sc = mk_sc();
    let exec = compile_xpath(expr, &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert_eq!(as_bool(&out), expected);
}

#[rstest]
fn treat_as_item_star() {
    let sc = mk_sc();
    let exec = compile_xpath("1 treat as item()", &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert_eq!(atoms(&out).len(), 1);
}

fn mk_sc() -> StaticContext {
    let mut sc = StaticContext::default();
    sc.namespaces
        .by_prefix
        .insert("xs".into(), "http://www.w3.org/2001/XMLSchema".into());
    sc
}

fn ctx() -> platynui_xpath::runtime::DynamicContext<DummyNode> {
    platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build()
}
