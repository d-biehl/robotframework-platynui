use super::common::error_default;
use crate::engine::runtime::{CallCtx, Error};
use crate::xdm::XdmSequence;

pub(super) fn error_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    error_default(args)
}

pub(super) fn trace_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    Ok(args[0].clone())
}
