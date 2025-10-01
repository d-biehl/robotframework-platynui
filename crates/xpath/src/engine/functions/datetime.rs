use super::common::{get_datetime, get_time, now_in_effective_tz, parse_xs_date_local};
use crate::engine::runtime::{CallCtx, Error, ErrorCode};
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence};
use chrono::{Datelike, FixedOffset as ChronoFixedOffset, TimeZone, Timelike, Offset};

pub(super) fn date_time_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() || args[1].is_empty() {
        return Ok(vec![]);
    }
    let (date, tz_date_opt) = match &args[0][0] {
        XdmItem::Atomic(XdmAtomicValue::Date { date, tz }) => (*date, *tz),
        XdmItem::Atomic(XdmAtomicValue::String(s))
        | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            let (d, tzo) = parse_xs_date_local(s)
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:date"))?;
            (d, tzo)
        }
        _ => {
            return Err(Error::from_code(
                ErrorCode::XPTY0004,
                "dateTime expects xs:date? and xs:time?",
            ));
        }
    };
    let (time, tz_time_opt) = match &args[1][0] {
        XdmItem::Atomic(XdmAtomicValue::Time { time, tz }) => (*time, *tz),
        XdmItem::Atomic(XdmAtomicValue::String(s))
        | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            let (t, tzo) = crate::util::temporal::parse_time_lex(s)
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:time"))?;
            (t, tzo)
        }
        _ => {
            return Err(Error::from_code(
                ErrorCode::XPTY0004,
                "dateTime expects xs:date? and xs:time?",
            ));
        }
    };
    let tz = match (tz_date_opt, tz_time_opt) {
        (Some(a), Some(b)) => {
            if a.local_minus_utc() == b.local_minus_utc() {
                Some(a)
            } else {
                return Err(Error::from_code(ErrorCode::FORG0001, "conflicting timezones"));
            }
        }
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    };
    let dt = crate::util::temporal::build_naive_datetime(date, time, tz);
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::DateTime(dt))])
}

pub(super) fn adjust_date_to_timezone_fn<
    N: 'static + Send + Sync + crate::model::XdmNode + Clone,
>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    let tz_opt = if args.len() == 1 || args[1].is_empty() {
        Some(ctx.dyn_ctx.timezone_override.unwrap_or_else(|| {
            ctx.dyn_ctx
                .now
                .map(|n| *n.offset())
                .unwrap_or_else(|| chrono::Utc.fix())
        }))
    } else {
        match &args[1][0] {
            XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => {
                ChronoFixedOffset::east_opt(*secs as i32)
                    .ok_or_else(|| Error::from_code(ErrorCode::FORG0001, "invalid timezone"))?
            }
            _ => {
                return Err(Error::from_code(
                    ErrorCode::XPTY0004,
                    "adjust-date-to-timezone expects xs:dayTimeDuration",
                ));
            }
        }
        .into()
    };
    let (date, _tz) = match &args[0][0] {
        XdmItem::Atomic(XdmAtomicValue::Date { date, tz: _ }) => (*date, None),
        XdmItem::Atomic(XdmAtomicValue::String(s))
        | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => parse_xs_date_local(s)
            .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:date"))?,
        _ => {
            return Err(Error::from_code(
                ErrorCode::XPTY0004,
                "adjust-date-to-timezone expects xs:date?",
            ));
        }
    };
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Date { date, tz: tz_opt })])
}

pub(super) fn adjust_time_to_timezone_fn<
    N: 'static + Send + Sync + crate::model::XdmNode + Clone,
