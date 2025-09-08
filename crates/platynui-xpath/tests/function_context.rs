use platynui_xpath::compile_xpath;
use platynui_xpath::model::{NodeKind, QName, XdmNode};
use platynui_xpath::runtime::{CallCtx, Error, FunctionRegistry, StaticContext};
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem, XdmSequence};
use rstest::rstest;
use std::sync::Arc;

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
    match &items[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => *b,
        _ => panic!("bool expected"),
    }
}

#[rstest]
fn callctx_exposes_default_collation() {
    // Build a custom registry with a probe function that inspects CallCtx
    let mut reg: FunctionRegistry<DummyNode> = FunctionRegistry::new();
    let fns = "http://www.w3.org/2005/xpath-functions".to_string();
    let name = platynui_xpath::xdm::ExpandedName { ns_uri: Some(fns), local: "probe-collation".into() };
    let fun = Arc::new(|ctx: &CallCtx<DummyNode>, _args: &[XdmSequence<DummyNode>]| -> Result<XdmSequence<DummyNode>, Error> {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(ctx.default_collation.is_some()))])
    });
    reg.register(name, 0, fun);

    let sc = StaticContext::default();
    let exec = compile_xpath("probe-collation()", &sc).unwrap();
    let ctx = platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new()
        .with_functions(Arc::new(reg))
        .build();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx).unwrap();
    assert!(as_bool(&out));
}

fn as_string<N>(items: &Vec<XdmItem<N>>) -> String {
    match &items[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => s.clone(),
        _ => panic!("string expected"),
    }
}

#[rstest]
fn default_collation_uri_fallback_on_dynamic_unknown() {
    // Function that returns the default collation URI
    let mut reg: FunctionRegistry<DummyNode> = FunctionRegistry::new();
    let fns = "http://www.w3.org/2005/xpath-functions".to_string();
    let name = platynui_xpath::xdm::ExpandedName { ns_uri: Some(fns), local: "probe-collation-uri".into() };
    let fun = Arc::new(|ctx: &CallCtx<DummyNode>, _args: &[XdmSequence<DummyNode>]| -> Result<XdmSequence<DummyNode>, Error> {
        let uri = ctx
            .default_collation
            .as_ref()
            .map(|c| c.uri().to_string())
            .unwrap_or_default();
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(uri))])
    });
    reg.register(name, 0, fun);

    let sc = StaticContext::default();
    let exec = compile_xpath("probe-collation-uri()", &sc).unwrap();
    let ctx = platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new()
        .with_functions(Arc::new(reg))
        .with_default_collation("urn:unknown") // should fall back to codepoint
        .build();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx).unwrap();
    assert_eq!(
        as_string(&out),
        "http://www.w3.org/2005/xpath-functions/collation/codepoint"
    );
}

#[rstest]
fn default_collation_uri_fallback_on_static_unknown() {
    // Same function registry as above
    let mut reg: FunctionRegistry<DummyNode> = FunctionRegistry::new();
    let fns = "http://www.w3.org/2005/xpath-functions".to_string();
    let name = platynui_xpath::xdm::ExpandedName { ns_uri: Some(fns), local: "probe-collation-uri".into() };
    let fun = Arc::new(|ctx: &CallCtx<DummyNode>, _args: &[XdmSequence<DummyNode>]| -> Result<XdmSequence<DummyNode>, Error> {
        let uri = ctx
            .default_collation
            .as_ref()
            .map(|c| c.uri().to_string())
            .unwrap_or_default();
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(uri))])
    });
    reg.register(name, 0, fun);

    let mut sc = StaticContext::default();
    sc.default_collation = Some("urn:unknown".into());
    let exec = compile_xpath("probe-collation-uri()", &sc).unwrap();
    let ctx = platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new()
        .with_functions(Arc::new(reg))
        .build();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx).unwrap();
    assert_eq!(
        as_string(&out),
        "http://www.w3.org/2005/xpath-functions/collation/codepoint"
    );
}

#[rstest]
fn callctx_exposes_regex_and_resolver_none_by_default() {
    let mut reg: FunctionRegistry<DummyNode> = FunctionRegistry::new();
    let fns = "http://www.w3.org/2005/xpath-functions".to_string();
    let name = platynui_xpath::xdm::ExpandedName { ns_uri: Some(fns), local: "probe-services".into() };
    let fun = Arc::new(|ctx: &CallCtx<DummyNode>, _args: &[XdmSequence<DummyNode>]| -> Result<XdmSequence<DummyNode>, Error> {
        let b = ctx.regex.is_none() && ctx.resolver.is_none();
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))])
    });
    reg.register(name, 0, fun);

    let sc = StaticContext::default();
    let exec = compile_xpath("probe-services()", &sc).unwrap();
    let ctx = platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new()
        .with_functions(Arc::new(reg))
        .build();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx).unwrap();
    assert!(as_bool(&out));
}
