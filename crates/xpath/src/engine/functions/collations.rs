use super::common::{
    atomic_equal_with_collation, compare_default, deep_equal_default, item_to_string,
};
use crate::engine::runtime::{CallCtx, Error};
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence, XdmSequenceStream};

pub(super) fn compare_fn<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args.len() == 2 {
        compare_default(ctx, &args[0], &args[1], None)
    } else {
        let uri = item_to_string(&args[2]);
        compare_default(ctx, &args[0], &args[1], Some(&uri))
    }
}

pub(super) fn codepoint_equal_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq0 = args[0].materialize()?;
    let seq1 = args[1].materialize()?;
    if seq0.is_empty() || seq1.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    let coll: std::rc::Rc<dyn crate::engine::collation::Collation> = ctx
        .dyn_ctx
        .collations
        .get(crate::engine::collation::CODEPOINT_URI)
        .unwrap_or_else(|| std::rc::Rc::new(crate::engine::collation::CodepointCollation));
    let a_item = seq0.first().cloned();
    let b_item = seq1.first().cloned();
    let eq = if let (Some(XdmItem::Atomic(a)), Some(XdmItem::Atomic(b))) = (a_item, b_item) {
        atomic_equal_with_collation(&a, &b, Some(coll.as_ref()))?
    } else {
        let sa = item_to_string(&seq0);
        let sb = item_to_string(&seq1);
        sa == sb
    };
    let result = vec![XdmItem::Atomic(XdmAtomicValue::Boolean(eq))];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn deep_equal_fn<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args.len() == 2 {
        deep_equal_default(ctx, &args[0], &args[1], None)
    } else {
        let uri = item_to_string(&args[2]);
        deep_equal_default(ctx, &args[0], &args[1], Some(&uri))
    }
}
