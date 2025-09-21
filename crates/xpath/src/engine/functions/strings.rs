use super::common::{
    as_string, item_to_string, normalize_space_default, substring_default, to_number,
};
use crate::engine::runtime::{CallCtx, Error, ErrorCode};
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence};
use smallvec::SmallVec;
use std::collections::{HashMap, hash_map::Entry};

pub(super) fn string_fn<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    match args.len() {
        0 => super::common::string_default(ctx, None),
        1 => super::common::string_default(ctx, Some(&args[0])),
        _ => unreachable!("registry guarantees arity in range"),
    }
}

pub(super) fn string_length_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let s = if args.is_empty() {
        let seq = super::common::string_default(ctx, None)?;
        item_to_string(&seq)
    } else {
        item_to_string(&args[0])
    };
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(s.chars().count() as i64))])
}

pub(super) fn untyped_atomic_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(item_to_string(&args[0])))])
}

pub(super) fn concat_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let mut out = String::new();
    for a in args {
        out.push_str(&item_to_string(a));
    }
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(out))])
}

pub(super) fn string_to_codepoints_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    let s = item_to_string(&args[0]);
    let mut out: XdmSequence<N> = Vec::with_capacity(s.chars().count());
    for ch in s.chars() {
        out.push(XdmItem::Atomic(XdmAtomicValue::Integer(ch as u32 as i64)));
    }
    Ok(out)
}

pub(super) fn codepoints_to_string_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let mut s = String::new();
    for it in &args[0] {
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
                return Err(Error::from_code(
                    ErrorCode::XPTY0004,
                    "codepoints-to-string expects xs:integer*",
                ));
            }
        }
    }
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(s))])
}

pub(super) fn contains_fn<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let uri_opt = if args.len() == 3 { Some(item_to_string(&args[2])) } else { None };
    contains_default(ctx, &args[0], &args[1], uri_opt.as_deref())
}

pub(super) fn starts_with_fn<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let uri_opt = if args.len() == 3 { Some(item_to_string(&args[2])) } else { None };
    starts_with_default(ctx, &args[0], &args[1], uri_opt.as_deref())
}

pub(super) fn ends_with_fn<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let uri_opt = if args.len() == 3 { Some(item_to_string(&args[2])) } else { None };
    ends_with_default(ctx, &args[0], &args[1], uri_opt.as_deref())
}

pub(super) fn substring_fn<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let s = item_to_string(&args[0]);
    let start_raw = to_number(&args[1])?;
    let out = if args.len() == 2 {
        substring_default(&s, start_raw, None)
    } else {
        let len_raw = to_number(&args[2])?;
        substring_default(&s, start_raw, Some(len_raw))
    };
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(out))])
}

pub(super) fn substring_before_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let s = item_to_string(&args[0]);
    let sub = item_to_string(&args[1]);
    if sub.is_empty() || s.is_empty() {
        return Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(String::new()))]);
    }
    if let Some(idx) = s.find(&sub) {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(s[..idx].to_string()))])
    } else {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(String::new()))])
    }
}

pub(super) fn substring_after_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let s = item_to_string(&args[0]);
    let sub = item_to_string(&args[1]);
    if sub.is_empty() {
        return Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(s))]);
    }
    if let Some(idx) = s.find(&sub) {
        let after = &s[idx + sub.len()..];
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(after.to_string()))])
    } else {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(String::new()))])
    }
}

pub(super) fn normalize_space_fn<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    match args.len() {
        0 => normalize_space_default(ctx, None),
        1 => normalize_space_default(ctx, Some(&args[0])),
        _ => unreachable!("registry guarantees arity in range"),
    }
}

pub(super) fn translate_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let s = item_to_string(&args[0]);
    let map = item_to_string(&args[1]);
    let trans = item_to_string(&args[2]);
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
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(out))])
}

pub(super) fn lower_case_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(item_to_string(&args[0]).to_lowercase()))])
}

pub(super) fn upper_case_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(item_to_string(&args[0]).to_uppercase()))])
}

pub(super) fn string_join_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let sep = item_to_string(&args[1]);
    let mut parts: SmallVec<[String; 8]> = SmallVec::new(); // Most joins have few parts
    for it in &args[0] {
        match it {
            XdmItem::Atomic(a) => parts.push(as_string(a)),
            XdmItem::Node(n) => parts.push(n.string_value()),
        }
    }
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(parts.join(&sep)))])
}

fn contains_default<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    s_seq: &XdmSequence<N>,
    sub_seq: &XdmSequence<N>,
    collation_uri: Option<&str>,
) -> Result<XdmSequence<N>, Error> {
    let s = item_to_string(s_seq);
    let sub = item_to_string(sub_seq);
    let uri_opt = collation_uri.and_then(|u| if u.is_empty() { None } else { Some(u) });
    let k = crate::engine::collation::resolve_collation(
        ctx.dyn_ctx,
        ctx.default_collation.as_ref(),
        uri_opt,
    )?;
    let c = k.as_trait();
    let b = c.key(&s).contains(&c.key(&sub));
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))])
}

fn starts_with_default<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    s_seq: &XdmSequence<N>,
    sub_seq: &XdmSequence<N>,
    collation_uri: Option<&str>,
) -> Result<XdmSequence<N>, Error> {
    let s = item_to_string(s_seq);
    let sub = item_to_string(sub_seq);
    let uri_opt = collation_uri.and_then(|u| if u.is_empty() { None } else { Some(u) });
    let k = crate::engine::collation::resolve_collation(
        ctx.dyn_ctx,
        ctx.default_collation.as_ref(),
        uri_opt,
    )?;
    let c = k.as_trait();
    let b = c.key(&s).starts_with(&c.key(&sub));
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))])
}

fn ends_with_default<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    s_seq: &XdmSequence<N>,
    sub_seq: &XdmSequence<N>,
    collation_uri: Option<&str>,
) -> Result<XdmSequence<N>, Error> {
    let s = item_to_string(s_seq);
    let sub = item_to_string(sub_seq);
    let uri_opt = collation_uri.and_then(|u| if u.is_empty() { None } else { Some(u) });
    let k = crate::engine::collation::resolve_collation(
        ctx.dyn_ctx,
        ctx.default_collation.as_ref(),
        uri_opt,
    )?;
    let c = k.as_trait();
    let b = c.key(&s).ends_with(&c.key(&sub));
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))])
}
