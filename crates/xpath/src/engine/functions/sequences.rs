use super::common::to_number;
use crate::engine::runtime::{CallCtx, Error, ErrorCode};
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence, XdmSequenceStream};

/// Stream-based empty() implementation (zero-copy, early termination).
///
/// Returns true if the sequence is empty, false otherwise.
/// Performance: O(1) - stops after checking first item.
pub(super) fn empty_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let is_empty = args[0].iter().next().is_none();
    Ok(XdmSequenceStream::from_item(XdmItem::Atomic(XdmAtomicValue::Boolean(is_empty))))
}

/// Stream-based exists() implementation (zero-copy, early termination).
///
/// Returns true if the sequence contains at least one item, false otherwise.
/// Performance: O(1) - stops after finding first item.
pub(super) fn exists_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let has_items = args[0].iter().next().is_some();
    Ok(XdmSequenceStream::from_item(XdmItem::Atomic(XdmAtomicValue::Boolean(has_items))))
}

/// Stream-based count() implementation (zero-copy, no materialization).
///
/// This is the preferred implementation that works directly with lazy streams.
/// Performance: O(n) iteration but no heap allocation for intermediate results.
pub(super) fn count_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let count = args[0].iter().try_fold(0i64, |acc, item_result| item_result.map(|_| acc + 1))?;
    Ok(XdmSequenceStream::from_item(XdmItem::Atomic(XdmAtomicValue::Integer(count))))
}

/// Stream-based exactly-one() implementation (validates and passes through).
///
/// Performance: O(2) - checks first two items for validation.
pub(super) fn exactly_one_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let mut iter = args[0].iter();
    let first = iter.next();
    let second = iter.next();

    match (first, second) {
        (Some(Ok(item)), None) => Ok(XdmSequenceStream::from_item(item)),
        (None, _) | (Some(Ok(_)), Some(_)) => {
            Err(Error::from_code(ErrorCode::FORG0005, "exactly-one requires a sequence of length 1"))
        }
        (Some(Err(e)), _) => Err(e),
    }
}

/// Stream-based one-or-more() implementation (validates and passes through).
///
/// Performance: O(1) - checks only first item for validation.
pub(super) fn one_or_more_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    // Check if stream has at least one item without consuming entire stream
    let mut iter = args[0].iter();
    if iter.next().is_none() {
        return Err(Error::from_code(ErrorCode::FORG0004, "one-or-more requires at least one item"));
    }
    // Return original stream (validation passed)
    Ok(args[0].clone())
}

/// Stream-based zero-or-one() implementation (validates and passes through).
///
/// Performance: O(2) - checks first two items for validation.
pub(super) fn zero_or_one_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let mut iter = args[0].iter();
    let first = iter.next();
    let second = iter.next();

    match (first, second) {
        (None, _) => Ok(XdmSequenceStream::empty()),
        (Some(Ok(item)), None) => Ok(XdmSequenceStream::from_item(item)),
        (Some(Ok(_)), Some(_)) => Err(Error::from_code(ErrorCode::FORG0004, "zero-or-one requires at most one item")),
        (Some(Err(e)), _) => Err(e),
    }
}

/// Stream-based reverse() implementation.
///
/// Materializes the input stream, reverses the items, and returns a new stream.
/// Performance: O(n) iteration + O(n) memory for materialized Vec.
pub(super) fn reverse_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    // Materialize the stream (reverse requires random access)
    let items: Vec<XdmItem<N>> = args[0].iter().collect::<Result<Vec<_>, _>>()?;
    // Reverse and return as stream
    let reversed: Vec<XdmItem<N>> = items.into_iter().rev().collect();
    Ok(XdmSequenceStream::from_vec(reversed))
}

/// Stream-based subsequence() implementation.
///
/// Returns a subsequence using skip/take iterator adapters (zero-copy until materialization).
/// Performance: O(start + length) iteration, no intermediate allocations.
pub(super) fn subsequence_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let start_raw = {
        let items: Vec<XdmItem<N>> = args[1].iter().collect::<Result<Vec<_>, _>>()?;
        to_number(&items)?
    };

    let len_raw_opt = if args.len() == 3 {
        let items: Vec<XdmItem<N>> = args[2].iter().collect::<Result<Vec<_>, _>>()?;
        Some(to_number(&items)?)
    } else {
        None
    };

    // Handle NaN, infinity, negative cases
    if start_raw.is_nan() || (start_raw.is_infinite() && start_raw.is_sign_positive()) {
        return Ok(XdmSequenceStream::empty());
    }

    if let Some(len) = len_raw_opt
        && (len.is_nan() || len <= 0.0)
    {
        return Ok(XdmSequenceStream::empty());
    }

    let start_rounded = crate::engine::functions::common::round_half_to_even_f64(start_raw);
    let from_index = if start_rounded <= 1.0 { 0 } else { (start_rounded as isize - 1).max(0) as usize };

    if let Some(len_raw) = len_raw_opt {
        let len_rounded = crate::engine::functions::common::round_half_to_even_f64(len_raw);
        if len_rounded <= 0.0 {
            return Ok(XdmSequenceStream::empty());
        }
        let take_count = len_rounded as usize;

        // Create iterator that skips and takes
        let iter = args[0].iter().skip(from_index).take(take_count);
        let items: Vec<XdmItem<N>> = iter.collect::<Result<Vec<_>, _>>()?;
        Ok(XdmSequenceStream::from_vec(items))
    } else {
        // No length specified - take all from start position
        let iter = args[0].iter().skip(from_index);
        let items: Vec<XdmItem<N>> = iter.collect::<Result<Vec<_>, _>>()?;
        Ok(XdmSequenceStream::from_vec(items))
    }
}

