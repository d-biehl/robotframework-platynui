use super::common::error_default;
use crate::engine::runtime::{CallCtx, Error};
use crate::xdm::{XdmSequence, XdmSequenceStream};

pub(super) fn error_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    error_default(args)
}

pub(super) fn trace_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq0 = args[0].materialize()?;
    let _seq1 = args[1].materialize()?;
    let result = seq0;
    Ok(XdmSequenceStream::from_vec(result))
}
