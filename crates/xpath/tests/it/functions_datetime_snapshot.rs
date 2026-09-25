use chrono::{FixedOffset, TimeZone};
use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmAtomicValue as A, xdm::XdmItem as I};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;

fn dt(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32, offset_min: i32) -> chrono::DateTime<FixedOffset> {
    let tz = FixedOffset::east_opt(offset_min * 60).unwrap();
    tz.with_ymd_and_hms(y, mo, d, h, mi, s).unwrap()
}

#[rstest]
fn snapshot_defaults_utc() {
    // Now with UTC offset; no override
    let now = dt(2025, 9, 12, 1, 2, 3, 0);
    let ctx = DynamicContextBuilder::default().with_now(now).build();

    let r_dt = evaluate_expr::<N>("current-dateTime()", &ctx).unwrap();
    let r_d = evaluate_expr::<N>("current-date()", &ctx).unwrap();
    let r_t = evaluate_expr::<N>("current-time()", &ctx).unwrap();
    let r_tz = evaluate_expr::<N>("implicit-timezone()", &ctx).unwrap();

    match &r_dt[0] {
        I::Atomic(A::DateTime(dt_out)) => assert_eq!(*dt_out, now),
        _ => panic!("expected dateTime"),
    }
    match &r_d[0] {
        I::Atomic(A::Date { date, tz }) => {
            assert_eq!(*date, now.date_naive());
            assert_eq!(tz.unwrap().local_minus_utc(), 0);
        }
        _ => panic!("expected date"),
    }
    match &r_t[0] {
        I::Atomic(A::Time { time, tz }) => {
            assert_eq!(*time, now.time());
            assert_eq!(tz.unwrap().local_minus_utc(), 0);
        }
        _ => panic!("expected time"),
    }
    if let I::Atomic(A::DayTimeDuration(secs)) = &r_tz[0] {
        assert_eq!(*secs, 0);
    } else {
        panic!("expected DayTimeDuration");
    }
}

#[rstest]
fn snapshot_with_timezone_override() {
    // Base now at UTC; override to +02:30
    let now = dt(2025, 9, 12, 1, 2, 3, 0);
    let ctx = DynamicContextBuilder::default().with_now(now).with_timezone(150).build();

    let r_dt = evaluate_expr::<N>("current-dateTime()", &ctx).unwrap();
    let r_d = evaluate_expr::<N>("current-date()", &ctx).unwrap();
    let r_t = evaluate_expr::<N>("current-time()", &ctx).unwrap();
    let r_tz = evaluate_expr::<N>("implicit-timezone()", &ctx).unwrap();

    // Local time shifts by +02:30 from 01:02:03 -> 03:32:03
    let tz = chrono::FixedOffset::east_opt(150 * 60).unwrap();
    let expected = now.with_timezone(&tz);
    match &r_dt[0] {
        I::Atomic(A::DateTime(dt_out)) => assert_eq!(*dt_out, expected),
        _ => panic!("expected dateTime"),
    }
    match &r_d[0] {
        I::Atomic(A::Date { date, tz }) => {
            assert_eq!(*date, expected.date_naive());
            assert_eq!(tz.unwrap().local_minus_utc(), 150 * 60);
        }
        _ => panic!("expected date"),
    }
    match &r_t[0] {
        I::Atomic(A::Time { time, tz }) => {
            assert_eq!(*time, expected.time());
            assert_eq!(tz.unwrap().local_minus_utc(), 150 * 60);
        }
        _ => panic!("expected time"),
    }
    if let I::Atomic(A::DayTimeDuration(secs)) = &r_tz[0] {
        assert_eq!(*secs, 150 * 60);
    } else {
        panic!("expected DayTimeDuration");
    }
}

#[rstest]
fn snapshot_respects_now_offset_no_override() {
    // Now carries -04:00 offset; no override set
    let now = dt(2025, 9, 12, 7, 8, 9, -240);
    let ctx = DynamicContextBuilder::default().with_now(now).build();

    let r_dt = evaluate_expr::<N>("current-dateTime()", &ctx).unwrap();
    let r_d = evaluate_expr::<N>("current-date()", &ctx).unwrap();
    let r_t = evaluate_expr::<N>("current-time()", &ctx).unwrap();
    let r_tz = evaluate_expr::<N>("implicit-timezone()", &ctx).unwrap();

    match &r_dt[0] {
        I::Atomic(A::DateTime(dt_out)) => assert_eq!(*dt_out, now),
        _ => panic!("expected dateTime"),
    }
    match &r_d[0] {
        I::Atomic(A::Date { date, tz }) => {
            assert_eq!(*date, now.date_naive());
            assert_eq!(tz.unwrap().local_minus_utc(), -4 * 3600);
        }
        _ => panic!("expected date"),
    }
    match &r_t[0] {
        I::Atomic(A::Time { time, tz }) => {
            assert_eq!(*time, now.time());
            assert_eq!(tz.unwrap().local_minus_utc(), -4 * 3600);
        }
        _ => panic!("expected time"),
    }
    if let I::Atomic(A::DayTimeDuration(secs)) = &r_tz[0] {
        assert_eq!(*secs, -4 * 3600);
    } else {
        panic!("expected DayTimeDuration");
    }
}
