use super::common::{get_datetime, get_time, now_in_effective_tz, parse_xs_date_local};
use crate::engine::runtime::{CallCtx, Error, ErrorCode};
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence, XdmSequenceStream};
use chrono::{Datelike, FixedOffset as ChronoFixedOffset, Offset, TimeZone, Timelike};

pub(super) fn date_time_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq0: XdmSequence<N> = args[0].materialize()?;
    let seq1: XdmSequence<N> = args[1].materialize()?;
    if seq0.is_empty() || seq1.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    let (date, tz_date_opt) = match &seq0[0] {
        XdmItem::Atomic(XdmAtomicValue::Date { date, tz }) => (*date, *tz),
        XdmItem::Atomic(XdmAtomicValue::String(s)) | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            let (d, tzo) =
                parse_xs_date_local(s).map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:date"))?;
            (d, tzo)
        }
        _ => {
            return Err(Error::from_code(ErrorCode::XPTY0004, "dateTime expects xs:date? and xs:time?"));
        }
    };
    let (time, tz_time_opt) = match &seq1[0] {
        XdmItem::Atomic(XdmAtomicValue::Time { time, tz }) => (*time, *tz),
        XdmItem::Atomic(XdmAtomicValue::String(s)) | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            let (t, tzo) = crate::util::temporal::parse_time_lex(s)
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:time"))?;
            (t, tzo)
        }
        _ => {
            return Err(Error::from_code(ErrorCode::XPTY0004, "dateTime expects xs:date? and xs:time?"));
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
    let result = vec![XdmItem::Atomic(XdmAtomicValue::DateTime(dt))];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn adjust_date_to_timezone_fn<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    let tz_opt = if args.len() == 1 || args[1].is_empty() {
        Some(
            ctx.dyn_ctx
                .timezone_override
                .unwrap_or_else(|| ctx.dyn_ctx.now.map(|n| *n.offset()).unwrap_or_else(|| chrono::Utc.fix())),
        )
    } else {
        match &args[1][0] {
            XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => ChronoFixedOffset::east_opt(*secs as i32)
                .ok_or_else(|| Error::from_code(ErrorCode::FORG0001, "invalid timezone"))?,
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
        XdmItem::Atomic(XdmAtomicValue::String(s)) | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            parse_xs_date_local(s).map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:date"))?
        }
        _ => {
            return Err(Error::from_code(ErrorCode::XPTY0004, "adjust-date-to-timezone expects xs:date?"));
        }
    };
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Date { date, tz: tz_opt })])
}

pub(super) fn adjust_date_to_timezone_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq0 = args[0].materialize()?;
    let seq1 = if args.len() >= 2 { args[1].materialize()? } else { vec![] };
    let materialized_args = vec![seq0, seq1];
    let result = adjust_date_to_timezone_fn(ctx, &materialized_args)?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn adjust_time_to_timezone_fn<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    let tz_opt = if args.len() == 1 || args[1].is_empty() {
        Some(
            ctx.dyn_ctx
                .timezone_override
                .unwrap_or_else(|| ctx.dyn_ctx.now.map(|n| *n.offset()).unwrap_or_else(|| chrono::Utc.fix())),
        )
    } else {
        match &args[1][0] {
            XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => ChronoFixedOffset::east_opt(*secs as i32)
                .ok_or_else(|| Error::from_code(ErrorCode::FORG0001, "invalid timezone"))?,
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
        XdmItem::Atomic(XdmAtomicValue::String(s)) | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            crate::util::temporal::parse_time_lex(s)
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:time"))?
        }
        _ => {
            return Err(Error::from_code(ErrorCode::XPTY0004, "adjust-time-to-timezone expects xs:time?"));
        }
    };
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Time { time, tz: tz_opt })])
}

