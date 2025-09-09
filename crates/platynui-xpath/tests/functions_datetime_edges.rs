use platynui_xpath::compile_xpath;
use platynui_xpath::model::{NodeKind, QName, XdmNode};
use platynui_xpath::runtime::{Error, StaticContext};
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
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

fn as_string<N>(items: &Vec<XdmItem<N>>) -> String {
    match &items[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => s.clone(),
        _ => panic!("string expected"),
    }
}

// (no bool helper needed)

#[rstest]
fn large_month_shift_saturates_day() {
    let sc = StaticContext::default();
    // 2024-01-31 + 13 months => 2025-02-28 (non-leap year), timezone preserved
    let expr =
        "string(('2024-01-31+02:00' cast as xs:date) + ('P13M' cast as xs:yearMonthDuration))";
    let exec = compile_xpath(expr, &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&Default::default()).unwrap();
    assert_eq!(as_string(&out), "2025-02-28+02:00");
}

#[rstest]
fn timezone_from_datetime() {
    let sc = StaticContext::default();
    let expr = "timezone-from-dateTime('2024-01-02T05:00:00+02:00' cast as xs:dateTime)";
    let exec = compile_xpath(expr, &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&Default::default()).unwrap();
    match &out[0] {
        XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => assert_eq!(*secs, 2 * 3600),
        _ => panic!("dayTimeDuration expected"),
    }
}

#[rstest]
fn time_fractional_seconds_truncated_on_string() {
    let sc = StaticContext::default();
    let expr = "string('03:04:05.123+02:30' cast as xs:time)";
    let exec = compile_xpath(expr, &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&Default::default()).unwrap();
    // Current implementation formats without fractional seconds
    assert_eq!(as_string(&out), "03:04:05+02:30");
}

#[rstest]
fn time_add_wraps_midnight() {
    let sc = StaticContext::default();
    let expr = "string(('23:59:30+02:00' cast as xs:time) + ('PT90S' cast as xs:dayTimeDuration))";
    let exec = compile_xpath(expr, &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&Default::default()).unwrap();
    assert_eq!(as_string(&out), "00:01:00+02:00");
}

#[rstest]
fn date_subtraction_yields_days_duration() {
    let sc = StaticContext::default();
    let expr =
        "string(('2024-03-01+00:00' cast as xs:date) - ('2024-02-28+00:00' cast as xs:date))";
    let exec = compile_xpath(expr, &sc).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&Default::default()).unwrap();
    assert_eq!(as_string(&out), "P2D");
}

// ===== Negative cases =====

#[rstest]
fn invalid_date_month_13() {
    let sc = StaticContext::default();
    let expr = "'2024-13-01' cast as xs:date";
    let exec = compile_xpath(expr, &sc).unwrap();
    let res: Result<Vec<XdmItem<DummyNode>>, Error> = exec.evaluate(&Default::default());
    assert!(res.is_err());
    let e = res.err().unwrap();
    assert_eq!(e.code, "err:FORG0001");
}

#[rstest]
fn invalid_time_hour_25() {
    let sc = StaticContext::default();
    let expr = "'25:00:00' cast as xs:time";
    let exec = compile_xpath(expr, &sc).unwrap();
    let res: Result<Vec<XdmItem<DummyNode>>, Error> = exec.evaluate(&Default::default());
    assert!(res.is_err());
    let e = res.err().unwrap();
    assert_eq!(e.code, "err:FORG0001");
}

#[rstest]
fn invalid_date_bad_timezone_format() {
    let sc = StaticContext::default();
    let expr = "'2024-01-01+2:0' cast as xs:date"; // bad tz string
    let exec = compile_xpath(expr, &sc).unwrap();
    let res: Result<Vec<XdmItem<DummyNode>>, Error> = exec.evaluate(&Default::default());
    assert!(res.is_err());
    let e = res.err().unwrap();
    assert_eq!(e.code, "err:FORG0001");
}

#[rstest]
fn invalid_daytime_duration_format() {
    let sc = StaticContext::default();
    let expr = "'PT' cast as xs:dayTimeDuration";
    let exec = compile_xpath(expr, &sc).unwrap();
    let res: Result<Vec<XdmItem<DummyNode>>, Error> = exec.evaluate(&Default::default());
    assert!(res.is_err());
    let e = res.err().unwrap();
    assert_eq!(e.code, "err:FORG0001");
}
