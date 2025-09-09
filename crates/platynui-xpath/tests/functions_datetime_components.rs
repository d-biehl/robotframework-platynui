// no external chrono imports needed here
use platynui_xpath::compile_xpath;
use platynui_xpath::model::{NodeKind, QName, XdmNode};
use platynui_xpath::runtime::StaticContext;
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

fn first_int<N>(items: &Vec<XdmItem<N>>) -> i64 {
    match &items[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(i)) => *i,
        _ => panic!("integer expected"),
    }
}

#[rstest]
fn year_month_day_from_date_time() {
    let sc = StaticContext::default();
    let expr_y = "year-from-dateTime('2024-01-02T03:04:05+02:30' cast as xs:dateTime)";
    let expr_m = "month-from-dateTime('2024-01-02T03:04:05+02:30' cast as xs:dateTime)";
    let expr_d = "day-from-dateTime('2024-01-02T03:04:05+02:30' cast as xs:dateTime)";
    let ex_y = compile_xpath(expr_y, &sc).unwrap();
    let ex_m = compile_xpath(expr_m, &sc).unwrap();
    let ex_d = compile_xpath(expr_d, &sc).unwrap();

    let ctx = platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build();
    let out_y: Vec<XdmItem<DummyNode>> = ex_y.evaluate(&ctx).unwrap();
    let out_m: Vec<XdmItem<DummyNode>> = ex_m.evaluate(&ctx).unwrap();
    let out_d: Vec<XdmItem<DummyNode>> = ex_d.evaluate(&ctx).unwrap();

    assert_eq!(first_int(&out_y), 2024);
    assert_eq!(first_int(&out_m), 1);
    assert_eq!(first_int(&out_d), 2);
}

#[rstest]
fn time_components_and_implicit_timezone() {
    let sc = StaticContext::default();
    let ex_h = compile_xpath("hours-from-time('03:04:05+02:30' cast as xs:time)", &sc).unwrap();
    let ex_m = compile_xpath("minutes-from-time('03:04:05+02:30' cast as xs:time)", &sc).unwrap();
    let ex_s = compile_xpath("seconds-from-time('03:04:05+02:30' cast as xs:time)", &sc).unwrap();
    let ex_tz = compile_xpath("implicit-timezone()", &sc).unwrap();

    let ctx = platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new()
        .with_timezone(150) // +02:30
        .build();

    let out_h: Vec<XdmItem<DummyNode>> = ex_h.evaluate(&ctx).unwrap();
    let out_mi: Vec<XdmItem<DummyNode>> = ex_m.evaluate(&ctx).unwrap();
    let out_s: Vec<XdmItem<DummyNode>> = ex_s.evaluate(&ctx).unwrap();
    let out_tz: Vec<XdmItem<DummyNode>> = ex_tz.evaluate(&ctx).unwrap();

    assert_eq!(first_int(&out_h), 3);
    assert_eq!(first_int(&out_mi), 4);
    assert_eq!(first_int(&out_s), 5);

    match &out_tz[0] {
        XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => {
            assert_eq!(*secs, 2 * 3600 + 30 * 60);
        }
        _ => panic!("expected dayTimeDuration"),
    }
}