>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    let tz_opt = if args.len() == 1 || args[1].is_empty() {
        Some(ctx.dyn_ctx.timezone_override.unwrap_or_else(|| {
            ctx.dyn_ctx
                .now
                .map(|n| *n.offset())
                .unwrap_or_else(|| chrono::Utc.fix())
        }))
    } else {
        match &args[1][0] {
            XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => {
                ChronoFixedOffset::east_opt(*secs as i32)
                    .ok_or_else(|| Error::from_code(ErrorCode::FORG0001, "invalid timezone"))?
            }
            _ => {
                return Err(Error::from_code(
                    ErrorCode::XPTY0004,
                    "adjust-time-to-timezone expects xs:dayTimeDuration",
                ));
            }
        }
        .into()
    };
    let (time, _tz) = match &args[0][0] {
        XdmItem::Atomic(XdmAtomicValue::Time { time, tz: _ }) => (*time, None),
        XdmItem::Atomic(XdmAtomicValue::String(s))
        | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            crate::util::temporal::parse_time_lex(s)
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:time"))?
        }
        _ => {
            return Err(Error::from_code(
                ErrorCode::XPTY0004,
                "adjust-time-to-timezone expects xs:time?",
            ));
        }
    };
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Time { time, tz: tz_opt })])
}

pub(super) fn adjust_datetime_to_timezone_fn<
    N: 'static + Send + Sync + crate::model::XdmNode + Clone,
>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    let tz_opt = if args.len() == 1 || args[1].is_empty() {
        Some(ctx.dyn_ctx.timezone_override.unwrap_or_else(|| {
            ctx.dyn_ctx
                .now
                .map(|n| *n.offset())
                .unwrap_or_else(|| chrono::Utc.fix())
        }))
    } else {
        Some(match &args[1][0] {
            XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => {
                ChronoFixedOffset::east_opt(*secs as i32)
                    .ok_or_else(|| Error::from_code(ErrorCode::FORG0001, "invalid timezone"))?
            }
            _ => {
                return Err(Error::from_code(
                    ErrorCode::XPTY0004,
                    "adjust-dateTime-to-timezone expects xs:dayTimeDuration",
                ));
            }
        })
    };
    let dt = match &args[0][0] {
        XdmItem::Atomic(XdmAtomicValue::DateTime(dt)) => *dt,
        XdmItem::Atomic(XdmAtomicValue::String(s))
        | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            crate::util::temporal::parse_date_time_lex(s)
                .map(|(d, t, tz)| crate::util::temporal::build_naive_datetime(d, t, tz))
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:dateTime"))?
        }
        _ => {
            return Err(Error::from_code(
                ErrorCode::XPTY0004,
                "adjust-dateTime-to-timezone expects xs:dateTime?",
            ));
        }
    };
    let naive = dt.naive_utc();
    let res = match tz_opt {
        Some(ofs) => ofs.from_utc_datetime(&naive),
        None => chrono::Utc.fix().from_utc_datetime(&naive),
    };
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::DateTime(res))])
}

pub(super) fn current_datetime_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    _args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let dt = now_in_effective_tz(ctx);
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::DateTime(dt))])
}

pub(super) fn current_date_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    _args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let dt = now_in_effective_tz(ctx);
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Date {
        date: dt.date_naive(),
        tz: Some(*dt.offset()),
    })])
}

pub(super) fn current_time_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    _args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let dt = now_in_effective_tz(ctx);
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Time { time: dt.time(), tz: Some(*dt.offset()) })])
}

pub(super) fn implicit_timezone_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    _args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let offset_secs = if let Some(tz) = ctx.dyn_ctx.timezone_override {
        tz.local_minus_utc()
    } else if let Some(n) = ctx.dyn_ctx.now {
        n.offset().local_minus_utc()
    } else {
        0
    };
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(offset_secs as i64))])
}

pub(super) fn year_from_datetime_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    match get_datetime(&args[0])? {
        None => Ok(vec![]),
        Some(dt) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(dt.year() as i64))]),
    }
}

pub(super) fn hours_from_datetime_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    match get_datetime(&args[0])? {
        None => Ok(vec![]),
        Some(dt) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(dt.hour() as i64))]),
    }
}

pub(super) fn minutes_from_datetime_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    match get_datetime(&args[0])? {
        None => Ok(vec![]),
        Some(dt) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(dt.minute() as i64))]),
    }
}

pub(super) fn seconds_from_datetime_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    match get_datetime(&args[0])? {
        None => Ok(vec![]),
        Some(dt) => {
            let secs = dt.second() as f64 + (dt.nanosecond() as f64) / 1_000_000_000.0;
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Decimal(secs))])
        }
    }
}

