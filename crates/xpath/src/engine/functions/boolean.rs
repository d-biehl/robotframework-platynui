use super::common::{data_default, ebv};
use crate::engine::runtime::{CallCtx, Error};
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence, XdmSequenceStream};

/// Stream-based data() implementation.
/// Handles both 0-arity (uses context item) and 1-arity versions.
pub(super) fn data_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let result = if args.is_empty() {
        data_default(ctx, None)?
    } else {
        let seq = args[0].materialize()?;
        data_default(ctx, Some(&seq))?
    };
    Ok(XdmSequenceStream::from_vec(result))
}

//---- Stream-based boolean functions ----

/// Stream-based true() - no materialization needed, constant result.
pub(super) fn fn_true_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    _args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    Ok(XdmSequenceStream::from_item(XdmItem::Atomic(XdmAtomicValue::Boolean(true))))
}

/// Stream-based false() - no materialization needed, constant result.
pub(super) fn fn_false_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    _args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    Ok(XdmSequenceStream::from_item(XdmItem::Atomic(XdmAtomicValue::Boolean(false))))
}

/// Stream-based not() - materializes input for EBV calculation.
pub(super) fn fn_not_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let b = ebv(&seq)?;
    Ok(XdmSequenceStream::from_item(XdmItem::Atomic(XdmAtomicValue::Boolean(!b))))
}

/// Stream-based boolean() implementation.
pub(super) fn fn_boolean_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let b = ebv(&seq)?;
    Ok(XdmSequenceStream::from_vec(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]))
}
