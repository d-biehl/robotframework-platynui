use super::common::{matches_default, replace_default, tokenize_default};
use crate::engine::runtime::{CallCtx, Error};
use crate::xdm::XdmSequence;

pub(super) fn matches_fn<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args.len() == 2 {
        matches_default(ctx, &args[0], &args[1], None)
    } else {
        let flags = super::common::item_to_string(&args[2]);
        matches_default(ctx, &args[0], &args[1], Some(&flags))
    }
}

pub(super) fn replace_fn<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args.len() == 3 {
        replace_default(ctx, &args[0], &args[1], &args[2], None)
    } else {
        let flags = super::common::item_to_string(&args[3]);
        replace_default(ctx, &args[0], &args[1], &args[2], Some(&flags))
    }
}

pub(super) fn tokenize_fn<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args.len() == 2 {
        tokenize_default(ctx, &args[0], &args[1], None)
    } else {
        let flags = super::common::item_to_string(&args[2]);
        tokenize_default(ctx, &args[0], &args[1], Some(&flags))
    }
}
