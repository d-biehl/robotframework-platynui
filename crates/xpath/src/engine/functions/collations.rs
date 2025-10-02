use super::common::{
    atomic_equal_with_collation, compare_default, deep_equal_default, item_to_string,
};
use crate::engine::runtime::{CallCtx, Error};
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence};

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

pub(super) fn codepoint_equal_fn<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() || args[1].is_empty() {
        return Ok(vec![]);
    }
    let coll: std::rc::Rc<dyn crate::engine::collation::Collation> = ctx
        .dyn_ctx
        .collations
        .get(crate::engine::collation::CODEPOINT_URI)
        .unwrap_or_else(|| std::rc::Rc::new(crate::engine::collation::CodepointCollation));
    let a_item = args[0].first().cloned();
    let b_item = args[1].first().cloned();
    let eq = if let (Some(XdmItem::Atomic(a)), Some(XdmItem::Atomic(b))) = (a_item, b_item) {
        atomic_equal_with_collation(&a, &b, Some(coll.as_ref()))?
    } else {
        let sa = item_to_string(&args[0]);
        let sb = item_to_string(&args[1]);
        sa == sb
    };
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(eq))])
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
