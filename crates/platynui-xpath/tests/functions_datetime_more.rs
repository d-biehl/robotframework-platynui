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
}

#[rstest]
fn timezone_components_for_date_and_time() {
    let sc = StaticContext::default();

    // timezone-from-date returns empty if no tz, otherwise duration
    let ex1 = compile_xpath("timezone-from-date('2024-01-02' cast as xs:date)", &sc).unwrap();
    let out1: Vec<XdmItem<DummyNode>> = ex1.evaluate(&Default::default()).unwrap();
    assert!(out1.is_empty());
    let ex2 = compile_xpath(
        "timezone-from-date('2024-01-02+02:30' cast as xs:date)",
        &sc,
    )
    .unwrap();
    let out2: Vec<XdmItem<DummyNode>> = ex2.evaluate(&Default::default()).unwrap();
    match &out2[0] {
        XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => {
            assert_eq!(*secs, 2 * 3600 + 30 * 60)
        }
        _ => panic!("dtd"),
    }

    // timezone-from-time returns empty if no tz, otherwise duration
    let ex3 = compile_xpath("timezone-from-time('03:04:05' cast as xs:time)", &sc).unwrap();
    let out3: Vec<XdmItem<DummyNode>> = ex3.evaluate(&Default::default()).unwrap();
    assert!(out3.is_empty());
    let ex4 = compile_xpath("timezone-from-time('03:04:05+02:30' cast as xs:time)", &sc).unwrap();
    let out4: Vec<XdmItem<DummyNode>> = ex4.evaluate(&Default::default()).unwrap();
    match &out4[0] {
        XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => {
            assert_eq!(*secs, 2 * 3600 + 30 * 60)
        }
        _ => panic!("dtd"),
    }
}
