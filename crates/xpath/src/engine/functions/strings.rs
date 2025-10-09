use super::common::{as_string, item_to_string, normalize_space_default, substring_default, to_number};
use crate::engine::runtime::{CallCtx, Error, ErrorCode};
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence, XdmSequenceStream};
use itertools::Itertools;
use std::collections::{HashMap, hash_map::Entry};

/// Stream-based string() implementation.
/// Handles both 0-arity (uses context item) and 1-arity versions.
pub(super) fn string_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let result = if args.is_empty() {
        super::common::string_default(ctx, None)?
    } else {
        let seq = args[0].materialize()?;
        super::common::string_default(ctx, Some(&seq))?
    };
    Ok(XdmSequenceStream::from_vec(result))
}
/// Stream-based string-length() implementation.
/// Handles both 0-arity (uses context item) and 1-arity versions.
pub(super) fn string_length_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let s = if args.is_empty() {
        let seq = super::common::string_default(ctx, None)?;
        item_to_string(&seq)
    } else {
        let seq = args[0].materialize()?;
        item_to_string(&seq)
    };
    let result = vec![XdmItem::Atomic(XdmAtomicValue::Integer(s.chars().count() as i64))];
    Ok(XdmSequenceStream::from_vec(result))
}
/// Stream-based untypedAtomic() implementation.
pub(super) fn untyped_atomic_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq = args[0].materialize()?;
    let result = vec![XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(item_to_string(&seq)))];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn concat_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let mut out = String::new();
    for stream in args {
        let seq = stream.materialize()?;
        out.push_str(&item_to_string(&seq));
    }
    Ok(XdmSequenceStream::from_vec(vec![XdmItem::Atomic(XdmAtomicValue::String(out))]))
}

/// Stream-based string-to-codepoints() implementation.
pub(super) fn string_to_codepoints_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::empty());
    }
    let s = item_to_string(&seq);
    let mut out: XdmSequence<N> = Vec::with_capacity(s.chars().count());
    for ch in s.chars() {
        out.push(XdmItem::Atomic(XdmAtomicValue::Integer(ch as u32 as i64)));
    }
    Ok(XdmSequenceStream::from_vec(out))
}

