use crate::engine::runtime::{CallCtx, Error, ErrorCode};
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence};

pub(super) fn years_from_duration_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    match &args[0][0] {
        XdmItem::Atomic(XdmAtomicValue::YearMonthDuration(months)) => Ok(vec![XdmItem::Atomic(
            XdmAtomicValue::Integer((*months / 12) as i64),
        )]),
        XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(_)) => {
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
        }
        _ => Err(Error::from_code(
            ErrorCode::XPTY0004,
            "years-from-duration expects xs:duration",
        )),
    }
}

pub(super) fn months_from_duration_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    match &args[0][0] {
        XdmItem::Atomic(XdmAtomicValue::YearMonthDuration(months)) => Ok(vec![XdmItem::Atomic(
            XdmAtomicValue::Integer((*months % 12) as i64),
        )]),
        XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(_)) => {
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
        }
        _ => Err(Error::from_code(
            ErrorCode::XPTY0004,
            "months-from-duration expects xs:duration",
        )),
    }
}

pub(super) fn days_from_duration_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    match &args[0][0] {
        XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => Ok(vec![XdmItem::Atomic(
            XdmAtomicValue::Integer(*secs / (24 * 3600)),
        )]),
        XdmItem::Atomic(XdmAtomicValue::YearMonthDuration(_)) => {
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
        }
        _ => Err(Error::from_code(
            ErrorCode::XPTY0004,
            "days-from-duration expects xs:duration",
        )),
    }
}

pub(super) fn hours_from_duration_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    match &args[0][0] {
        XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => {
            let rem = *secs % (24 * 3600);
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(rem / 3600))])
        }
        XdmItem::Atomic(XdmAtomicValue::YearMonthDuration(_)) => {
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
        }
        _ => Err(Error::from_code(
            ErrorCode::XPTY0004,
            "hours-from-duration expects xs:duration",
        )),
    }
}

pub(super) fn minutes_from_duration_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    match &args[0][0] {
        XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => {
            let rem = *secs % 3600;
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(rem / 60))])
        }
        XdmItem::Atomic(XdmAtomicValue::YearMonthDuration(_)) => {
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
        }
        _ => Err(Error::from_code(
            ErrorCode::XPTY0004,
            "minutes-from-duration expects xs:duration",
        )),
    }
}

pub(super) fn seconds_from_duration_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    match &args[0][0] {
        XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => {
            let rem = *secs % 60;
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Decimal(rem as f64))])
        }
        XdmItem::Atomic(XdmAtomicValue::YearMonthDuration(_)) => {
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Decimal(0.0))])
        }
        _ => Err(Error::from_code(
            ErrorCode::XPTY0004,
            "seconds-from-duration expects xs:duration",
        )),
    }
}