pub(super) fn adjust_time_to_timezone_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq0 = args[0].materialize()?;
    let seq1 = if args.len() >= 2 { args[1].materialize()? } else { vec![] };
    let materialized_args = vec![seq0, seq1];
    let result = adjust_time_to_timezone_fn(ctx, &materialized_args)?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn adjust_datetime_to_timezone_fn<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    let tz_opt = if args.len() == 1 || args[1].is_empty() {
        Some(
            ctx.dyn_ctx
                .timezone_override
                .unwrap_or_else(|| ctx.dyn_ctx.now.map(|n| *n.offset()).unwrap_or_else(|| chrono::Utc.fix())),
        )
    } else {
        Some(match &args[1][0] {
            XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => ChronoFixedOffset::east_opt(*secs as i32)
                .ok_or_else(|| Error::from_code(ErrorCode::FORG0001, "invalid timezone"))?,
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
        XdmItem::Atomic(XdmAtomicValue::String(s)) | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            crate::util::temporal::parse_date_time_lex(s)
                .map(|(d, t, tz)| crate::util::temporal::build_naive_datetime(d, t, tz))
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:dateTime"))?
        }
        _ => {
            return Err(Error::from_code(ErrorCode::XPTY0004, "adjust-dateTime-to-timezone expects xs:dateTime?"));
        }
    };
    let naive = dt.naive_utc();
    let res = match tz_opt {
        Some(ofs) => ofs.from_utc_datetime(&naive),
        None => chrono::Utc.fix().from_utc_datetime(&naive),
    };
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::DateTime(res))])
}

