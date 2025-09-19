use super::common::parse_duration_lexical;
use crate::engine::runtime::{CallCtx, Error};
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
        XdmItem::Atomic(XdmAtomicValue::String(s))
        | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            let (m_opt, s_opt) = parse_duration_lexical(s)?;
            if let Some(m) = m_opt {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                    (m / 12) as i64,
                ))])
            } else if s_opt.is_some() {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
            } else {
                Ok(vec![])
            }
        }
        XdmItem::Node(n) => {
            let (m_opt, s_opt) = parse_duration_lexical(&n.string_value())?;
            if let Some(m) = m_opt {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                    (m / 12) as i64,
                ))])
            } else if s_opt.is_some() {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
            } else {
                Ok(vec![])
            }
        }
        _ => Ok(vec![]),
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
        XdmItem::Atomic(XdmAtomicValue::String(s))
        | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            let (m_opt, s_opt) = parse_duration_lexical(s)?;
            if let Some(m) = m_opt {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                    (m % 12) as i64,
                ))])
            } else if s_opt.is_some() {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
            } else {
                Ok(vec![])
            }
        }
        XdmItem::Node(n) => {
            let (m_opt, s_opt) = parse_duration_lexical(&n.string_value())?;
            if let Some(m) = m_opt {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                    (m % 12) as i64,
                ))])
            } else if s_opt.is_some() {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
            } else {
                Ok(vec![])
            }
        }
        _ => Ok(vec![]),
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
        XdmItem::Atomic(XdmAtomicValue::String(s))
        | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            let (m_opt, s_opt) = parse_duration_lexical(s)?;
            if let Some(sec) = s_opt {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                    sec / (24 * 3600),
                ))])
            } else if m_opt.is_some() {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
            } else {
                Ok(vec![])
            }
        }
        XdmItem::Node(n) => {
            let (m_opt, s_opt) = parse_duration_lexical(&n.string_value())?;
            if let Some(sec) = s_opt {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                    sec / (24 * 3600),
                ))])
            } else if m_opt.is_some() {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
            } else {
                Ok(vec![])
            }
        }
        _ => Ok(vec![]),
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
        XdmItem::Atomic(XdmAtomicValue::String(s))
        | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            let (m_opt, s_opt) = parse_duration_lexical(s)?;
            if let Some(sec) = s_opt {
                let rem = sec % (24 * 3600);
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(rem / 3600))])
            } else if m_opt.is_some() {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
            } else {
                Ok(vec![])
            }
        }
        XdmItem::Node(n) => {
            let (m_opt, s_opt) = parse_duration_lexical(&n.string_value())?;
            if let Some(sec) = s_opt {
                let rem = sec % (24 * 3600);
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(rem / 3600))])
            } else if m_opt.is_some() {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
            } else {
                Ok(vec![])
            }
        }
        _ => Ok(vec![]),
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
        XdmItem::Atomic(XdmAtomicValue::String(s))
        | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            let (m_opt, s_opt) = parse_duration_lexical(s)?;
            if let Some(sec) = s_opt {
                let rem = sec % 3600;
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(rem / 60))])
            } else if m_opt.is_some() {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
            } else {
                Ok(vec![])
            }
        }
        XdmItem::Node(n) => {
            let (m_opt, s_opt) = parse_duration_lexical(&n.string_value())?;
            if let Some(sec) = s_opt {
                let rem = sec % 3600;
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(rem / 60))])
            } else if m_opt.is_some() {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
            } else {
                Ok(vec![])
            }
        }
        _ => Ok(vec![]),
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
        XdmItem::Atomic(XdmAtomicValue::String(s))
        | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            let (m_opt, s_opt) = parse_duration_lexical(s)?;
            if let Some(sec) = s_opt {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Decimal(
                    (sec % 60) as f64,
                ))])
            } else if m_opt.is_some() {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Decimal(0.0))])
            } else {
                Ok(vec![])
            }
        }
        XdmItem::Node(n) => {
            let (m_opt, s_opt) = parse_duration_lexical(&n.string_value())?;
            if let Some(sec) = s_opt {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Decimal(
                    (sec % 60) as f64,
                ))])
            } else if m_opt.is_some() {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Decimal(0.0))])
            } else {
                Ok(vec![])
            }
        }
        _ => Ok(vec![]),
    }
}
