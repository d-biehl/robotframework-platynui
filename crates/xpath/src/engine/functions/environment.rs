use super::common::{item_to_string, require_context_item};
use crate::engine::runtime::{CallCtx, Error, ErrorCode};
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence, XdmSequenceStream};
use unicode_normalization::UnicodeNormalization;
use url::Url;

/// Stream-based default-collation() implementation.
pub(super) fn default_collation_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    _args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let uri = if let Some(c) = &ctx.default_collation {
        c.uri().to_string()
    } else if let Some(s) = &ctx.static_ctx.default_collation {
        s.clone()
    } else {
        crate::engine::collation::CODEPOINT_URI.to_string()
    };
    let result = vec![XdmItem::Atomic(XdmAtomicValue::String(uri))];
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based static-base-uri() implementation.
pub(super) fn static_base_uri_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    _args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let result = if let Some(b) = &ctx.static_ctx.base_uri {
        vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(b.clone()))]
    } else {
        vec![]
    };
    Ok(XdmSequenceStream::from_vec(result))
}

/// Stream-based root() implementation.
///
/// Handles both 0-arity (uses context item) and 1-arity versions.
pub(super) fn root_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let item_opt = if args.is_empty() {
        Some(require_context_item(ctx)?)
    } else {
        let seq: XdmSequence<N> = args[0].materialize()?;
        if seq.is_empty() { None } else { Some(seq[0].clone()) }
    };
    let Some(item) = item_opt else {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    };
    match item {
        XdmItem::Node(n) => {
            let mut cur = n.clone();
            let mut p = cur.parent();
            while let Some(pp) = p {
                cur = pp.clone();
                p = cur.parent();
            }
            Ok(XdmSequenceStream::from_vec(vec![XdmItem::Node(cur)]))
        }
        _ => Err(Error::from_code(ErrorCode::XPTY0004, "root() expects node()")),
    }
}

pub(super) fn base_uri_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let item_opt = if args.is_empty() {
        Some(require_context_item(ctx)?)
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
            if let Some(uri) = n.base_uri() {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(uri))])
            } else {
                Ok(vec![])
            }
        }
        _ => Err(Error::from_code(ErrorCode::XPTY0004, "base-uri() expects node()")),
    }
}

pub(super) fn base_uri_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq = if args.is_empty() { vec![] } else { args[0].materialize()? };
    let materialized_args = if args.is_empty() {
        // Arity 0 - use context item
        vec![]
    } else {
        vec![seq]
    };
    let result = base_uri_fn(ctx, &materialized_args)?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn document_uri_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let item_opt = if args.is_empty() {
        Some(require_context_item(ctx)?)
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
            if matches!(n.kind(), crate::model::NodeKind::Document)
                && let Some(uri) = n.base_uri()
            {
                return Ok(vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(uri))]);
            }
            Ok(vec![])
        }
        _ => Err(Error::from_code(ErrorCode::XPTY0004, "document-uri() expects node()")),
    }
}

pub(super) fn document_uri_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq = if args.is_empty() { vec![] } else { args[0].materialize()? };
    let materialized_args = if args.is_empty() {
        // Arity 0 - use context item
        vec![]
    } else {
        vec![seq]
    };
    let result = document_uri_fn(ctx, &materialized_args)?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn lang_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "fn:lang expects at most one string argument"));
    }
    if args[0].is_empty() {
        return Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(false))]);
    }
    let test = item_to_string(&args[0]).to_ascii_lowercase();
    let target_item = if args.len() == 1 {
        require_context_item(ctx)?
    } else {
        if args[1].len() != 1 {
            return Err(Error::from_code(ErrorCode::FORG0006, "fn:lang requires exactly one node in second argument"));
        }
        args[1][0].clone()
    };
    let mut n = match target_item {
        XdmItem::Node(node) => node,
        _ => {
            return Err(Error::from_code(ErrorCode::XPTY0004, "fn:lang requires node() as second argument"));
        }
    };
    let mut lang_val: Option<String> = None;
    loop {
        for a in n.attributes() {
            if let Some(q) = a.name() {
                let is_xml_lang = q.local == "lang"
                    && (q.prefix.as_deref() == Some("xml") || q.ns_uri.as_deref() == Some(crate::consts::XML_URI));
                if is_xml_lang {
                    lang_val = Some(a.string_value());
                    break;
                }
            }
        }
        if lang_val.is_some() {
            break;
        }
        if let Some(p) = n.parent() {
            n = p;
        } else {
            break;
        }
    }
    let result = if let Some(lang) = lang_val {
        let l = lang.to_ascii_lowercase();
        l == test || (l.starts_with(&test) && l.chars().nth(test.len()) == Some('-'))
    } else {
        false
    };
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(result))])
}