pub(super) fn month_from_datetime_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    match get_datetime(&args[0])? {
        None => Ok(vec![]),
        Some(dt) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(dt.month() as i64))]),
    }
}

pub(super) fn day_from_datetime_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    match get_datetime(&args[0])? {
        None => Ok(vec![]),
        Some(dt) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(dt.day() as i64))]),
    }
}

pub(super) fn hours_from_time_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    match get_time(&args[0])? {
        None => Ok(vec![]),
        Some((time, _)) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(time.hour() as i64))]),
    }
}

pub(super) fn minutes_from_time_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    match get_time(&args[0])? {
        None => Ok(vec![]),
        Some((time, _)) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(time.minute() as i64))]),
    }
}

pub(super) fn seconds_from_time_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    match get_time(&args[0])? {
        None => Ok(vec![]),
        Some((time, _)) => {
            let secs = time.second() as f64 + (time.nanosecond() as f64) / 1_000_000_000.0;
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Decimal(secs))])
        }
    }
}

pub(super) fn timezone_from_datetime_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    match get_datetime(&args[0])? {
        None => Ok(vec![]),
        Some(dt) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(
            dt.offset().local_minus_utc() as i64,
        ))]),
    }
}

pub(super) fn timezone_from_date_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    match &args[0][0] {
        XdmItem::Atomic(XdmAtomicValue::Date { tz, .. }) => {
            if let Some(off) = tz {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(
                    off.local_minus_utc() as i64
                ))])
            } else {
                Ok(vec![])
            }
        }
        XdmItem::Atomic(XdmAtomicValue::String(s))
        | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            if let Ok((_d, Some(off))) = parse_xs_date_local(s) {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(
                    off.local_minus_utc() as i64
                ))])
            } else {
                Ok(vec![])
            }
        }
        XdmItem::Node(n) => {
            if let Ok((_d, Some(off))) = parse_xs_date_local(&n.string_value()) {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(
                    off.local_minus_utc() as i64
                ))])
            } else {
                Ok(vec![])
            }
        }
        _ => Ok(vec![]),
    }
}

pub(super) fn timezone_from_time_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    match get_time(&args[0])? {
        None => Ok(vec![]),
        Some((_t, Some(off))) => {
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(off.local_minus_utc() as i64))])
        }
        Some((_t, None)) => Ok(vec![]),
    }
}

pub(super) fn year_from_date_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    match &args[0][0] {
        XdmItem::Atomic(XdmAtomicValue::Date { date, .. }) => {
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(date.year() as i64))])
        }
        XdmItem::Atomic(XdmAtomicValue::String(s))
        | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            let (d, _) = parse_xs_date_local(s)
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:date"))?;
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(d.year() as i64))])
        }
        XdmItem::Node(n) => {
            let (d, _) = parse_xs_date_local(&n.string_value())
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:date"))?;
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(d.year() as i64))])
        }
        _ => Ok(vec![]),
    }
}

pub(super) fn month_from_date_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    match &args[0][0] {
        XdmItem::Atomic(XdmAtomicValue::Date { date, .. }) => {
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(date.month() as i64))])
        }
        XdmItem::Atomic(XdmAtomicValue::String(s))
        | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            let (d, _) = parse_xs_date_local(s)
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:date"))?;
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(d.month() as i64))])
        }
        XdmItem::Node(n) => {
            let (d, _) = parse_xs_date_local(&n.string_value())
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:date"))?;
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(d.month() as i64))])
        }
        _ => Ok(vec![]),
    }
}

pub(super) fn day_from_date_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    match &args[0][0] {
        XdmItem::Atomic(XdmAtomicValue::Date { date, .. }) => {
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(date.day() as i64))])
        }
        XdmItem::Atomic(XdmAtomicValue::String(s))
        | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            let (d, _) = parse_xs_date_local(s)
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:date"))?;
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(d.day() as i64))])
        }
        XdmItem::Node(n) => {
            let (d, _) = parse_xs_date_local(&n.string_value())
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:date"))?;
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(d.day() as i64))])
        }
        _ => Ok(vec![]),
    }
}
