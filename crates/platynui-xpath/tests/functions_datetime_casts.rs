use platynui_xpath::compile_xpath;
use platynui_xpath::model::{NodeKind, QName, XdmNode};
use platynui_xpath::runtime::{Error, StaticContext};
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use rstest::rstest;

#[derive(Debug, Clone, PartialEq, Eq)]
struct DummyNode;
impl XdmNode for DummyNode {
    fn kind(&self) -> NodeKind { NodeKind::Text }
    fn name(&self) -> Option<QName> { None }
    fn string_value(&self) -> String { String::new() }
    fn parent(&self) -> Option<Self> { None }
    fn children(&self) -> Vec<Self> { vec![] }
    fn attributes(&self) -> Vec<Self> { vec![] }
}

fn as_string<N>(items: &Vec<XdmItem<N>>) -> String {
    match &items[0] { XdmItem::Atomic(XdmAtomicValue::String(s)) => s.clone(), _ => panic!("string expected") }
}

#[rstest]
fn cast_datetime_without_timezone_errors() {
    let sc = StaticContext::default();
    let exec = compile_xpath("'2024-01-01T00:00:00' cast as xs:dateTime", &sc).unwrap();
    let res: Result<Vec<XdmItem<DummyNode>>, Error> = exec.evaluate(&Default::default());
    assert!(res.is_err());
    let e = res.err().unwrap();
    assert_eq!(e.code, "err:FORG0001");
}

#[rstest]
fn cast_date_without_timezone_ok() {
    let sc = StaticContext::default();
    let exec = compile_xpath("string('2024-01-01' cast as xs:date)", &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&Default::default()).unwrap();
    assert_eq!(as_string(&out), "2024-01-01");
}

#[rstest]
fn cast_daytimeduration_full_lexical() {
    let sc = StaticContext::default();
    let exec = compile_xpath("string('P1DT2H30M15S' cast as xs:dayTimeDuration)", &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&Default::default()).unwrap();
    assert_eq!(as_string(&out), "P1DT2H30M15S");
}
