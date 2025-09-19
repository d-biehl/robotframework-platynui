use super::common::{distinct_values_impl, index_of_default, subsequence_default, to_number};
use crate::engine::runtime::{CallCtx, Error, ErrorCode};
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence};

pub(super) fn empty_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(
        args[0].is_empty(),
    ))])
}

pub(super) fn exists_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(
        !args[0].is_empty(),
    ))])
}

pub(super) fn count_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
        args[0].len() as i64,
    ))])
}

pub(super) fn exactly_one_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].len() != 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0005,
            "exactly-one requires a sequence of length 1",
        ));
    }
    Ok(args[0].clone())
}

pub(super) fn one_or_more_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Err(Error::from_code(
            ErrorCode::FORG0004,
            "one-or-more requires at least one item",
        ));
    }
    Ok(args[0].clone())
}

pub(super) fn zero_or_one_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0004,
            "zero-or-one requires at most one item",
        ));
    }
    Ok(args[0].clone())
}

pub(super) fn reverse_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let s: XdmSequence<N> = args[0].iter().cloned().rev().collect();
    Ok(s)
}

pub(super) fn subsequence_fn<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let start_raw = to_number(&args[1])?;
    if args.len() == 2 {
        subsequence_default(&args[0], start_raw, None)
    } else {
        let len_raw = to_number(&args[2])?;
        subsequence_default(&args[0], start_raw, Some(len_raw))
    }
}

pub(super) fn distinct_values_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args.len() == 1 {
        distinct_values_impl(ctx, &args[0], None)
    } else {
        let uri = super::common::item_to_string(&args[1]);
        let k = crate::engine::collation::resolve_collation(
            ctx.dyn_ctx,
            ctx.default_collation.as_ref(),
            Some(&uri),
        )?;
        distinct_values_impl(ctx, &args[0], Some(k.as_trait()))
    }
}

pub(super) fn index_of_fn<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args.len() == 2 {
        index_of_default(ctx, &args[0], &args[1], None)
    } else {
        let uri = super::common::item_to_string(&args[2]);
        index_of_default(
            ctx,
            &args[0],
            &args[1],
            if uri.is_empty() { None } else { Some(&uri) },
        )
    }
}

pub(super) fn insert_before_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let mut out: XdmSequence<N> = Vec::new();
    let pos = to_number(&args[1])?.floor() as isize;
    let insert_at = pos.max(1) as usize;
    let mut i = 1usize;
    for it in &args[0] {
        if i == insert_at {
            out.extend(args[2].iter().cloned());
        }
        out.push(it.clone());
        i += 1;
    }
    if insert_at > args[0].len() {
        out.extend(args[2].iter().cloned());
    }
    Ok(out)
}

pub(super) fn remove_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let mut out: XdmSequence<N> = Vec::new();
    let pos = to_number(&args[1])?.floor() as isize;
    let remove_at = pos.max(1) as usize;
    for (i, it) in args[0].iter().enumerate() {
        if i + 1 != remove_at {
            out.push(it.clone());
        }
    }
    Ok(out)
}

pub(super) fn unordered_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    Ok(args[0].clone())
}
