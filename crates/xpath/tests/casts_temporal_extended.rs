use chrono::prelude::*; // bring year(), month(), etc. into scope
use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::runtime::ErrorCode;
use platynui_xpath::{
    StaticContextBuilder, compile_with_context, evaluate_expr, evaluator::evaluate, xdm::XdmAtomicValue as A,
    xdm::XdmItem as I,
};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;
fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

fn expect_err(expr: &str, frag: &str) {
    let c = ctx();
    let err = evaluate_expr::<N>(expr, &c).unwrap_err();
    // For now, only FORG0001 cases are used here; map requested frag to enum when known
    if frag == "FORG0001" {
        assert_eq!(err.code_enum(), ErrorCode::FORG0001, "expected {frag} in {expr} => {:?}", err.code_qname());
    } else if frag == "XPTY0004" {
        assert_eq!(err.code_enum(), ErrorCode::XPTY0004, "expected {frag} in {expr} => {:?}", err.code_qname());
    } else {
        // Fallback: check local part of QName contains fragment
        let q = err.code_qname().unwrap();
        assert!(q.local.contains(frag), "expected fragment {frag} in {expr} => {:?}", q);
    }
}

#[rstest]
#[case("xs:date('2024-02-29')", "2024-02-29")] // leap day
#[case("xs:date('2025-09-12Z')", "2025-09-12")] // with Z timezone
fn cast_date_basic(#[case] expr: &str, #[case] date_str: &str) {
    let c = ctx();
    let r = evaluate_expr::<N>(expr, &c).unwrap();
    if let I::Atomic(A::Date { date, .. }) = &r[0] {
        assert_eq!(date.to_string(), date_str);
    } else {
        panic!("expected date");
    }
}

#[rstest]
fn cast_date_invalid_day() {
    expect_err("xs:date('2025-02-30')", "FORG0001");
}

#[rstest]
#[case("xs:time('23:59:59')", (23,59,59))]
#[case("xs:time('00:00:00Z')", (0,0,0))]
fn cast_time_basic(#[case] expr: &str, #[case] hms: (u32, u32, u32)) {
    let c = ctx();
    let r = evaluate_expr::<N>(expr, &c).unwrap();
    if let I::Atomic(A::Time { time, .. }) = &r[0] {
        assert_eq!((time.hour(), time.minute(), time.second()), hms);
    } else {
        panic!("expected time");
    }
}

#[rstest]
fn cast_time_invalid() {
    expect_err("xs:time('25:00:00')", "FORG0001");
}

#[rstest]
#[case("xs:dateTime('2025-09-12T01:02:03Z')", (2025,9,12,1,2,3))]
fn cast_datetime_basic(#[case] expr: &str, #[case] ymdhms: (i32, u32, u32, u32, u32, u32)) {
    let c = ctx();
    let r = evaluate_expr::<N>(expr, &c).unwrap();
    if let I::Atomic(A::DateTime(dt)) = &r[0] {
        assert_eq!((dt.year(), dt.month(), dt.day(), dt.hour(), dt.minute(), dt.second()), ymdhms);
    } else {
        panic!("expected dateTime");
    }
}

#[rstest]
fn cast_datetime_invalid() {
    expect_err("xs:dateTime('2025-13-01T00:00:00Z')", "FORG0001");
}

#[rstest]
#[case("xs:yearMonthDuration('-P2Y3M')", -27)]
#[case("xs:yearMonthDuration('P0Y0M')", 0)]
fn cast_ym_duration_extended(#[case] expr: &str, #[case] months: i32) {
    let c = ctx();
    let r = evaluate_expr::<N>(expr, &c).unwrap();
    if let I::Atomic(A::YearMonthDuration(m)) = &r[0] {
        assert_eq!(*m, months);
    } else {
        panic!("expected yearMonthDuration");
    }
}

#[rstest]
#[case("xs:dayTimeDuration('-P1DT2H')", -(86400 + 2*3600))]
#[case("xs:dayTimeDuration('P0D')", 0)]
fn cast_dt_duration_extended(#[case] expr: &str, #[case] secs: i64) {
    let c = ctx();
    let r = evaluate_expr::<N>(expr, &c).unwrap();
    if let I::Atomic(A::DayTimeDuration(s)) = &r[0] {
        assert_eq!(*s, secs);
    } else {
        panic!("expected dayTimeDuration");
    }
}

#[rstest]
fn cast_dt_duration_invalid_time_component() {
    expect_err("xs:dayTimeDuration('P1D2H')", "FORG0001");
}

// QName extended tests
fn static_ctx_with_ns() -> platynui_xpath::engine::runtime::StaticContext {
    StaticContextBuilder::default().with_namespace("ex", "http://example.com").build()
}

#[rstest]
fn cast_qname_with_prefix_success() {
    let sc = static_ctx_with_ns();
    let dc: platynui_xpath::engine::runtime::DynamicContext<N> = DynamicContextBuilder::default().build();
    let compiled = compile_with_context("xs:QName('ex:local')", &sc).unwrap();
    let r = evaluate(&compiled, &dc).unwrap();
    if let I::Atomic(A::QName { prefix, local, ns_uri }) = &r[0] {
        assert_eq!(prefix.as_deref(), Some("ex"));
        assert_eq!(local, "local");
        assert_eq!(ns_uri.as_deref(), Some("http://example.com"));
    } else {
        panic!("expected QName");
    }
}

#[rstest]
fn cast_qname_with_unknown_prefix_error() {
    let sc = StaticContextBuilder::default().with_namespace("ex", "http://example.com").build();
    let dc: platynui_xpath::engine::runtime::DynamicContext<N> = DynamicContextBuilder::default().build();
    let compiled = compile_with_context("xs:QName('foo:local')", &sc).unwrap();
    let err = evaluate(&compiled, &dc).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::FORG0001);
}
