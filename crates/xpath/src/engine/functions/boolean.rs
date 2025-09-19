use super::common::{data_default, ebv};
use crate::engine::runtime::{CallCtx, Error};
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence};

pub(super) fn fn_true<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    _args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(true))])
}

pub(super) fn fn_false<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    _args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(false))])
}

pub(super) fn data_fn<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    match args.len() {
        0 => data_default(ctx, None),
        1 => data_default(ctx, Some(&args[0])),
        _ => unreachable!("registry guarantees arity in range"),
    }
}

pub(super) fn fn_not<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let b = ebv(&args[0])?;
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(!b))])
}

pub(super) fn fn_boolean<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let b = ebv(&args[0])?;
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))])
}