pub(super) fn adjust_datetime_to_timezone_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq0 = args[0].materialize()?;
    let seq1 = if args.len() >= 2 { args[1].materialize()? } else { vec![] };
    let materialized_args = vec![seq0, seq1];
    let result = adjust_datetime_to_timezone_fn(ctx, &materialized_args)?;
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based current-dateTime() implementation.
pub(super) fn current_datetime_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    _args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let dt = now_in_effective_tz(ctx);
    let result = vec![XdmItem::Atomic(XdmAtomicValue::DateTime(dt))];
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based current-date() implementation.
pub(super) fn current_date_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    _args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let dt = now_in_effective_tz(ctx);
    let result = vec![XdmItem::Atomic(XdmAtomicValue::Date { date: dt.date_naive(), tz: Some(*dt.offset()) })];
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based current-time() implementation.
pub(super) fn current_time_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    _args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let dt = now_in_effective_tz(ctx);
    let result = vec![XdmItem::Atomic(XdmAtomicValue::Time { time: dt.time(), tz: Some(*dt.offset()) })];
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based implicit-timezone() implementation.
pub(super) fn implicit_timezone_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    _args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let offset_secs = if let Some(tz) = ctx.dyn_ctx.timezone_override {
        tz.local_minus_utc()
    } else if let Some(n) = ctx.dyn_ctx.now {
        n.offset().local_minus_utc()
    } else {
        0
    };
    let result = vec![XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(offset_secs as i64))];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn year_from_datetime_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = match get_datetime(&seq)? {
        None => vec![],
        Some(dt) => vec![XdmItem::Atomic(XdmAtomicValue::Integer(dt.year() as i64))],
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn hours_from_datetime_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = match get_datetime(&seq)? {
        None => vec![],
        Some(dt) => vec![XdmItem::Atomic(XdmAtomicValue::Integer(dt.hour() as i64))],
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn minutes_from_datetime_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = match get_datetime(&seq)? {
        None => vec![],
        Some(dt) => vec![XdmItem::Atomic(XdmAtomicValue::Integer(dt.minute() as i64))],
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn seconds_from_datetime_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = match get_datetime(&seq)? {
        None => vec![],
        Some(dt) => {
            let secs = dt.second() as f64 + (dt.nanosecond() as f64) / 1_000_000_000.0;
            vec![XdmItem::Atomic(XdmAtomicValue::Decimal(secs))]
        }
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn month_from_datetime_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = match get_datetime(&seq)? {
        None => vec![],
        Some(dt) => vec![XdmItem::Atomic(XdmAtomicValue::Integer(dt.month() as i64))],
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn day_from_datetime_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = match get_datetime(&seq)? {
        None => vec![],
        Some(dt) => vec![XdmItem::Atomic(XdmAtomicValue::Integer(dt.day() as i64))],
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn hours_from_time_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = match get_time(&seq)? {
        None => vec![],
        Some((time, _)) => vec![XdmItem::Atomic(XdmAtomicValue::Integer(time.hour() as i64))],
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn minutes_from_time_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = match get_time(&seq)? {
        None => vec![],
        Some((time, _)) => vec![XdmItem::Atomic(XdmAtomicValue::Integer(time.minute() as i64))],
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn seconds_from_time_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = match get_time(&seq)? {
        None => vec![],
        Some((time, _)) => {
            let secs = time.second() as f64 + (time.nanosecond() as f64) / 1_000_000_000.0;
            vec![XdmItem::Atomic(XdmAtomicValue::Decimal(secs))]
        }
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn timezone_from_datetime_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = match get_datetime(&seq)? {
        None => vec![],
        Some(dt) => vec![XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(dt.offset().local_minus_utc() as i64))],
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn timezone_from_date_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    let result = match &seq[0] {
        XdmItem::Atomic(XdmAtomicValue::Date { tz, .. }) => {
            if let Some(off) = tz {
                vec![XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(off.local_minus_utc() as i64))]
            } else {
                vec![]
            }
        }
        XdmItem::Atomic(XdmAtomicValue::String(s)) | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            if let Ok((_d, Some(off))) = parse_xs_date_local(s) {
                vec![XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(off.local_minus_utc() as i64))]
            } else {
                vec![]
            }
        }
        XdmItem::Node(n) => {
            if let Ok((_d, Some(off))) = parse_xs_date_local(&n.string_value()) {
                vec![XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(off.local_minus_utc() as i64))]
            } else {
                vec![]
            }
        }
        _ => vec![],
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn timezone_from_time_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = match get_time(&seq)? {
        None => vec![],
        Some((_t, Some(off))) => {
            vec![XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(off.local_minus_utc() as i64))]
        }
        Some((_t, None)) => vec![],
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn year_from_date_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    let result = match &seq[0] {
        XdmItem::Atomic(XdmAtomicValue::Date { date, .. }) => {
            vec![XdmItem::Atomic(XdmAtomicValue::Integer(date.year() as i64))]
        }
        XdmItem::Atomic(XdmAtomicValue::String(s)) | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            let (d, _) =
                parse_xs_date_local(s).map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:date"))?;
            vec![XdmItem::Atomic(XdmAtomicValue::Integer(d.year() as i64))]
        }
        XdmItem::Node(n) => {
            let (d, _) = parse_xs_date_local(&n.string_value())
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:date"))?;
            vec![XdmItem::Atomic(XdmAtomicValue::Integer(d.year() as i64))]
        }
        _ => vec![],
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn month_from_date_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    let result = match &seq[0] {
        XdmItem::Atomic(XdmAtomicValue::Date { date, .. }) => {
            vec![XdmItem::Atomic(XdmAtomicValue::Integer(date.month() as i64))]
        }
        XdmItem::Atomic(XdmAtomicValue::String(s)) | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            let (d, _) =
                parse_xs_date_local(s).map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:date"))?;
            vec![XdmItem::Atomic(XdmAtomicValue::Integer(d.month() as i64))]
        }
        XdmItem::Node(n) => {
            let (d, _) = parse_xs_date_local(&n.string_value())
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:date"))?;
            vec![XdmItem::Atomic(XdmAtomicValue::Integer(d.month() as i64))]
        }
        _ => vec![],
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn day_from_date_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    let result = match &seq[0] {
        XdmItem::Atomic(XdmAtomicValue::Date { date, .. }) => {
            vec![XdmItem::Atomic(XdmAtomicValue::Integer(date.day() as i64))]
        }
        XdmItem::Atomic(XdmAtomicValue::String(s)) | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            let (d, _) =
                parse_xs_date_local(s).map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:date"))?;
            vec![XdmItem::Atomic(XdmAtomicValue::Integer(d.day() as i64))]
        }
        XdmItem::Node(n) => {
            let (d, _) = parse_xs_date_local(&n.string_value())
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:date"))?;
            vec![XdmItem::Atomic(XdmAtomicValue::Integer(d.day() as i64))]
        }
        _ => vec![],
    };
    Ok(XdmSequenceStream::from_vec(result))
}
