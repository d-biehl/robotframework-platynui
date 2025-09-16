use super::common::item_to_string;
use crate::engine::runtime::{CallCtx, Error, ErrorCode};
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence};
use unicode_normalization::UnicodeNormalization;

pub(super) fn default_collation_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    _args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let uri = if let Some(c) = &ctx.default_collation {
        c.uri().to_string()
    } else if let Some(s) = &ctx.static_ctx.default_collation {
        s.clone()
    } else {
        crate::engine::collation::CODEPOINT_URI.to_string()
    };
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(uri))])
}

pub(super) fn static_base_uri_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    _args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if let Some(b) = &ctx.static_ctx.base_uri {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(b.clone()))])
    } else {
        Ok(vec![])
    }
}

pub(super) fn root_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let node_opt = if args.is_empty() {
        ctx.dyn_ctx.context_item.clone()
    } else {
        args[0].first().cloned()
    };
    let Some(item) = node_opt else {
        return Ok(vec![]);
    };
    match item {
        XdmItem::Node(n) => {
            let mut cur = n.clone();
            let mut p = cur.parent();
            while let Some(pp) = p {
                cur = pp.clone();
                p = cur.parent();
            }
            Ok(vec![XdmItem::Node(cur)])
        }
        _ => Err(Error::from_code(
            ErrorCode::XPTY0004,
            "root() expects node()",
        )),
    }
}

pub(super) fn base_uri_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let node_opt = if args.is_empty() {
        ctx.dyn_ctx.context_item.clone()
    } else {
        args[0].first().cloned()
    };
    let Some(item) = node_opt else {
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
        _ => Err(Error::from_code(
            ErrorCode::XPTY0004,
            "base-uri() expects node()",
        )),
    }
}

pub(super) fn document_uri_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let node_opt = if args.is_empty() {
        ctx.dyn_ctx.context_item.clone()
    } else {
        args[0].first().cloned()
    };
    let Some(item) = node_opt else {
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
        _ => Err(Error::from_code(
            ErrorCode::XPTY0004,
            "document-uri() expects node()",
        )),
    }
}

pub(super) fn lang_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(false))]);
    }
    let test = item_to_string(&args[0]).to_ascii_lowercase();
    let Some(XdmItem::Node(mut n)) = ctx.dyn_ctx.context_item.clone() else {
        return Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(false))]);
    };
    let mut lang_val: Option<String> = None;
    loop {
        for a in n.attributes() {
            if let Some(q) = a.name() {
                let is_xml_lang = q.local == "lang"
                    && (q.prefix.as_deref() == Some("xml")
                        || q.ns_uri.as_deref() == Some(crate::consts::XML_URI));
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

pub(super) fn encode_for_uri_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let s = item_to_string(&args[0]);
    fn is_unreserved(ch: char) -> bool {
        ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '~' | '/')
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
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(out))])
}

pub(super) fn nilled_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    let item = &args[0][0];
    let XdmItem::Node(n) = item else {
        return Ok(vec![]);
    };
    if !matches!(n.kind(), crate::model::NodeKind::Element) {
        return Ok(vec![]);
    }
    let mut is_nilled = false;
    for a in n.attributes() {
        if let Some(q) = a.name() {
            let is_xsi_nil = q.local == "nil"
                && (q.prefix.as_deref() == Some("xsi")
                    || q.ns_uri.as_deref() == Some(crate::consts::XSI));
            if is_xsi_nil {
                let v = a.string_value().trim().to_ascii_lowercase();
                if v == "true" || v == "1" {
                    is_nilled = true;
                    break;
                }
            }
        }
    }
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(is_nilled))])
}

pub(super) fn iri_to_uri_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let s = item_to_string(&args[0]);
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
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(out))])
}

pub(super) fn escape_html_uri_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(String::new()))]);
    }
    let s = item_to_string(&args[0]);
    let mut out = String::new();
    for ch in s.chars() {
        if ch == ' ' {
            out.push_str("%20");
        } else {
            out.push(ch);
        }
    }
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(out))])
}

pub(super) fn resolve_uri_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    let rel = item_to_string(&args[0]);
    let is_abs = rel.contains(":") || rel.starts_with('/') || rel.starts_with("#");
    if is_abs {
        return Ok(vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(rel))]);
    }
    let base = if args.len() == 2 && !args[1].is_empty() {
        Some(item_to_string(&args[1]))
    } else {
        ctx.static_ctx.base_uri.clone()
    };
    let Some(mut baseu) = base else {
        return Ok(vec![]);
    };
    if !baseu.ends_with('/') {
        if let Some(idx) = baseu.rfind('/') {
            baseu.truncate(idx + 1);
        } else {
            baseu.push('/');
        }
    }
    let joined = format!("{}{}", baseu, rel);
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(joined))])
}

pub(super) fn normalize_unicode_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(String::new()))]);
    }
    let s = item_to_string(&args[0]);
    let form = if args.len() == 2 {
        item_to_string(&args[1]).to_uppercase()
    } else {
        "NFC".to_string()
    };
    let out = match form.as_str() {
        "NFC" => s.nfc().collect::<String>(),
        "NFD" => s.nfd().collect::<String>(),
        "NFKC" => s.nfkc().collect::<String>(),
        "NFKD" => s.nfkd().collect::<String>(),
        _ => {
            return Err(Error::from_code(
                ErrorCode::FORG0001,
                "invalid normalization form",
            ));
        }
    };
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(out))])
}

pub(super) fn doc_available_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(false))]);
    }
    let uri = item_to_string(&args[0]);
    if let Some(nr) = &ctx.dyn_ctx.node_resolver {
        match nr.doc_node(&uri) {
            Ok(Some(_)) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(true))]),
            Ok(None) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(false))]),
            Err(_e) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(false))]),
        }
    } else {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(false))])
    }
}

pub(super) fn doc_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    let uri = item_to_string(&args[0]);
    if let Some(nr) = &ctx.dyn_ctx.node_resolver {
        match nr.doc_node(&uri) {
            Ok(Some(n)) => return Ok(vec![XdmItem::Node(n)]),
            Ok(None) => {
                return Err(Error::from_code(
                    ErrorCode::FODC0005,
                    "document not available",
                ));
            }
            Err(_e) => {
                return Err(Error::from_code(
                    ErrorCode::FODC0005,
                    "error retrieving document",
                ));
            }
        }
    }
    Err(Error::from_code(
        ErrorCode::FODC0005,
        "no node resolver configured for fn:doc",
    ))
}

pub(super) fn collection_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let uri = if args.len() == 1 && !args[0].is_empty() {
        Some(item_to_string(&args[0]))
    } else {
        None
    };
    if let Some(nr) = &ctx.dyn_ctx.node_resolver {
        let nodes = nr.collection_nodes(uri.as_deref())?;
        return Ok(nodes.into_iter().map(XdmItem::Node).collect());
    }
    Ok(vec![])
}
