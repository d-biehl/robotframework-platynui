use super::common::{
    NumericKind, a_as_i128, classify_numeric, minmax_impl, num_unary, number_default, round_default,
    round_half_to_even_default, sum_default,
};
use crate::engine::runtime::{CallCtx, Error, ErrorCode};
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence, XdmSequenceStream};

/// Stream-based number() implementation.
///
/// Handles both 0-arity (uses context item) and 1-arity versions.
pub(super) fn number_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let result = if args.is_empty() {
        number_default(ctx, None)?
    } else {
        let seq = args[0].materialize()?;
        number_default(ctx, Some(&seq))?
    };
    Ok(XdmSequenceStream::from_vec(result))
}
/// Stream-based abs() implementation.
pub(super) fn abs_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq = args[0].materialize()?;
    let result = num_unary(&[seq], |n| n.abs());
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based floor() implementation.
pub(super) fn floor_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq = args[0].materialize()?;
    let result = num_unary(&[seq], |n| n.floor());
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based ceiling() implementation.
pub(super) fn ceiling_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq = args[0].materialize()?;
    let result = num_unary(&[seq], |n| n.ceil());
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based round() implementation.
///
/// Handles both 1-arity and 2-arity versions.
pub(super) fn round_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq0 = args[0].materialize()?;
    let result = if args.len() == 1 {
        round_default(&seq0, None)?
    } else {
        let seq1 = args[1].materialize()?;
        round_default(&seq0, Some(&seq1))?
    };
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based round-half-to-even() implementation.
///
/// Handles both 1-arity and 2-arity versions.
pub(super) fn round_half_to_even_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq0 = args[0].materialize()?;
    let result = if args.len() == 1 {
        round_half_to_even_default(&seq0, None)?
    } else {
        let seq1 = args[1].materialize()?;
        round_half_to_even_default(&seq0, Some(&seq1))?
    };
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based avg() implementation.
///
/// Materializes input and performs average calculation.
pub(super) fn avg_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq = args[0].materialize()?;

    if seq.is_empty() {
        return Ok(XdmSequenceStream::empty());
    }

    enum AvgState {
        Numeric,
        YearMonth,
        DayTime,
    }
    let mut state: Option<AvgState> = None;
    let mut kind = NumericKind::Integer;
    let mut int_acc: i128 = 0;
    let mut dec_acc: f64 = 0.0;
    let mut use_int_acc = true;
    let mut ym_total: i64 = 0;
    let mut dt_total: i128 = 0;
    let mut count: i64 = 0;

    for it in &seq {
        let XdmItem::Atomic(a) = it else {
            return Err(Error::from_code(ErrorCode::XPTY0004, "avg on non-atomic item"));
        };
        match a {
            XdmAtomicValue::YearMonthDuration(months) => {
                state = match state {
                    None => {
                        ym_total = *months as i64;
                        Some(AvgState::YearMonth)
                    }
                    Some(AvgState::YearMonth) => {
                        ym_total = ym_total
                            .checked_add(*months as i64)
                            .ok_or_else(|| Error::from_code(ErrorCode::FOAR0002, "yearMonthDuration overflow"))?;
                        Some(AvgState::YearMonth)
                    }
                    _ => {
                        return Err(Error::from_code(ErrorCode::XPTY0004, "avg requires values of a single type"));
                    }
                };
            }
            XdmAtomicValue::DayTimeDuration(secs) => {
                state = match state {
                    None => {
                        dt_total = *secs as i128;
                        Some(AvgState::DayTime)
                    }
                    Some(AvgState::DayTime) => {
                        dt_total = dt_total
                            .checked_add(*secs as i128)
                            .ok_or_else(|| Error::from_code(ErrorCode::FOAR0002, "dayTimeDuration overflow"))?;
                        Some(AvgState::DayTime)
                    }
                    _ => {
                        return Err(Error::from_code(ErrorCode::XPTY0004, "avg requires values of a single type"));
                    }
                };
            }
            _ => {
                if let Some((nk, num)) = classify_numeric(a)? {
                    if nk == NumericKind::Double && num.is_nan() {
                        return Ok(XdmSequenceStream::from_item(XdmItem::Atomic(XdmAtomicValue::Double(f64::NAN))));
                    }
                    state = match state {
                        None => Some(AvgState::Numeric),
                        Some(AvgState::Numeric) => Some(AvgState::Numeric),
                        _ => {
                            return Err(Error::from_code(ErrorCode::XPTY0004, "avg requires values of a single type"));
                        }
                    };
                    kind = kind.promote(nk);
                    match nk {
                        NumericKind::Integer if use_int_acc => {
                            if let Some(i) = a_as_i128(a) {
                                if let Some(v) = int_acc.checked_add(i) {
                                    int_acc = v;
                                } else {
                                    use_int_acc = false;
                                    dec_acc = int_acc as f64 + i as f64;
                                    kind = kind.promote(NumericKind::Decimal);
                                }
                            }
                        }
                        _ => {
                            if use_int_acc {
                                dec_acc = int_acc as f64;
                                use_int_acc = false;
                            }
                            dec_acc += num;
                        }
                    }
                } else {
                    return Err(Error::from_code(ErrorCode::XPTY0004, "avg requires numeric or duration values"));
                }
            }
        }
        count += 1;
    }

    if count == 0 {
        return Ok(XdmSequenceStream::empty());
    }

    let out = match state.unwrap_or(AvgState::Numeric) {
        AvgState::Numeric => {
            let total = if use_int_acc && matches!(kind, NumericKind::Integer) { int_acc as f64 } else { dec_acc };
            let mean = total / (count as f64);
            match kind {
                NumericKind::Integer | NumericKind::Decimal => XdmAtomicValue::Decimal(mean),
                NumericKind::Float => XdmAtomicValue::Float(mean as f32),
                NumericKind::Double => XdmAtomicValue::Double(mean),
            }
        }
        AvgState::YearMonth => {
            if ym_total % count != 0 {
                return Err(Error::from_code(ErrorCode::FOAR0002, "average yearMonthDuration is not integral months"));
            }
            let months: i32 = (ym_total / count)
                .try_into()
                .map_err(|_| Error::from_code(ErrorCode::FOAR0002, "yearMonthDuration overflow"))?;
            XdmAtomicValue::YearMonthDuration(months)
        }
        AvgState::DayTime => {
            if dt_total % (count as i128) != 0 {
                return Err(Error::from_code(ErrorCode::FOAR0002, "average dayTimeDuration has fractional seconds"));
            }
            let secs: i64 = (dt_total / (count as i128))
                .try_into()
                .map_err(|_| Error::from_code(ErrorCode::FOAR0002, "dayTimeDuration overflow"))?;
            XdmAtomicValue::DayTimeDuration(secs)
        }
    };

    Ok(XdmSequenceStream::from_item(XdmItem::Atomic(out)))
}