/// Stream-based insert-before() implementation.
///
/// Inserts items from $inserts before the item at position $pos.
/// Performance: O(n) iteration, lazy until materialization.
pub(super) fn insert_before_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    // Extract position
    let pos_items: Vec<XdmItem<N>> = args[1].iter().collect::<Result<Vec<_>, _>>()?;
    let pos = to_number(&pos_items)?.floor() as isize;
    let insert_at = pos.max(1) as usize;

    // Materialize inserts stream
    let inserts: Vec<XdmItem<N>> = args[2].iter().collect::<Result<Vec<_>, _>>()?;

    // Build result using iterator combinators
    let mut result: Vec<XdmItem<N>> = Vec::new();
    let mut i = 1usize;

    for item_result in args[0].iter() {
        let item = item_result?;
        if i == insert_at {
            result.extend(inserts.iter().cloned());
        }
        result.push(item);
        i += 1;
    }

    // If insert position is beyond sequence length, append at end
    if insert_at >= i {
        result.extend(inserts.iter().cloned());
    }

    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based remove() implementation.
///
/// Removes the item at position $pos from the sequence.
/// Performance: O(n) iteration with filter, lazy until materialization.
pub(super) fn remove_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    // Extract position
    let pos_items: Vec<XdmItem<N>> = args[1].iter().collect::<Result<Vec<_>, _>>()?;
    let pos = to_number(&pos_items)?.floor() as isize;
    let remove_at = pos.max(1) as usize;

    // Use enumerate + filter to skip the item at remove_at
    let result: Vec<XdmItem<N>> = args[0]
        .iter()
        .enumerate()
        .filter_map(|(i, item_result)| {
            if i + 1 == remove_at {
                None // Skip this item
            } else {
                Some(item_result)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(XdmSequenceStream::from_vec(result))
}

//---- Stream-based distinct-values ----

pub(super) fn distinct_values_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    // Materialize and use existing implementation
    let seq: XdmSequence<N> = args[0].materialize()?;

    if args.len() == 1 {
        let result = super::common::distinct_values_impl(ctx, &seq, None)?;
        Ok(XdmSequenceStream::from_vec(result))
    } else {
        let uri_items: Vec<XdmItem<N>> = args[1].iter().collect::<Result<Vec<_>, _>>()?;
        let uri = super::common::item_to_string(&uri_items);
        let k = crate::engine::collation::resolve_collation(ctx.dyn_ctx, ctx.default_collation.as_ref(), Some(&uri))?;
        let result = super::common::distinct_values_impl(ctx, &seq, Some(k.as_trait()))?;
        Ok(XdmSequenceStream::from_vec(result))
    }
}

//---- Stream-based index-of ----

pub(super) fn index_of_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    // Materialize for now (search needs to compare all items anyway)
    let seq: XdmSequence<N> = args[0].materialize()?;
    let search: XdmSequence<N> = args[1].materialize()?;

    let collation_uri: Option<&str> = if args.len() == 3 {
        let uri_items: Vec<XdmItem<N>> = args[2].iter().collect::<Result<Vec<_>, _>>()?;
        let uri = super::common::item_to_string(&uri_items);
        if uri.is_empty() {
            None
        } else {
            Some(Box::leak(uri.into_boxed_str())) // Leak for 'static lifetime
        }
    } else {
        None
    };

    let result = super::common::index_of_default(ctx, &seq, &search, collation_uri)?;
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based unordered() implementation (zero-copy passthrough).
///
/// Simply returns the input stream as-is (unordered() is a hint to optimizer).
/// Performance: O(1) - no iteration, no materialization.
pub(super) fn unordered_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    // Clone the stream cursor (cheap operation)
    Ok(args[0].clone())
}
