use super::common::{error_default, item_to_string};
use crate::engine::runtime::{CallCtx, Error};
use crate::xdm::XdmSequenceStream;
use tracing::debug;

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
    let seq1 = args[1].materialize()?;
    let label = item_to_string(&seq1);
    let value = item_to_string(&seq0);
    debug!(label = %label, value = %value, "fn:trace");
    Ok(XdmSequenceStream::from_vec(seq0))
}
