use platynui_xpath::compile_xpath;
use platynui_xpath::model::{NodeKind, QName, XdmNode};
use platynui_xpath::runtime::StaticContext;
use platynui_xpath::xdm::{XdmItem, XdmAtomicValue};
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

fn as_bool<N>(items: &Vec<XdmItem<N>>) -> bool {
    match &items[0] { XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => *b, _ => panic!("bool expected") }
}
fn as_string<N>(items: &Vec<XdmItem<N>>) -> String {
    match &items[0] { XdmItem::Atomic(XdmAtomicValue::String(s)) => s.clone(), _ => panic!("string expected") }
}

#[rstest]
fn matches_basic_true() {
    let sc = StaticContext::default();
    let exec = compile_xpath("matches('abc','b')", &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build()).unwrap();
    assert!(as_bool(&out));
}

#[rstest]
fn matches_case_insensitive_flag() {
    let sc = StaticContext::default();
    let exec = compile_xpath("matches('ABC','b','i')", &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build()).unwrap();
    assert!(as_bool(&out));
}

#[rstest]
fn matches_multiline_flag_anchors() {
    let sc = StaticContext::default();
    let exec = compile_xpath(r#"matches("a
b
c", '^b', 'm')"#, &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec
        .evaluate(&platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build())
        .unwrap();
    assert!(as_bool(&out));
}

#[rstest]
fn matches_dotall_flag() {
    let sc = StaticContext::default();
    let exec_true = compile_xpath(r#"matches("a
b", 'a.b', 's')"#, &sc).unwrap();
    let exec_false = compile_xpath(r#"matches("a
b", 'a.b')"#, &sc).unwrap();
    let out_true: Vec<XdmItem<DummyNode>> = exec_true
        .evaluate(&platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build())
        .unwrap();
    let out_false: Vec<XdmItem<DummyNode>> = exec_false
        .evaluate(&platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build())
        .unwrap();
    assert!(as_bool(&out_true));
    assert!(!as_bool(&out_false));
}

#[rstest]
fn matches_free_spacing_flag() {
    let sc = StaticContext::default();
    let exec = compile_xpath("matches('ab', 'a b', 'x')", &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec
        .evaluate(&platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build())
        .unwrap();
    assert!(as_bool(&out));
}

#[rstest]
fn matches_invalid_flag_errors() {
    let sc = StaticContext::default();
    let exec = compile_xpath("matches('a','a','z')", &sc).unwrap();
    let err = exec
        .evaluate(&platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build())
        .expect_err("FORX0002 expected");
    assert_eq!(err.code, "err:FORX0002");
}

#[rstest]
fn replace_basic() {
    let sc = StaticContext::default();
    let exec = compile_xpath("replace('abc','b','X')", &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec
        .evaluate(&platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build())
        .unwrap();
    assert_eq!(as_string(&out), "aXc");
}

#[rstest]
fn tokenize_basic() {
    let sc = StaticContext::default();
    let exec = compile_xpath("tokenize('a,b;c','[,;]')", &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec
        .evaluate(&platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build())
        .unwrap();
    let parts: Vec<String> = out
        .into_iter()
        .map(|it| match it { XdmItem::Atomic(XdmAtomicValue::String(s)) => s, _ => panic!("string") })
        .collect();
    assert_eq!(parts, vec!["a", "b", "c"]);
}

#[rstest]
fn matches_invalid_pattern_errors() {
    let sc = StaticContext::default();
    let exec = compile_xpath("matches('a','(')", &sc).unwrap();
    let err = exec
        .evaluate(&platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build())
        .expect_err("FORX0002 expected");
    assert_eq!(err.code, "err:FORX0002");
}
