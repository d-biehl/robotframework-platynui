use super::common::{local_name_default, name_default, node_name_default, parse_qname_lexical};
use crate::engine::runtime::{CallCtx, Error, ErrorCode};
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence, XdmSequenceStream};
use std::collections::HashMap;

/// Stream-based node-name() implementation.
///
/// Materializes input and delegates to node_name_default().
pub(super) fn node_name_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = node_name_default(ctx, Some(&seq))?;
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based name() implementation.
///
/// Handles both 0-arity (uses context item) and 1-arity versions.
pub(super) fn name_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let result = if args.is_empty() {
        name_default(ctx, None)?
    } else {
        let seq: XdmSequence<N> = args[0].materialize()?;
        name_default(ctx, Some(&seq))?
    };
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based local-name() implementation.
///
/// Handles both 0-arity (uses context item) and 1-arity versions.
pub(super) fn local_name_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let result = if args.is_empty() {
        local_name_default(ctx, None)?
    } else {
        let seq: XdmSequence<N> = args[0].materialize()?;
        local_name_default(ctx, Some(&seq))?
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn qname_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq0 = args[0].materialize()?;
    let seq1 = args[1].materialize()?;
    let ns_opt = if seq0.is_empty() {
        None
    } else {
        match &seq0[0] {
            XdmItem::Atomic(XdmAtomicValue::String(s)) | XdmItem::Atomic(XdmAtomicValue::AnyUri(s)) => Some(s.clone()),
            _ => {
                return Err(Error::from_code(ErrorCode::FORG0001, "QName namespace must be a string or anyURI"));
            }
        }
    };
    if seq1.is_empty() {
        return Err(Error::from_code(ErrorCode::FORG0001, "QName requires lexical QName"));
    }
    let qn_lex = match &seq1[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => s.clone(),
        _ => {
            return Err(Error::from_code(ErrorCode::FORG0001, "QName lexical must be string"));
        }
    };
    let (prefix_opt, local) =
        parse_qname_lexical(&qn_lex).map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid QName lexical"))?;
    let ns_uri = ns_opt.and_then(|s| if s.is_empty() { None } else { Some(s) });
    let result = vec![XdmItem::Atomic(XdmAtomicValue::QName { ns_uri, prefix: prefix_opt, local })];
    Ok(XdmSequenceStream::from_vec(result))
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
    map.entry("xml".to_string()).or_insert(crate::consts::XML_URI.to_string());
    map
}

