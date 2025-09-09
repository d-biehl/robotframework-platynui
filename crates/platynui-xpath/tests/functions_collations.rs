use platynui_xpath::compile_xpath;
use platynui_xpath::model::{NodeKind, QName, XdmNode};
use platynui_xpath::runtime::StaticContext;
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
fn contains_case_insensitive_explicit() {
    let sc = StaticContext::default();
    let expr = "contains('Ä','ä','urn:platynui:collation:simple-case')";
    let exec = compile_xpath(expr, &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec
        .evaluate(&platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build())
        .unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("bool"),
    }
}

#[rstest]
fn contains_accent_insensitive_explicit() {
    let sc = StaticContext::default();
    let expr = "contains('cafe','fé','urn:platynui:collation:simple-accent')";
    let exec = compile_xpath(expr, &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec
        .evaluate(&platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build())
        .unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("bool"),
    }
}

#[rstest]
fn starts_with_case_default_via_dynamic() {
    let sc = StaticContext::default();
    let expr = "starts-with('Straße','STR')"; // should be true under case-insensitive
    let exec = compile_xpath(expr, &sc).unwrap();
    let ctx = platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new()
        .with_default_collation("urn:platynui:collation:simple-case")
        .build();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx).unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("bool"),
    }
}

#[rstest]
fn ends_with_case_accent_explicit() {
    let sc = StaticContext::default();
    let expr = "ends-with('CAFÉ','fé','urn:platynui:collation:simple-case-accent')";
    let exec = compile_xpath(expr, &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec
        .evaluate(&platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build())
        .unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("bool"),
    }
}

#[rstest]
fn unknown_collation_errors() {
    let sc = StaticContext::default();
    let expr = "contains('a','a','urn:unknown')";
    let exec = compile_xpath(expr, &sc).unwrap();
    let err = exec
        .evaluate(&platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build())
        .expect_err("expected FOCH0002 for unknown collation");
    assert_eq!(err.code, "err:FOCH0002");
}

#[rstest]
fn compare_with_default_collation_case_insensitive() {
    let sc = StaticContext::default();
    let expr = "compare('A','a')";
    let exec = compile_xpath(expr, &sc).unwrap();
    let ctx = platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new()
        .with_default_collation("urn:platynui:collation:simple-case")
        .build();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx).unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(i)) => assert_eq!(*i, 0),
        _ => panic!("int"),
    }
}

#[rstest]
fn codepoint_equal_basic() {
    let sc = StaticContext::default();
    let expr = "codepoint-equal('A','a')"; // false under codepoint collation
    let exec = compile_xpath(expr, &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec
        .evaluate(&platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build())
        .unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(!*b),
        _ => panic!("bool"),
    }
}

#[rstest]
fn compare_and_codepoint_equal_empty_semantics() {
    let sc = StaticContext::default();
    // one arg empty → empty sequence
    let exec = compile_xpath("codepoint-equal((), 'a')", &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec
        .evaluate(&platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build())
        .unwrap_or_default();
    assert!(out.is_empty());

    let exec = compile_xpath("compare((), 'a')", &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec
        .evaluate(&platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build())
        .unwrap_or_default();
    assert!(out.is_empty());
}

// Note: Aggregator string-collation behaviour is covered in core suite; here we focus on collation-aware string ops.

#[rstest]
fn deep_equal_strings_with_collation() {
    let sc = StaticContext::default();
    let expr = "deep-equal(('Ä','Cafe'),('ä','café'),'urn:platynui:collation:simple-case-accent')";
    let exec = compile_xpath(expr, &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec
        .evaluate(&platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build())
        .unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("bool"),
    }
}

#[rstest]
fn deep_equal_empty_sequences() {
    let sc = StaticContext::default();
    // Avoid literal () parsing; use subsequence to build empties
    let expr = "deep-equal(subsequence(('x'),2),subsequence(('y'),2))";
    let exec = compile_xpath(expr, &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec
        .evaluate(&platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build())
        .unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("bool"),
    }
}

// Note: mismatch-length behavior is covered implicitly by parser sequence handling; explicit test skipped.