/// Stream-based codepoints-to-string() implementation.
pub(super) fn codepoints_to_string_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq = args[0].materialize()?;
    let mut s = String::new();
    for it in &seq {
        match it {
            XdmItem::Atomic(XdmAtomicValue::Integer(i)) => {
                let v = *i;
                if !(0..=0x10FFFF).contains(&v) {
                    return Err(Error::from_code(ErrorCode::FORG0001, "invalid code point"));
                }
                let u = v as u32;
                if let Some(c) = char::from_u32(u) {
                    s.push(c);
                } else {
                    return Err(Error::from_code(ErrorCode::FORG0001, "invalid code point"));
                }
            }
            _ => {
                return Err(Error::from_code(ErrorCode::XPTY0004, "codepoints-to-string expects xs:integer*"));
            }
        }
    }
    let result = vec![XdmItem::Atomic(XdmAtomicValue::String(s))];
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based contains() implementation.
///
/// Handles both 2-arity and 3-arity versions (with optional collation).
pub(super) fn contains_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let s_seq = args[0].materialize()?;
    let sub_seq = args[1].materialize()?;
    let uri_opt = if args.len() == 3 {
        let coll_seq = args[2].materialize()?;
        Some(item_to_string(&coll_seq))
    } else {
        None
    };
    let result = contains_default(ctx, &s_seq, &sub_seq, uri_opt.as_deref())?;
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based starts-with() implementation.
///
/// Handles both 2-arity and 3-arity versions (with optional collation).
pub(super) fn starts_with_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let s_seq = args[0].materialize()?;
    let sub_seq = args[1].materialize()?;
    let uri_opt = if args.len() == 3 {
        let coll_seq = args[2].materialize()?;
        Some(item_to_string(&coll_seq))
    } else {
        None
    };
    let result = starts_with_default(ctx, &s_seq, &sub_seq, uri_opt.as_deref())?;
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based ends-with() implementation.
///
/// Handles both 2-arity and 3-arity versions (with optional collation).
pub(super) fn ends_with_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let s_seq = args[0].materialize()?;
    let sub_seq = args[1].materialize()?;
    let uri_opt = if args.len() == 3 {
        let coll_seq = args[2].materialize()?;
        Some(item_to_string(&coll_seq))
    } else {
        None
    };
    let result = ends_with_default(ctx, &s_seq, &sub_seq, uri_opt.as_deref())?;
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based substring() implementation.
///
/// Handles both 2-arity and 3-arity versions.
pub(super) fn substring_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let s_seq = args[0].materialize()?;
    let start_seq = args[1].materialize()?;
    let s = item_to_string(&s_seq);
    let start_raw = to_number(&start_seq)?;
    let out = if args.len() == 2 {
        substring_default(&s, start_raw, None)
    } else {
        let len_seq = args[2].materialize()?;
        let len_raw = to_number(&len_seq)?;
        substring_default(&s, start_raw, Some(len_raw))
    };
    let result = vec![XdmItem::Atomic(XdmAtomicValue::String(out))];
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based substring-before() implementation.
pub(super) fn substring_before_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let s_seq = args[0].materialize()?;
    let sub_seq = args[1].materialize()?;
    let s = item_to_string(&s_seq);
    let sub = item_to_string(&sub_seq);
    let result = if sub.is_empty() || s.is_empty() {
        vec![XdmItem::Atomic(XdmAtomicValue::String(String::new()))]
    } else if let Some(idx) = s.find(&sub) {
        vec![XdmItem::Atomic(XdmAtomicValue::String(s[..idx].to_string()))]
    } else {
        vec![XdmItem::Atomic(XdmAtomicValue::String(String::new()))]
    };
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based substring-after() implementation.
pub(super) fn substring_after_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let s_seq = args[0].materialize()?;
    let sub_seq = args[1].materialize()?;
    let s = item_to_string(&s_seq);
    let sub = item_to_string(&sub_seq);
    let result = if sub.is_empty() {
        vec![XdmItem::Atomic(XdmAtomicValue::String(s))]
    } else if let Some(idx) = s.find(&sub) {
        let after = &s[idx + sub.len()..];
        vec![XdmItem::Atomic(XdmAtomicValue::String(after.to_string()))]
    } else {
        vec![XdmItem::Atomic(XdmAtomicValue::String(String::new()))]
    };
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based normalize-space() implementation.
///
/// Handles both 0-arity (uses context item) and 1-arity versions.
pub(super) fn normalize_space_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let result = if args.is_empty() {
        normalize_space_default(ctx, None)?
    } else {
        let seq = args[0].materialize()?;
        normalize_space_default(ctx, Some(&seq))?
    };
    Ok(XdmSequenceStream::from_vec(result))
}
/// Stream-based translate() implementation.
pub(super) fn translate_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let s_seq = args[0].materialize()?;
    let map_seq = args[1].materialize()?;
    let trans_seq = args[2].materialize()?;
    let s = item_to_string(&s_seq);
    let map = item_to_string(&map_seq);
    let trans = item_to_string(&trans_seq);
    let mut table: HashMap<char, Option<char>> = HashMap::new();
    let mut trans_iter = trans.chars();
    for m in map.chars() {
        match table.entry(m) {
            Entry::Vacant(e) => {
                let repl = trans_iter.next();
                e.insert(repl);
            }
            Entry::Occupied(_) => {
                let _ = trans_iter.next();
            }
        }
    }
    let mut out = String::new();
    for ch in s.chars() {
        if let Some(opt) = table.get(&ch) {
            if let Some(rep) = opt {
                out.push(*rep);
            }
        } else {
            out.push(ch);
        }
    }
    let result = vec![XdmItem::Atomic(XdmAtomicValue::String(out))];
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based lower-case() implementation.
pub(super) fn lower_case_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq = args[0].materialize()?;
    let result = vec![XdmItem::Atomic(XdmAtomicValue::String(item_to_string(&seq).to_lowercase()))];
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based upper-case() implementation.
pub(super) fn upper_case_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq = args[0].materialize()?;
    let result = vec![XdmItem::Atomic(XdmAtomicValue::String(item_to_string(&seq).to_uppercase()))];
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based string-join() implementation.
pub(super) fn string_join_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let sep_seq: XdmSequence<N> = args[1].materialize()?;
    let sep = item_to_string(&sep_seq);

    // Stream the first argument and join efficiently
    let items: Vec<XdmItem<N>> = args[0].iter().collect::<Result<Vec<_>, _>>()?;
    let joined = items
        .iter()
        .map(|it| match it {
            XdmItem::Atomic(a) => as_string(a),
            XdmItem::Node(n) => n.string_value(),
        })
        .join(&sep);

    Ok(XdmSequenceStream::from_item(XdmItem::Atomic(XdmAtomicValue::String(joined))))
}

fn contains_default<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    s_seq: &XdmSequence<N>,
    sub_seq: &XdmSequence<N>,
    collation_uri: Option<&str>,
) -> Result<XdmSequence<N>, Error> {
    let s = item_to_string(s_seq);
    let sub = item_to_string(sub_seq);
    let uri_opt = collation_uri.and_then(|u| if u.is_empty() { None } else { Some(u) });
    let k = crate::engine::collation::resolve_collation(ctx.dyn_ctx, ctx.default_collation.as_ref(), uri_opt)?;
    let c = k.as_trait();
    let b = c.key(&s).contains(&c.key(&sub));
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))])
}

fn starts_with_default<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    s_seq: &XdmSequence<N>,
    sub_seq: &XdmSequence<N>,
    collation_uri: Option<&str>,
) -> Result<XdmSequence<N>, Error> {
    let s = item_to_string(s_seq);
    let sub = item_to_string(sub_seq);
    let uri_opt = collation_uri.and_then(|u| if u.is_empty() { None } else { Some(u) });
    let k = crate::engine::collation::resolve_collation(ctx.dyn_ctx, ctx.default_collation.as_ref(), uri_opt)?;
    let c = k.as_trait();
    let b = c.key(&s).starts_with(&c.key(&sub));
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))])
}

fn ends_with_default<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    s_seq: &XdmSequence<N>,
    sub_seq: &XdmSequence<N>,
    collation_uri: Option<&str>,
) -> Result<XdmSequence<N>, Error> {
    let s = item_to_string(s_seq);
    let sub = item_to_string(sub_seq);
    let uri_opt = collation_uri.and_then(|u| if u.is_empty() { None } else { Some(u) });
    let k = crate::engine::collation::resolve_collation(ctx.dyn_ctx, ctx.default_collation.as_ref(), uri_opt)?;
    let c = k.as_trait();
    let b = c.key(&s).ends_with(&c.key(&sub));
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))])
}
