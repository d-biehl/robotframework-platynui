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
fn union_intersect_except_require_nodes() {
    let sc = StaticContext::default();
    for expr in ["1 | 2", "1 intersect 2", "1 except 2"] {
        let ex = compile_xpath(expr, &sc).unwrap();
        let res: Result<Vec<XdmItem<DummyNode>>, Error> = ex.evaluate(&Default::default());
        assert!(res.is_err(), "{} should error", expr);
        assert_eq!(res.err().unwrap().code, "err:XPTY0020");
    }
}