//---- Stream-based aggregate functions ----

/// Stream-based sum() implementation.
///
/// Materializes the input stream and delegates to existing sum_default().
/// Performance: O(n) iteration with accumulation logic.
pub(super) fn sum_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq = args[0].materialize()?;

    let result = if args.len() == 1 {
        sum_default(&seq, None)?
    } else {
        let zero_seq = args[1].materialize()?;
        sum_default(&seq, Some(&zero_seq))?
    };

    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based min() implementation.
///
/// Materializes input and delegates to minmax_impl() with is_min=true.
pub(super) fn min_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;

    if seq.is_empty() {
        return Ok(XdmSequenceStream::empty());
    }

    let result = if args.len() == 1 {
        minmax_impl(ctx, &seq, None, true)?
    } else {
        let collation_seq: XdmSequence<N> = args[1].materialize()?;
        let uri = super::common::item_to_string(&collation_seq);
        let k = crate::engine::collation::resolve_collation(ctx.dyn_ctx, ctx.default_collation.as_ref(), Some(&uri))?;
        minmax_impl(ctx, &seq, Some(k.as_trait()), true)?
    };

    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based max() implementation.
///
/// Materializes input and delegates to minmax_impl() with is_min=false.
pub(super) fn max_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;

    if seq.is_empty() {
        return Ok(XdmSequenceStream::empty());
    }

    let result = if args.len() == 1 {
        minmax_impl(ctx, &seq, None, false)?
    } else {
        let collation_seq: XdmSequence<N> = args[1].materialize()?;
        let uri = super::common::item_to_string(&collation_seq);
        let k = crate::engine::collation::resolve_collation(ctx.dyn_ctx, ctx.default_collation.as_ref(), Some(&uri))?;
        minmax_impl(ctx, &seq, Some(k.as_trait()), false)?
    };

    Ok(XdmSequenceStream::from_vec(result))
}
