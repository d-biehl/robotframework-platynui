use super::common::{matches_default, replace_default, tokenize_default};
use crate::engine::runtime::{CallCtx, Error};
use crate::xdm::XdmSequenceStream;

pub(super) fn matches_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq0 = args[0].materialize()?;
    let seq1 = args[1].materialize()?;
    let flags_opt = if args.len() == 3 {
        let seq2 = args[2].materialize()?;
        Some(super::common::item_to_string(&seq2))
    } else {
        None
    };
    let result = matches_default(ctx, &seq0, &seq1, flags_opt.as_deref())?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn replace_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq0 = args[0].materialize()?;
    let seq1 = args[1].materialize()?;
    let seq2 = args[2].materialize()?;
    let flags_opt = if args.len() == 4 {
        let seq3 = args[3].materialize()?;
        Some(super::common::item_to_string(&seq3))
    } else {
        None
    };
    let result = replace_default(ctx, &seq0, &seq1, &seq2, flags_opt.as_deref())?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn tokenize_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq0 = args[0].materialize()?;
    let seq1 = args[1].materialize()?;
    let flags_opt = if args.len() == 3 {
        let seq2 = args[2].materialize()?;
        Some(super::common::item_to_string(&seq2))
    } else {
        None
    };
    let result = tokenize_default(ctx, &seq0, &seq1, flags_opt.as_deref())?;
    Ok(XdmSequenceStream::from_vec(result))
}