pub(super) fn lang_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq0 = args[0].materialize()?;
    let seq1 = if args.len() >= 2 { args[1].materialize()? } else { vec![] };
    let materialized_args = if seq1.is_empty() { vec![seq0] } else { vec![seq0, seq1] };
    let result = lang_fn(ctx, &materialized_args)?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn encode_for_uri_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "encode-for-uri expects at most one string argument"));
    }
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![XdmItem::Atomic(XdmAtomicValue::String(String::new()))]));
    }
    let s = item_to_string(&seq);
    fn is_unreserved(ch: char) -> bool {
        ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '~')
    }
    let mut out = String::new();
    for ch in s.chars() {
        if is_unreserved(ch) {
            out.push(ch);
        } else {
            let mut buf = [0u8; 4];
            for b in ch.encode_utf8(&mut buf).as_bytes() {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    let result = vec![XdmItem::Atomic(XdmAtomicValue::String(out))];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn nilled_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    let item = &seq[0];
    let XdmItem::Node(n) = item else {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    };
    if !matches!(n.kind(), crate::model::NodeKind::Element) {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    let mut is_nilled = false;
    for a in n.attributes() {
        if let Some(q) = a.name() {
            let is_xsi_nil = q.local == "nil"
                && (q.prefix.as_deref() == Some("xsi") || q.ns_uri.as_deref() == Some(crate::consts::XSI));
            if is_xsi_nil {
                let v = a.string_value().trim().to_ascii_lowercase();
                if v == "true" || v == "1" {
                    is_nilled = true;
                    break;
                }
            }
        }
    }
    let result = vec![XdmItem::Atomic(XdmAtomicValue::Boolean(is_nilled))];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn iri_to_uri_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let s = item_to_string(&seq);
    let mut out = String::new();
    for ch in s.chars() {
        if ch.is_ascii() && ch != ' ' {
            out.push(ch);
        } else {
            let mut buf = [0u8; 4];
            for b in ch.encode_utf8(&mut buf).as_bytes() {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    let result = vec![XdmItem::Atomic(XdmAtomicValue::String(out))];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn escape_html_uri_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "escape-html-uri expects at most one string argument"));
    }
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![XdmItem::Atomic(XdmAtomicValue::String(String::new()))]));
    }
    let s = item_to_string(&seq);
    let mut out = String::new();
    for ch in s.chars() {
        if ch.is_ascii() && (ch as u32) >= 32 && (ch as u32) <= 126 && ch != ' ' {
            out.push(ch);
        } else {
            let mut buf = [0u8; 4];
            for b in ch.encode_utf8(&mut buf).as_bytes() {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    let result = vec![XdmItem::Atomic(XdmAtomicValue::String(out))];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn resolve_uri_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "resolve-uri expects at most one URI argument"));
    }
    if args.len() == 2 && args[1].len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "resolve-uri expects at most one base URI argument"));
    }
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    let rel = item_to_string(&args[0]);
    if let Ok(abs) = Url::parse(&rel) {
        let abs_str: String = abs.into();
        return Ok(vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(abs_str))]);
    }
    let base_candidate = if args.len() == 2 {
        if args[1].is_empty() { ctx.static_ctx.base_uri.clone() } else { Some(item_to_string(&args[1])) }
    } else {
        ctx.static_ctx.base_uri.clone()
    };
    let base_str = base_candidate.ok_or_else(|| Error::from_code(ErrorCode::FONS0005, "base-uri is undefined"))?;
    let base_url = Url::parse(&base_str).map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid base URI"))?;
    let joined = base_url.join(&rel).map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid relative URI"))?;
    let joined_str: String = joined.into();
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(joined_str))])
}