pub(super) fn resolve_qname_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq0 = args[0].materialize()?;
    let seq1 = args[1].materialize()?;
    if seq0.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    let s = match &seq0[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => s.clone(),
        _ => {
            return Err(Error::from_code(ErrorCode::FORG0001, "resolve-QName requires string"));
        }
    };
    let enode = match &seq1[0] {
        XdmItem::Node(n) => n.clone(),
        _ => {
            return Err(Error::from_code(ErrorCode::XPTY0004, "resolve-QName requires element()"));
        }
    };
    if !matches!(enode.kind(), crate::model::NodeKind::Element) {
        return Err(Error::from_code(ErrorCode::XPTY0004, "resolve-QName requires element()"));
    }
    let (prefix_opt, local) =
        parse_qname_lexical(&s).map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid QName lexical"))?;
    let ns_uri = match &prefix_opt {
        None => None,
        Some(p) => inscope_for(enode).get(p).cloned(),
    };
    if prefix_opt.is_some() && ns_uri.is_none() {
        return Err(Error::from_code(ErrorCode::FORG0001, "unknown prefix"));
    }
    let result = vec![XdmItem::Atomic(XdmAtomicValue::QName { ns_uri, prefix: prefix_opt, local })];
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based namespace-uri-from-QName() implementation.
pub(super) fn namespace_uri_from_qname_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    let result = if let XdmItem::Atomic(XdmAtomicValue::QName { ns_uri, .. }) = &seq[0] {
        if let Some(uri) = ns_uri { vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(uri.clone()))] } else { vec![] }
    } else {
        return Err(Error::from_code(ErrorCode::XPTY0004, "namespace-uri-from-QName expects xs:QName"));
    };
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based local-name-from-QName() implementation.
pub(super) fn local_name_from_qname_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    let result = if let XdmItem::Atomic(XdmAtomicValue::QName { local, .. }) = &seq[0] {
        vec![XdmItem::Atomic(XdmAtomicValue::NCName(local.clone()))]
    } else {
        return Err(Error::from_code(ErrorCode::XPTY0004, "local-name-from-QName expects xs:QName"));
    };
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based prefix-from-QName() implementation.
pub(super) fn prefix_from_qname_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    let result = if let XdmItem::Atomic(XdmAtomicValue::QName { prefix, .. }) = &seq[0] {
        if let Some(p) = prefix { vec![XdmItem::Atomic(XdmAtomicValue::NCName(p.clone()))] } else { vec![] }
    } else {
        return Err(Error::from_code(ErrorCode::XPTY0004, "prefix-from-QName expects xs:QName"));
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn namespace_uri_for_prefix_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq0 = args[0].materialize()?;
    let seq1 = args[1].materialize()?;
    if seq0.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    let p = match &seq0[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => s.clone(),
        _ => {
            return Err(Error::from_code(ErrorCode::FORG0001, "prefix must be string"));
        }
    };
    let enode = match &seq1[0] {
        XdmItem::Node(n) => n.clone(),
        _ => {
            return Err(Error::from_code(ErrorCode::XPTY0004, "namespace-uri-for-prefix requires element()"));
        }
    };
    if !matches!(enode.kind(), crate::model::NodeKind::Element) {
        return Err(Error::from_code(ErrorCode::XPTY0004, "namespace-uri-for-prefix requires element()"));
    }
    if p.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    let map = inscope_for(enode);
    let result =
        if let Some(uri) = map.get(&p) { vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(uri.clone()))] } else { vec![] };
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based in-scope-prefixes() implementation.
pub(super) fn in_scope_prefixes_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq = args[0].materialize()?;
    let enode = match &seq[0] {
        XdmItem::Node(n) => n.clone(),
        _ => {
            return Err(Error::from_code(ErrorCode::XPTY0004, "in-scope-prefixes requires element()"));
        }
    };
    if !matches!(enode.kind(), crate::model::NodeKind::Element) {
        return Err(Error::from_code(ErrorCode::XPTY0004, "in-scope-prefixes requires element()"));
    }
    let map = inscope_for(enode);
    let mut out: Vec<XdmItem<N>> = Vec::with_capacity(map.len());
    for k in map.keys() {
        out.push(XdmItem::Atomic(XdmAtomicValue::NCName(k.clone())));
    }
    Ok(XdmSequenceStream::from_vec(out))
}

/// Stream-based namespace-uri() implementation.
///
/// Handles both 0-arity (uses context item) and 1-arity versions.
pub(super) fn namespace_uri_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let item_opt = if args.is_empty() {
        Some(super::common::require_context_item(ctx)?)
    } else {
        let seq = args[0].materialize()?;
        if seq.is_empty() { None } else { Some(seq[0].clone()) }
    };
    let Some(item) = item_opt else {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    };
    let result = match item {
        XdmItem::Node(n) => {
            use crate::model::NodeKind;
            if matches!(n.kind(), NodeKind::Namespace) {
                vec![]
            } else if let Some(q) = n.name() {
                if let Some(uri) = q.ns_uri {
                    vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(uri))]
                } else if let Some(pref) = q.prefix {
                    let map = inscope_for(n.clone());
                    if let Some(uri) = map.get(&pref) {
                        vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(uri.clone()))]
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        }
        _ => return Err(Error::from_code(ErrorCode::XPTY0004, "namespace-uri() expects node()")),
    };
    Ok(XdmSequenceStream::from_vec(result))
}
