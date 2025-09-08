use chrono::{DateTime, FixedOffset};
use platynui_xpath::compile_xpath;
use platynui_xpath::model::{NodeKind, QName, XdmNode};
use platynui_xpath::runtime::StaticContext;
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
fn current_datetime_with_now() {
    let sc = StaticContext::default();
    let exec_dt = compile_xpath("current-dateTime()", &sc).unwrap();
    let exec_d = compile_xpath("current-date()", &sc).unwrap();
    let exec_t = compile_xpath("current-time()", &sc).unwrap();

    let now: DateTime<FixedOffset> = DateTime::parse_from_rfc3339("2024-01-02T03:04:05+02:30").unwrap();
    let ctx = platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new()
        .with_now(now)
        .build();

    let out_dt: Vec<XdmItem<DummyNode>> = exec_dt.evaluate(&ctx).unwrap();
    let out_d: Vec<XdmItem<DummyNode>> = exec_d.evaluate(&ctx).unwrap();
    let out_t: Vec<XdmItem<DummyNode>> = exec_t.evaluate(&ctx).unwrap();

    assert_eq!(as_string(&out_dt), "2024-01-02T03:04:05+02:30");
    assert_eq!(as_string(&out_d), "2024-01-02+02:30");
    assert_eq!(as_string(&out_t), "03:04:05+02:30");
}

#[rstest]
fn current_datetime_with_timezone_override() {
    let sc = StaticContext::default();
    let exec_dt = compile_xpath("current-dateTime()", &sc).unwrap();
    let exec_d = compile_xpath("current-date()", &sc).unwrap();
    let exec_t = compile_xpath("current-time()", &sc).unwrap();

    let now: DateTime<FixedOffset> = DateTime::parse_from_rfc3339("2024-01-02T03:04:05+00:00").unwrap();
    let ctx = platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new()
        .with_now(now)
        .with_timezone(120) // +02:00
        .build();

    let out_dt: Vec<XdmItem<DummyNode>> = exec_dt.evaluate(&ctx).unwrap();
    let out_d: Vec<XdmItem<DummyNode>> = exec_d.evaluate(&ctx).unwrap();
    let out_t: Vec<XdmItem<DummyNode>> = exec_t.evaluate(&ctx).unwrap();

    assert_eq!(as_string(&out_dt), "2024-01-02T05:04:05+02:00");
    assert_eq!(as_string(&out_d), "2024-01-02+02:00");
    assert_eq!(as_string(&out_t), "05:04:05+02:00");
}

