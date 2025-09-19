use super::common::{local_name_default, name_default, node_name_default, parse_qname_lexical};
use crate::engine::runtime::{CallCtx, Error, ErrorCode};
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence};
use std::collections::HashMap;

pub(super) fn node_name_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    node_name_default(ctx, Some(&args[0]))
}

pub(super) fn name_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    match args.len() {
        0 => name_default(ctx, None),
        1 => name_default(ctx, Some(&args[0])),
        _ => unreachable!("registry guarantees arity in range"),
    }
}

pub(super) fn local_name_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    match args.len() {
        0 => local_name_default(ctx, None),
        1 => local_name_default(ctx, Some(&args[0])),
        _ => unreachable!("registry guarantees arity in range"),
    }
}

pub(super) fn qname_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let ns_opt = if args[0].is_empty() {
        None
    } else {
        match &args[0][0] {
            XdmItem::Atomic(XdmAtomicValue::String(s))
            | XdmItem::Atomic(XdmAtomicValue::AnyUri(s)) => Some(s.clone()),
            _ => {
                return Err(Error::from_code(
                    ErrorCode::FORG0001,
                    "QName namespace must be a string or anyURI",
                ));
            }
        }
    };
    if args[1].is_empty() {
        return Err(Error::from_code(
            ErrorCode::FORG0001,
            "QName requires lexical QName",
        ));
    }
    let qn_lex = match &args[1][0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => s.clone(),
        _ => {
            return Err(Error::from_code(
                ErrorCode::FORG0001,
                "QName lexical must be string",
            ));
        }
    };
    let (prefix_opt, local) = parse_qname_lexical(&qn_lex)
        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid QName lexical"))?;
    let ns_uri = ns_opt.and_then(|s| if s.is_empty() { None } else { Some(s) });
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::QName {
        ns_uri,
        prefix: prefix_opt,
        local,
    })])
}

fn inscope_for<N: crate::model::XdmNode + Clone>(mut n: N) -> HashMap<String, String> {
    use crate::model::NodeKind;
    let mut map: HashMap<String, String> = HashMap::new();
    loop {
        if matches!(n.kind(), NodeKind::Element) {
            for ns in n.namespaces() {
                if let Some(q) = ns.name()
                    && let (Some(p), Some(uri)) = (q.prefix, q.ns_uri)
                {
                    map.entry(p).or_insert(uri);
                }
            }
        }
        if let Some(p) = n.parent() {
            n = p;
        } else {
            break;
        }
    }
    map.entry("xml".to_string())
        .or_insert(crate::consts::XML_URI.to_string());
    map
}

pub(super) fn resolve_qname_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    let s = match &args[0][0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => s.clone(),
        _ => {
            return Err(Error::from_code(
                ErrorCode::FORG0001,
                "resolve-QName requires string",
            ));
        }
    };
    let enode = match &args[1][0] {
        XdmItem::Node(n) => n.clone(),
        _ => {
            return Err(Error::from_code(
                ErrorCode::XPTY0004,
                "resolve-QName requires element()",
            ));
        }
    };
    if !matches!(enode.kind(), crate::model::NodeKind::Element) {
        return Err(Error::from_code(
            ErrorCode::XPTY0004,
            "resolve-QName requires element()",
        ));
    }
    let (prefix_opt, local) = parse_qname_lexical(&s)
        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid QName lexical"))?;
    let ns_uri = match &prefix_opt {
        None => None,
        Some(p) => inscope_for(enode).get(p).cloned(),
    };
    if prefix_opt.is_some() && ns_uri.is_none() {
        return Err(Error::from_code(ErrorCode::FORG0001, "unknown prefix"));
    }
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::QName {
        ns_uri,
        prefix: prefix_opt,
        local,
    })])
}

pub(super) fn namespace_uri_from_qname_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if let XdmItem::Atomic(XdmAtomicValue::QName { ns_uri, .. }) = &args[0][0] {
        if let Some(uri) = ns_uri {
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(uri.clone()))])
        } else {
            Ok(vec![])
        }
    } else {
        Err(Error::from_code(
            ErrorCode::XPTY0004,
            "namespace-uri-from-QName expects xs:QName",
        ))
    }
}

pub(super) fn local_name_from_qname_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if let XdmItem::Atomic(XdmAtomicValue::QName { local, .. }) = &args[0][0] {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::NCName(local.clone()))])
    } else {
        Err(Error::from_code(
            ErrorCode::XPTY0004,
            "local-name-from-QName expects xs:QName",
        ))
    }
}

pub(super) fn prefix_from_qname_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if let XdmItem::Atomic(XdmAtomicValue::QName { prefix, .. }) = &args[0][0] {
        if let Some(p) = prefix {
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::NCName(p.clone()))])
        } else {
            Ok(vec![])
        }
    } else {
        Err(Error::from_code(
            ErrorCode::XPTY0004,
            "prefix-from-QName expects xs:QName",
        ))
    }
}

pub(super) fn namespace_uri_for_prefix_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    let p = match &args[0][0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => s.clone(),
        _ => {
            return Err(Error::from_code(
                ErrorCode::FORG0001,
                "prefix must be string",
            ));
        }
    };
    let enode = match &args[1][0] {
        XdmItem::Node(n) => n.clone(),
        _ => {
            return Err(Error::from_code(
                ErrorCode::XPTY0004,
                "namespace-uri-for-prefix requires element()",
            ));
        }
    };
    if !matches!(enode.kind(), crate::model::NodeKind::Element) {
        return Err(Error::from_code(
            ErrorCode::XPTY0004,
            "namespace-uri-for-prefix requires element()",
        ));
    }
    if p.is_empty() {
        return Ok(vec![]);
    }
    let map = inscope_for(enode);
    if let Some(uri) = map.get(&p) {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(uri.clone()))])
    } else {
        Ok(vec![])
    }
}

pub(super) fn in_scope_prefixes_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let enode = match &args[0][0] {
        XdmItem::Node(n) => n.clone(),
        _ => {
            return Err(Error::from_code(
                ErrorCode::XPTY0004,
                "in-scope-prefixes requires element()",
            ));
        }
    };
    if !matches!(enode.kind(), crate::model::NodeKind::Element) {
        return Err(Error::from_code(
            ErrorCode::XPTY0004,
            "in-scope-prefixes requires element()",
        ));
    }
    let map = inscope_for(enode);
    let mut out: Vec<XdmItem<_>> = Vec::with_capacity(map.len());
    for k in map.keys() {
        out.push(XdmItem::Atomic(XdmAtomicValue::NCName(k.clone())));
    }
    Ok(out)
}

pub(super) fn namespace_uri_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let item_opt = if args.is_empty() {
        Some(super::common::require_context_item(ctx)?)
    } else if args[0].is_empty() {
        None
    } else {
        Some(args[0][0].clone())
    };
    let Some(item) = item_opt else {
        return Ok(vec![]);
    };
    match item {
        XdmItem::Node(n) => {
            use crate::model::NodeKind;
            if matches!(n.kind(), NodeKind::Namespace) {
                return Ok(vec![]);
            }
            if let Some(q) = n.name() {
                if let Some(uri) = q.ns_uri {
                    return Ok(vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(uri))]);
                }
                if let Some(pref) = q.prefix {
                    let map = inscope_for(n.clone());
                    if let Some(uri) = map.get(&pref) {
                        return Ok(vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(uri.clone()))]);
                    }
                }
            }
            Ok(vec![])
        }
        _ => Err(Error::from_code(
            ErrorCode::XPTY0004,
            "namespace-uri() expects node()",
        )),
    }
}
