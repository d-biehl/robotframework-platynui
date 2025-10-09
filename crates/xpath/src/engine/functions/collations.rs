use super::common::{atomic_equal_with_collation, compare_default, deep_equal_default, item_to_string};
use crate::engine::runtime::{CallCtx, Error};
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequenceStream};

pub(super) fn compare_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq0 = args[0].materialize()?;
    let seq1 = args[1].materialize()?;
    let uri_opt = if args.len() == 3 {
        let seq2 = args[2].materialize()?;
        Some(item_to_string(&seq2))
    } else {
        None
    };
    let result = compare_default(ctx, &seq0, &seq1, uri_opt.as_deref())?;
    Ok(XdmSequenceStream::from_vec(result))
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

pub(super) fn deep_equal_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq0 = args[0].materialize()?;
    let seq1 = args[1].materialize()?;
    let uri_opt = if args.len() == 3 {
        let seq2 = args[2].materialize()?;
        Some(item_to_string(&seq2))
    } else {
        None
    };
    let result = deep_equal_default(ctx, &seq0, &seq1, uri_opt.as_deref())?;
    Ok(XdmSequenceStream::from_vec(result))
}
