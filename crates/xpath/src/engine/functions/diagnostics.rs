use super::common::error_default;
use crate::engine::runtime::{CallCtx, Error};
use crate::xdm::XdmSequenceStream;

pub(super) fn error_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let materialized_args: Result<Vec<_>, _> = args.iter().map(|stream| stream.materialize()).collect();
    error_default(&materialized_args?)?;
    unreachable!("error_default always returns Err")
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
