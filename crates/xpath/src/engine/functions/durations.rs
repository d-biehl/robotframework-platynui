use crate::engine::runtime::{CallCtx, Error, ErrorCode};
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequenceStream};

pub(super) fn years_from_duration_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    let result = match &seq[0] {
        XdmItem::Atomic(XdmAtomicValue::YearMonthDuration(months)) => {
            vec![XdmItem::Atomic(XdmAtomicValue::Integer((*months / 12) as i64))]
        }
        XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(_)) => {
            vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))]
        }
        _ => {
            return Err(Error::from_code(ErrorCode::XPTY0004, "years-from-duration expects xs:duration"));
        }
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn months_from_duration_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    let result = match &seq[0] {
        XdmItem::Atomic(XdmAtomicValue::YearMonthDuration(months)) => {
            vec![XdmItem::Atomic(XdmAtomicValue::Integer((*months % 12) as i64))]
        }
        XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(_)) => {
            vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))]
        }
        _ => {
            return Err(Error::from_code(ErrorCode::XPTY0004, "months-from-duration expects xs:duration"));
        }
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn days_from_duration_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    let result = match &seq[0] {
        XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => {
            vec![XdmItem::Atomic(XdmAtomicValue::Integer(*secs / (24 * 3600)))]
        }
        XdmItem::Atomic(XdmAtomicValue::YearMonthDuration(_)) => {
            vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))]
        }
        _ => {
            return Err(Error::from_code(ErrorCode::XPTY0004, "days-from-duration expects xs:duration"));
        }
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn hours_from_duration_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    let result = match &seq[0] {
        XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => {
            let rem = *secs % (24 * 3600);
            vec![XdmItem::Atomic(XdmAtomicValue::Integer(rem / 3600))]
        }
        XdmItem::Atomic(XdmAtomicValue::YearMonthDuration(_)) => {
            vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))]
        }
        _ => {
            return Err(Error::from_code(ErrorCode::XPTY0004, "hours-from-duration expects xs:duration"));
        }
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn minutes_from_duration_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    let result = match &seq[0] {
        XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => {
            let rem = *secs % 3600;
            vec![XdmItem::Atomic(XdmAtomicValue::Integer(rem / 60))]
        }
        XdmItem::Atomic(XdmAtomicValue::YearMonthDuration(_)) => {
            vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))]
        }
        _ => {
            return Err(Error::from_code(ErrorCode::XPTY0004, "minutes-from-duration expects xs:duration"));
        }
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn seconds_from_duration_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    let result = match &seq[0] {
        XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => {
            let rem = *secs % 60;
            vec![XdmItem::Atomic(XdmAtomicValue::Decimal(rem as f64))]
        }
        XdmItem::Atomic(XdmAtomicValue::YearMonthDuration(_)) => {
            vec![XdmItem::Atomic(XdmAtomicValue::Decimal(0.0))]
        }
        _ => {
            return Err(Error::from_code(ErrorCode::XPTY0004, "seconds-from-duration expects xs:duration"));
        }
    };
    Ok(XdmSequenceStream::from_vec(result))
}
