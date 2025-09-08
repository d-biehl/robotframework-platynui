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

fn expect_string<N>(items: &Vec<XdmItem<N>>) -> String {
    match &items[0] { XdmItem::Atomic(XdmAtomicValue::String(s)) => s.clone(), _ => panic!("string expected") }
}

// (no integer helper needed)

#[rstest]
fn datetime_add_daytimeduration() {
    let sc = StaticContext::default();
    // sanity: string rendering of cast dateTime
    let str_dt = compile_xpath("string('2024-01-02T03:04:05+00:00' cast as xs:dateTime)", &sc).unwrap();
    let out_str_dt: Vec<XdmItem<DummyNode>> = str_dt.evaluate(&Default::default()).unwrap();
    assert_eq!(expect_string(&out_str_dt), "2024-01-02T03:04:05+00:00");

    // sanity: yearMonthDuration and dayTimeDuration string render from cast
    let ymd = compile_xpath("string('P6M' cast as xs:yearMonthDuration)", &sc).unwrap();
    let out_ymd: Vec<XdmItem<DummyNode>> = ymd.evaluate(&Default::default()).unwrap();
    assert_eq!(expect_string(&out_ymd), "P6M");
    let ymd1 = compile_xpath("string('P1M' cast as xs:yearMonthDuration)", &sc).unwrap();
    let out_ymd1: Vec<XdmItem<DummyNode>> = ymd1.evaluate(&Default::default()).unwrap();
    assert_eq!(expect_string(&out_ymd1), "P1M");
    let dtd = compile_xpath("string('PT10S' cast as xs:dayTimeDuration)", &sc).unwrap();
    let out_dtd: Vec<XdmItem<DummyNode>> = dtd.evaluate(&Default::default()).unwrap();
    assert_eq!(expect_string(&out_dtd), "PT10S");
    let expr = "('2024-01-02T03:04:05+00:00' cast as xs:dateTime) + ('P1DT2H' cast as xs:dayTimeDuration)";
    let exec = compile_xpath(expr, &sc).unwrap();
    let _out: Vec<XdmItem<DummyNode>> = exec.evaluate(&Default::default()).unwrap();
    // Our evaluator formats dateTime to string via string() cast path in tests; to check arith, cast to string explicitly
    let exec_str = compile_xpath("string(('2024-01-02T03:04:05+00:00' cast as xs:dateTime) + ('P1DT2H' cast as xs:dayTimeDuration))", &sc).unwrap();
    let out_str: Vec<XdmItem<DummyNode>> = exec_str.evaluate(&Default::default()).unwrap();
    assert_eq!(expect_string(&out_str), "2024-01-03T05:04:05+00:00");
}

#[rstest]
fn date_add_yearmonthduration() {
    let sc = StaticContext::default();
    // sanity: render casted date
    let sdate = compile_xpath("string('2024-01-31+02:00' cast as xs:date)", &sc).unwrap();
    let sdate_o: Vec<XdmItem<DummyNode>> = sdate.evaluate(&Default::default()).unwrap();
    assert_eq!(expect_string(&sdate_o), "2024-01-31+02:00");
    // sanity: render casted yearMonthDuration
    let exec = compile_xpath("string(('2024-01-31+02:00' cast as xs:date) + ('P1M' cast as xs:yearMonthDuration))", &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&Default::default()).unwrap();
    // 2024-02-29 (leap year), saturate day
    assert_eq!(expect_string(&out), "2024-02-29+02:00");
}

#[rstest]
fn duration_mul_div() {
    let sc = StaticContext::default();
    let exec1 = compile_xpath("string(('PT10S' cast as xs:dayTimeDuration) * 3)", &sc).unwrap();
    let out1: Vec<XdmItem<DummyNode>> = exec1.evaluate(&Default::default()).unwrap();
    assert_eq!(expect_string(&out1), "PT30S");

    let exec2 = compile_xpath("('P2Y' cast as xs:yearMonthDuration) div ('P6M' cast as xs:yearMonthDuration)", &sc).unwrap();
    let out2: Vec<XdmItem<DummyNode>> = exec2.evaluate(&Default::default()).unwrap();
    assert!(matches!(&out2[0], XdmItem::Atomic(XdmAtomicValue::Double(d)) if (*d - 4.0).abs() < 1e-9));

    // ymd div number
    let exec3 = compile_xpath("string(('P6M' cast as xs:yearMonthDuration) div 2)", &sc).unwrap();
    let out3: Vec<XdmItem<DummyNode>> = exec3.evaluate(&Default::default()).unwrap();
    assert_eq!(expect_string(&out3), "P3M");

    // ymd + ymd
    let exec4 = compile_xpath("string(('P2Y' cast as xs:yearMonthDuration) + ('P6M' cast as xs:yearMonthDuration))", &sc).unwrap();
    let out4: Vec<XdmItem<DummyNode>> = exec4.evaluate(&Default::default()).unwrap();
    assert_eq!(expect_string(&out4), "P2Y6M");
}

#[rstest]
fn datetime_comparisons() {
    let sc = StaticContext::default();
    let exec = compile_xpath("('2024-01-02T00:00:00+00:00' cast as xs:dateTime) lt ('2024-01-03T00:00:00+00:00' cast as xs:dateTime)", &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&Default::default()).unwrap();
    match &out[0] { XdmItem::Atomic(XdmAtomicValue::Boolean(true)) => {}, _ => panic!("expected true") }
}
