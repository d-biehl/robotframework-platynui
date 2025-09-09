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
fn unknown_variable_errors() {
    let sc = StaticContext::default();
    let ex = compile_xpath("$does_not_exist", &sc).unwrap();
    let res: Result<Vec<XdmItem<DummyNode>>, Error> = ex.evaluate(&Default::default());
    assert!(res.is_err());
    assert_eq!(res.err().unwrap().code, "err:XPST0008");
}

#[rstest]
fn unknown_function_errors() {
    let sc = StaticContext::default();
    let ex = compile_xpath("does-not-exist()", &sc).unwrap();
    let res: Result<Vec<XdmItem<DummyNode>>, Error> = ex.evaluate(&Default::default());
    assert!(res.is_err());
    assert_eq!(res.err().unwrap().code, "err:XPST0017");
}

#[rstest]
fn axis_step_on_atomic_is_static_error() {
    let sc = StaticContext::default();
    let res = platynui_xpath::compile_xpath("'a'/node()", &sc);
    let err = res.expect_err("expected static parse/compile error");
    assert_eq!(err.code, "err:XPST0003");
}
