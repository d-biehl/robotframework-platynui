use platynui_xpath::compile_xpath;
use platynui_xpath::model::{NodeKind, QName, XdmNode};
use platynui_xpath::runtime::{Error, StaticContext};
use platynui_xpath::xdm::{XdmItem};
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

#[rstest]
fn ebv_not_over_date_errors() {
    let sc = StaticContext::default();
    let exec = compile_xpath("not('2024-01-01' cast as xs:date)", &sc).unwrap();
    let res: Result<Vec<XdmItem<DummyNode>>, Error> = exec.evaluate(&Default::default());
    assert!(res.is_err());
    assert_eq!(res.err().unwrap().code, "err:FORG0006");
}