pub(super) fn resolve_uri_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq0 = args[0].materialize()?;
    let seq1 = if args.len() >= 2 { args[1].materialize()? } else { vec![] };
    let materialized_args = if seq1.is_empty() { vec![seq0] } else { vec![seq0, seq1] };
    let result = resolve_uri_fn(ctx, &materialized_args)?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn normalize_unicode_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "normalize-unicode expects at most one string argument"));
    }
    if args.len() == 2 && args[1].len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "normalize-unicode expects at most one form argument"));
    }
    if args[0].is_empty() {
        return Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(String::new()))]);
    }
    let s = item_to_string(&args[0]);
    let form = if args.len() == 2 {
        if args[1].is_empty() { String::new() } else { item_to_string(&args[1]).trim().to_uppercase() }
    } else {
        "NFC".to_string()
    };
    let out = match form.as_str() {
        "" => s,
        "NFC" => s.nfc().collect::<String>(),
        "NFD" => s.nfd().collect::<String>(),
        "NFKC" => s.nfkc().collect::<String>(),
        "NFKD" => s.nfkd().collect::<String>(),
        "FULLY-NORMALIZED" => {
            return Err(Error::from_code(ErrorCode::FOCH0003, "FULLY-NORMALIZED is not supported"));
        }
        _ => {
            return Err(Error::from_code(ErrorCode::FOCH0003, "unsupported normalization form"));
        }
    };
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(out))])
}

pub(super) fn normalize_unicode_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq0 = args[0].materialize()?;
    let seq1 = if args.len() >= 2 { args[1].materialize()? } else { vec![] };
    let materialized_args = if seq1.is_empty() { vec![seq0] } else { vec![seq0, seq1] };
    let result = normalize_unicode_fn(ctx, &materialized_args)?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn doc_available_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(false))]));
    }
    let uri = item_to_string(&seq);
    let result = if let Some(nr) = &ctx.dyn_ctx.node_resolver {
        match nr.doc_node(&uri) {
            Ok(Some(_)) => vec![XdmItem::Atomic(XdmAtomicValue::Boolean(true))],
            Ok(None) => vec![XdmItem::Atomic(XdmAtomicValue::Boolean(false))],
            Err(_e) => vec![XdmItem::Atomic(XdmAtomicValue::Boolean(false))],
        }
    } else {
        vec![XdmItem::Atomic(XdmAtomicValue::Boolean(false))]
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn doc_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    let uri = item_to_string(&seq);
    if let Some(nr) = &ctx.dyn_ctx.node_resolver {
        match nr.doc_node(&uri) {
            Ok(Some(n)) => return Ok(XdmSequenceStream::from_vec(vec![XdmItem::Node(n)])),
            Ok(None) => {
                return Err(Error::from_code(ErrorCode::FODC0005, "document not available"));
            }
            Err(_e) => {
                return Err(Error::from_code(ErrorCode::FODC0005, "error retrieving document"));
            }
        }
    }
    Err(Error::from_code(ErrorCode::FODC0005, "no node resolver configured for fn:doc"))
}

pub(super) fn collection_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "collection() accepts at most one argument"));
    }
    if args.first().is_some_and(|seq| seq.len() > 1) {
        return Err(Error::from_code(ErrorCode::FORG0006, "collection() argument must be a single string"));
    }
    let uri_opt = if args.is_empty() || args.first().is_some_and(|seq| seq.is_empty()) {
        None
    } else {
        Some(item_to_string(&args[0]))
    };
    let resolver = ctx.dyn_ctx.node_resolver.as_ref().ok_or_else(|| {
        if uri_opt.is_some() {
            Error::from_code(ErrorCode::FODC0004, "no collection resolver available")
        } else {
            Error::from_code(ErrorCode::FODC0002, "default collection is undefined")
        }
    })?;
    let nodes = resolver.collection_nodes(uri_opt.as_deref())?;
    Ok(nodes.into_iter().map(XdmItem::Node).collect())
}

pub(super) fn collection_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq = if args.is_empty() { vec![] } else { args[0].materialize()? };
    let materialized_args = if seq.is_empty() { vec![] } else { vec![seq] };
    let result = collection_fn(ctx, &materialized_args)?;
    Ok(XdmSequenceStream::from_vec(result))
}
