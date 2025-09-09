use platynui_xpath::compile_xpath;
use platynui_xpath::model::{NodeKind, QName, XdmNode};
use platynui_xpath::runtime::{Error, StaticContext};
use platynui_xpath::xdm::XdmItem;
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
fn sum_with_non_numeric_errors() {
    let sc = StaticContext::default();
    let ex = compile_xpath("sum((1,'Zero'))", &sc).unwrap();
    let res: Result<Vec<XdmItem<DummyNode>>, Error> = ex.evaluate(&Default::default());
    assert!(res.is_err());
    assert_eq!(res.err().unwrap().code, "err:FORG0001");
}

#[rstest]
fn avg_with_non_numeric_errors() {
    let sc = StaticContext::default();
    let ex = compile_xpath("avg((1,'Zero'))", &sc).unwrap();
    let res: Result<Vec<XdmItem<DummyNode>>, Error> = ex.evaluate(&Default::default());
    assert!(res.is_err());
    assert_eq!(res.err().unwrap().code, "err:FORG0001");
}

#[rstest]
fn min_mixed_numeric_and_string_errors() {
    let sc = StaticContext::default();
    let ex = compile_xpath("min((1 to 2, 'Zero'))", &sc).unwrap();
    let res: Result<Vec<XdmItem<DummyNode>>, Error> = ex.evaluate(&Default::default());
    assert!(res.is_err());
    assert_eq!(res.err().unwrap().code, "err:FORG0006");
}

#[rstest]
fn max_mixed_string_and_numeric_errors() {
    let sc = StaticContext::default();
    let ex = compile_xpath("max(('a', (2 to 2)))", &sc).unwrap();
    let res: Result<Vec<XdmItem<DummyNode>>, Error> = ex.evaluate(&Default::default());
    assert!(res.is_err());
    assert_eq!(res.err().unwrap().code, "err:FORG0006");
}

#[rstest]
fn min_booleans_not_comparable_errors() {
    let sc = StaticContext::default();
    let ex = compile_xpath("min((true(), false()))", &sc).unwrap();
    let res: Result<Vec<XdmItem<DummyNode>>, Error> = ex.evaluate(&Default::default());
    assert!(res.is_err());
    assert_eq!(res.err().unwrap().code, "err:FORG0006");
}
