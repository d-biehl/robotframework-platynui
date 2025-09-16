use super::common::{
    NumericKind, a_as_i128, classify_numeric, minmax_impl, num_unary, number_default,
    round_default, round_half_to_even_default, sum_default,
};
use crate::engine::runtime::{CallCtx, Error, ErrorCode};
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence};

pub(super) fn number_fn<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    match args.len() {
        0 => number_default(ctx, None),
        1 => number_default(ctx, Some(&args[0])),
        _ => unreachable!("registry guarantees arity in range"),
    }
}

pub(super) fn abs_fn<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    Ok(num_unary(args, |n| n.abs()))
}

pub(super) fn floor_fn<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    Ok(num_unary(args, |n| n.floor()))
}

pub(super) fn ceiling_fn<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    Ok(num_unary(args, |n| n.ceil()))
}

pub(super) fn round_fn<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    match args.len() {
        1 => round_default(&args[0], None),
        2 => round_default(&args[0], Some(&args[1])),
        _ => unreachable!("registry guarantees arity in range"),
    }
}

pub(super) fn round_half_to_even_fn<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    match args.len() {
        1 => round_half_to_even_default(&args[0], None),
        2 => round_half_to_even_default(&args[0], Some(&args[1])),
        _ => unreachable!("registry guarantees arity in range"),
    }
}

pub(super) fn sum_fn<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    match args.len() {
        1 => sum_default(&args[0], None),
        2 => sum_default(&args[0], Some(&args[1])),
        _ => unreachable!("registry guarantees arity in range"),
    }
}

pub(super) fn avg_fn<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    let mut kind = NumericKind::Integer;
    let mut int_acc: i128 = 0;
    let mut dec_acc: f64 = 0.0;
    let mut use_int_acc = true;
    let mut count: i64 = 0;
    for it in &args[0] {
        let XdmItem::Atomic(a) = it else {
            return Err(Error::from_code(
                ErrorCode::XPTY0004,
                "avg on non-atomic item",
            ));
        };
        if let Some((nk, num)) = classify_numeric(a)? {
            if nk == NumericKind::Double && num.is_nan() {
                return Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(f64::NAN))]);
            }
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
            count += 1;
        } else {
            return Err(Error::from_code(
                ErrorCode::XPTY0004,
                "avg requires numeric values",
            ));
        }
    }
    if count == 0 {
        return Ok(vec![]);
    }
    let total = if use_int_acc && matches!(kind, NumericKind::Integer) {
        int_acc as f64
    } else {
        dec_acc
    };
    let mean = total / (count as f64);
    let out = match kind {
        NumericKind::Integer | NumericKind::Decimal => XdmAtomicValue::Decimal(mean),
        NumericKind::Float => XdmAtomicValue::Float(mean as f32),
        NumericKind::Double => XdmAtomicValue::Double(mean),
    };
    Ok(vec![XdmItem::Atomic(out)])
}

pub(super) fn min_fn<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args.len() == 1 {
        minmax_impl(ctx, &args[0], None, true)
    } else {
        let uri = super::common::item_to_string(&args[1]);
        let k = crate::engine::collation::resolve_collation(
            ctx.dyn_ctx,
            ctx.default_collation.as_ref(),
            Some(&uri),
        )?;
        minmax_impl(ctx, &args[0], Some(k.as_trait()), true)
    }
}

pub(super) fn max_fn<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args.len() == 1 {
        minmax_impl(ctx, &args[0], None, false)
    } else {
        let uri = super::common::item_to_string(&args[1]);
        let k = crate::engine::collation::resolve_collation(
            ctx.dyn_ctx,
            ctx.default_collation.as_ref(),
            Some(&uri),
        )?;
        minmax_impl(ctx, &args[0], Some(k.as_trait()), false)
    }
}
