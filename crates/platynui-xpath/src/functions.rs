//! Function families per XQuery and XPath Functions and Operators.
//! Minimal default registry with a few core functions to bootstrap evaluator tests.
//!
//! Registration conventions:
//! - Prefer a single registration per XPath function using arity ranges via `register_ns_range`.
//!   Dispatch inside the closure based on `args.len()` when needed (e.g., optional parameters).
//! - Use `register_ns_variadic` for truly variadic families (e.g., `fn:concat` with min arity).
//! - Keep helpers suffixed with `_default` to share logic across arities and reduce duplication.

use crate::runtime::{CallCtx, Error, ErrorCode, FunctionRegistry};
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence};
use base64::Engine as _; // for STANDARD.decode
use chrono::{
    DateTime as ChronoDateTime, Datelike, FixedOffset as ChronoFixedOffset, NaiveDate, NaiveTime,
    TimeZone, Timelike,
};
use unicode_normalization::UnicodeNormalization;

const FNS: &str = "http://www.w3.org/2005/xpath-functions";
const XS: &str = "http://www.w3.org/2001/XMLSchema";

fn ebv<N>(seq: &XdmSequence<N>) -> Result<bool, Error> {
    match seq.len() {
        0 => Ok(false),
        1 => match &seq[0] {
            XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => Ok(*b),
            XdmItem::Atomic(XdmAtomicValue::String(s)) => Ok(!s.is_empty()),
            XdmItem::Atomic(XdmAtomicValue::Integer(i)) => Ok(*i != 0),
            XdmItem::Atomic(XdmAtomicValue::Decimal(d)) => Ok(*d != 0.0),
            XdmItem::Atomic(XdmAtomicValue::Double(d)) => Ok(*d != 0.0 && !d.is_nan()),
            XdmItem::Atomic(XdmAtomicValue::Float(f)) => Ok(*f != 0.0 && !f.is_nan()),
            XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => Ok(!s.is_empty()),
            XdmItem::Atomic(_) => Err(Error::dynamic(
                ErrorCode::FORG0006,
                "EBV for this atomic type not supported yet",
            )),
            XdmItem::Node(_) => Ok(true),
        },
        _ => Err(Error::dynamic(
            ErrorCode::FORG0006,
            "EBV of sequence with more than one item",
        )),
    }
}

// Default implementations for collation-aware string predicates (2|3-arity)
fn contains_default<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    s_seq: &XdmSequence<N>,
    sub_seq: &XdmSequence<N>,
    collation_uri: Option<&str>,
) -> Result<XdmSequence<N>, Error> {
    let s = item_to_string(s_seq);
    let sub = item_to_string(sub_seq);
    let uri_opt = collation_uri.and_then(|u| if u.is_empty() { None } else { Some(u) });
    let k =
        crate::collation::resolve_collation(&ctx.dyn_ctx, ctx.default_collation.as_ref(), uri_opt)?;
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
    let k =
        crate::collation::resolve_collation(&ctx.dyn_ctx, ctx.default_collation.as_ref(), uri_opt)?;
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
    let k =
        crate::collation::resolve_collation(&ctx.dyn_ctx, ctx.default_collation.as_ref(), uri_opt)?;
    let c = k.as_trait();
    let b = c.key(&s).ends_with(&c.key(&sub));
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))])
}

pub fn default_function_registry<N: 'static + Send + Sync + crate::model::XdmNode + Clone>()
-> FunctionRegistry<N> {
    let mut reg: FunctionRegistry<N> = FunctionRegistry::new();

    // ===== Core booleans =====
    reg.register_ns(FNS, "true", 0, |_ctx, _args| {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(true))])
    });
    reg.register_ns(FNS, "false", 0, |_ctx, _args| {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(false))])
    });
    // data() / data($seq)
    reg.register_ns_range(FNS, "data", 0, Some(1), |ctx, args| match args.len() {
        0 => data_default(ctx, None),
        1 => data_default(ctx, Some(&args[0])),
        _ => unreachable!("registry guarantees arity in range"),
    });
    reg.register_ns(FNS, "not", 1, |_ctx, args| {
        let b = ebv(&args[0])?;
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(!b))])
    });
    // boolean($arg)
    reg.register_ns(FNS, "boolean", 1, |_ctx, args| {
        let b = ebv(&args[0])?;
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))])
    });

    // ===== Numeric core =====
    reg.register_ns_range(FNS, "number", 0, Some(1), |ctx, args| match args.len() {
        0 => number_default(ctx, None),
        1 => number_default(ctx, Some(&args[0])),
        _ => unreachable!("registry guarantees arity in range"),
    });

    // ===== String family =====
    reg.register_ns_range(FNS, "string", 0, Some(1), |ctx, args| match args.len() {
        0 => string_default(ctx, None),
        1 => string_default(ctx, Some(&args[0])),
        _ => unreachable!("registry guarantees arity in range"),
    });
    reg.register_ns(FNS, "string-length", 1, |_ctx, args| {
        let s = item_to_string(&args[0]);
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
            s.chars().count() as i64,
        ))])
    });
    // Minimal constructor helper to allow tests to build untypedAtomic values.
    // XPath 2.0 doesn't define a direct fn:untypedAtomic constructor, but test
    // scenarios use it as a placeholder for producing xs:untypedAtomic.
    reg.register_ns(FNS, "untypedAtomic", 1, |_ctx, args| {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(
            item_to_string(&args[0]),
        ))])
    });
    reg.register_ns_variadic(FNS, "concat", 2, |_ctx, args| {
        let mut out = String::new();
        for a in args {
            out.push_str(&item_to_string(a));
        }
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(out))])
    });
    // string-to-codepoints($arg as xs:string?) as xs:integer*
    reg.register_ns(FNS, "string-to-codepoints", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        let s = item_to_string(&args[0]);
        let mut out: XdmSequence<N> = Vec::with_capacity(s.chars().count());
        for ch in s.chars() {
            out.push(XdmItem::Atomic(XdmAtomicValue::Integer(ch as u32 as i64)));
        }
        Ok(out)
    });
    // codepoints-to-string($arg as xs:integer*) as xs:string
    reg.register_ns(FNS, "codepoints-to-string", 1, |_ctx, args| {
        // Accept zero or more integers; any non-integer item is a type error.
        let mut s = String::new();
        for it in &args[0] {
            match it {
                XdmItem::Atomic(XdmAtomicValue::Integer(i)) => {
                    let v = *i as i64;
                    if v < 0 || v > 0x10FFFF {
                        return Err(Error::dynamic(ErrorCode::FORG0001, "invalid code point"));
                    }
                    let u = v as u32;
                    if let Some(c) = char::from_u32(u) {
                        s.push(c);
                    } else {
                        return Err(Error::dynamic(ErrorCode::FORG0001, "invalid code point"));
                    }
                }
                _ => {
                    return Err(Error::dynamic(
                        ErrorCode::XPTY0004,
                        "codepoints-to-string expects xs:integer*",
                    ));
                }
            }
        }
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(s))])
    });
    // contains/starts-with/ends-with (2 or 3 args, collation-aware)
    reg.register_ns_range(FNS, "contains", 2, Some(3), |ctx, args| {
        let uri_opt = if args.len() == 3 {
            Some(item_to_string(&args[2]))
        } else {
            None
        };
        contains_default(ctx, &args[0], &args[1], uri_opt.as_deref())
    });
    reg.register_ns_range(FNS, "starts-with", 2, Some(3), |ctx, args| {
        let uri_opt = if args.len() == 3 {
            Some(item_to_string(&args[2]))
        } else {
            None
        };
        starts_with_default(ctx, &args[0], &args[1], uri_opt.as_deref())
    });
    reg.register_ns_range(FNS, "ends-with", 2, Some(3), |ctx, args| {
        let uri_opt = if args.len() == 3 {
            Some(item_to_string(&args[2]))
        } else {
            None
        };
        ends_with_default(ctx, &args[0], &args[1], uri_opt.as_deref())
    });
    reg.register_ns_range(FNS, "substring", 2, Some(3), |_ctx, args| {
        let s = item_to_string(&args[0]);
        let start_raw = to_number(&args[1])?;
        let out = if args.len() == 2 {
            substring_default(&s, start_raw, None)
        } else {
            let len_raw = to_number(&args[2])?;
            substring_default(&s, start_raw, Some(len_raw))
        };
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(out))])
    });

    // substring-before/after
    reg.register_ns(FNS, "substring-before", 2, |_ctx, args| {
        let s = item_to_string(&args[0]);
        let sub = item_to_string(&args[1]);
        if sub.is_empty() || s.is_empty() {
            return Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(String::new()))]);
        }
        if let Some(idx) = s.find(&sub) {
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(
                s[..idx].to_string(),
            ))])
        } else {
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(String::new()))])
        }
    });
    reg.register_ns(FNS, "substring-after", 2, |_ctx, args| {
        let s = item_to_string(&args[0]);
        let sub = item_to_string(&args[1]);
        if sub.is_empty() {
            return Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(s))]);
        }
        if let Some(idx) = s.find(&sub) {
            let after = &s[idx + sub.len()..];
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(
                after.to_string(),
            ))])
        } else {
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(String::new()))])
        }
    });

    // normalize-space() 0 or 1 arg
    reg.register_ns_range(FNS, "normalize-space", 0, Some(1), |ctx, args| {
        Ok(match args.len() {
            0 => normalize_space_default(ctx, None),
            1 => normalize_space_default(ctx, Some(&args[0])),
            _ => unreachable!("registry guarantees arity in range"),
        })
    });

    // translate($s, $map, $trans)
    reg.register_ns(FNS, "translate", 3, |_ctx, args| {
        let s = item_to_string(&args[0]);
        let map = item_to_string(&args[1]);
        let trans = item_to_string(&args[2]);
        use std::collections::HashMap;
        let mut table: HashMap<char, Option<char>> = HashMap::new();
        let mut trans_iter = trans.chars();
        for m in map.chars() {
            use std::collections::hash_map::Entry;
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
                } // else drop char
            } else {
                out.push(ch);
            }
        }
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(out))])
    });

    // lower-case / upper-case
    reg.register_ns(FNS, "lower-case", 1, |_ctx, args| {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(
            item_to_string(&args[0]).to_lowercase(),
        ))])
    });
    reg.register_ns(FNS, "upper-case", 1, |_ctx, args| {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(
            item_to_string(&args[0]).to_uppercase(),
        ))])
    });

    // string-join($seq, $sep)
    reg.register_ns(FNS, "string-join", 2, |_ctx, args| {
        let sep = item_to_string(&args[1]);
        let mut parts: Vec<String> = Vec::new();
        for it in &args[0] {
            match it {
                XdmItem::Atomic(a) => parts.push(as_string(a)),
                XdmItem::Node(n) => parts.push(n.string_value()),
            }
        }
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(
            parts.join(&sep),
        ))])
    });

    // ===== Node name functions (Task 72) =====
    // node-name($arg as node()?) as xs:QName?
    reg.register_ns(FNS, "node-name", 1, |ctx, args| {
        node_name_default(ctx, Some(&args[0]))
    });
    // name()/name($arg)
    reg.register_ns_range(FNS, "name", 0, Some(1), |ctx, args| {
        Ok(match args.len() {
            0 => name_default(ctx, None),
            1 => name_default(ctx, Some(&args[0])),
            _ => unreachable!("registry guarantees arity in range"),
        })
    });
    // local-name()/local-name($arg)
    reg.register_ns_range(FNS, "local-name", 0, Some(1), |ctx, args| {
        Ok(match args.len() {
            0 => local_name_default(ctx, None),
            1 => local_name_default(ctx, Some(&args[0])),
            _ => unreachable!("registry guarantees arity in range"),
        })
    });
    // namespace-uri()/namespace-uri($arg)
    reg.register_ns_range(FNS, "namespace-uri", 0, Some(1), |ctx, args| {
        match args.len() {
            0 => namespace_uri_default(ctx, None),
            1 => namespace_uri_default(ctx, Some(&args[0])),
            _ => unreachable!("registry guarantees arity in range"),
        }
    });

    // ===== QName / Namespace functions (Task 29) =====
    // fn:QName($namespaceURI as xs:string?, $qname as xs:string) as xs:QName
    reg.register_ns(FNS, "QName", 2, |_ctx, args| {
        if args[0].is_empty() {
            return Err(Error::dynamic(
                ErrorCode::FORG0001,
                "QName requires namespace string (use '' for none)",
            ));
        }
        // arg0: namespace string (possibly empty) as atomic
        let ns = match &args[0][0] {
            XdmItem::Atomic(XdmAtomicValue::String(s))
            | XdmItem::Atomic(XdmAtomicValue::AnyUri(s)) => s.clone(),
            _ => String::new(),
        };
        if args[1].is_empty() {
            return Err(Error::dynamic(
                ErrorCode::FORG0001,
                "QName requires lexical QName",
            ));
        }
        let qn_lex = match &args[1][0] {
            XdmItem::Atomic(XdmAtomicValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::dynamic(
                    ErrorCode::FORG0001,
                    "QName lexical must be string",
                ));
            }
        };
        let (prefix_opt, local) = parse_qname_lexical(&qn_lex)
            .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid QName lexical"))?;
        let ns_uri = if ns.is_empty() { None } else { Some(ns) };
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::QName {
            ns_uri,
            prefix: prefix_opt,
            local,
        })])
    });

    // Helper: collect in-scope namespaces for an element node (self + ancestors), first prefix wins; always include xml
    fn inscope_for<N: crate::model::XdmNode + Clone>(
        mut n: N,
    ) -> std::collections::HashMap<String, String> {
        use crate::model::NodeKind;
        let mut map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        // collect upwards
        loop {
            if matches!(n.kind(), NodeKind::Element) {
                for ns in n.namespaces() {
                    if let Some(q) = ns.name() {
                        if let (Some(p), Some(uri)) = (q.prefix, q.ns_uri) {
                            map.entry(p).or_insert(uri);
                        }
                    }
                }
            }
            if let Some(p) = n.parent() {
                n = p;
            } else {
                break;
            }
        }
        // implicit xml binding
        map.entry("xml".to_string())
            .or_insert("http://www.w3.org/XML/1998/namespace".to_string());
        map
    }

    // fn:resolve-QName($qname as xs:string?, $element as element()) as xs:QName?
    reg.register_ns(FNS, "resolve-QName", 2, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        // qname lexical
        let s = match &args[0][0] {
            XdmItem::Atomic(XdmAtomicValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::dynamic(
                    ErrorCode::FORG0001,
                    "resolve-QName requires string",
                ));
            }
        };
        // element node
        let enode = match &args[1][0] {
            XdmItem::Node(n) => n.clone(),
            _ => {
                return Err(Error::dynamic(
                    ErrorCode::XPTY0004,
                    "resolve-QName requires element()",
                ));
            }
        };
        let (prefix_opt, local) = parse_qname_lexical(&s)
            .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid QName lexical"))?;
        let ns_uri = match &prefix_opt {
            None => None,
            Some(p) => inscope_for(enode).get(p).cloned(),
        };
        if prefix_opt.is_some() && ns_uri.is_none() {
            return Err(Error::dynamic(ErrorCode::FORG0001, "unknown prefix"));
        }
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::QName {
            ns_uri,
            prefix: prefix_opt,
            local,
        })])
    });

    // fn:namespace-uri-from-QName($arg as xs:QName?) as xs:anyURI?
    reg.register_ns(FNS, "namespace-uri-from-QName", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        match &args[0][0] {
            XdmItem::Atomic(XdmAtomicValue::QName { ns_uri, .. }) => {
                if let Some(uri) = ns_uri {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(uri.clone()))])
                } else {
                    Ok(vec![])
                }
            }
            _ => Err(Error::dynamic(
                ErrorCode::XPTY0004,
                "namespace-uri-from-QName expects xs:QName",
            )),
        }
    });

    // fn:local-name-from-QName($arg as xs:QName?) as xs:NCName?
    reg.register_ns(FNS, "local-name-from-QName", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        match &args[0][0] {
            XdmItem::Atomic(XdmAtomicValue::QName { local, .. }) => {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::NCName(local.clone()))])
            }
            _ => Err(Error::dynamic(
                ErrorCode::XPTY0004,
                "local-name-from-QName expects xs:QName",
            )),
        }
    });

    // fn:prefix-from-QName($arg as xs:QName?) as xs:NCName?
    reg.register_ns(FNS, "prefix-from-QName", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        match &args[0][0] {
            XdmItem::Atomic(XdmAtomicValue::QName { prefix, .. }) => {
                if let Some(p) = prefix {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::NCName(p.clone()))])
                } else {
                    Ok(vec![])
                }
            }
            _ => Err(Error::dynamic(
                ErrorCode::XPTY0004,
                "prefix-from-QName expects xs:QName",
            )),
        }
    });

    // fn:namespace-uri-for-prefix($prefix as xs:string?, $element as element()) as xs:anyURI?
    reg.register_ns(FNS, "namespace-uri-for-prefix", 2, |_ctx, args| {
        // If empty prefix argument → return default element namespace; not yet supported (returns empty)
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        let p = match &args[0][0] {
            XdmItem::Atomic(XdmAtomicValue::String(s)) => s.clone(),
            _ => return Err(Error::dynamic(ErrorCode::FORG0001, "prefix must be string")),
        };
        let enode = match &args[1][0] {
            XdmItem::Node(n) => n.clone(),
            _ => {
                return Err(Error::dynamic(
                    ErrorCode::XPTY0004,
                    "namespace-uri-for-prefix requires element()",
                ));
            }
        };
        if p.is_empty() {
            return Ok(vec![]);
        }
        let map = inscope_for(enode);
        if let Some(uri) = map.get(&p) {
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(uri.clone()))])
        } else {
            Ok(vec![])
        }
    });

    // fn:in-scope-prefixes($element as element()) as xs:NCName*
    reg.register_ns(FNS, "in-scope-prefixes", 1, |_ctx, args| {
        let enode = match &args[0][0] {
            XdmItem::Node(n) => n.clone(),
            _ => {
                return Err(Error::dynamic(
                    ErrorCode::XPTY0004,
                    "in-scope-prefixes requires element()",
                ));
            }
        };
        let map = inscope_for(enode);
        let mut out: Vec<XdmItem<_>> = Vec::with_capacity(map.len());
        for k in map.keys() {
            out.push(XdmItem::Atomic(XdmAtomicValue::NCName(k.clone())));
        }
        Ok(out)
    });

    // fn:namespace-uri($arg as node()?) as xs:anyURI?
    // If omitted, use context item. Returns empty sequence if node has no namespace or is unnamed.
    reg.register_ns_range(FNS, "namespace-uri", 0, Some(1), |ctx, args| {
        let node_opt = if args.is_empty() {
            ctx.dyn_ctx.context_item.clone()
        } else {
            args[0].get(0).cloned()
        };
        let Some(item) = node_opt else {
            return Ok(vec![]);
        };
        match item {
            XdmItem::Node(n) => {
                if let Some(q) = n.name() {
                    if let Some(uri) = q.ns_uri {
                        return Ok(vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(uri))]);
                    }
                    if let Some(pref) = q.prefix {
                        // Resolve prefix via in-scope namespaces (self + ancestors)
                        let map = inscope_for(n.clone());
                        if let Some(uri) = map.get(&pref) {
                            return Ok(vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(uri.clone()))]);
                        }
                    }
                }
                Ok(vec![])
            }
            _ => Err(Error::dynamic(
                ErrorCode::XPTY0004,
                "namespace-uri() expects node()",
            )),
        }
    });

    // ===== Numeric family =====
    reg.register_ns(FNS, "abs", 1, |_ctx, args| Ok(num_unary(args, |n| n.abs())));
    reg.register_ns(FNS, "floor", 1, |_ctx, args| {
        Ok(num_unary(args, |n| n.floor()))
    });
    reg.register_ns(FNS, "ceiling", 1, |_ctx, args| {
        Ok(num_unary(args, |n| n.ceil()))
    });
    reg.register_ns_range(FNS, "round", 1, Some(2), |_ctx, args| match args.len() {
        1 => round_default(&args[0], None),
        2 => round_default(&args[0], Some(&args[1])),
        _ => unreachable!("registry guarantees arity in range"),
    });
    reg.register_ns_range(
        FNS,
        "round-half-to-even",
        1,
        Some(2),
        |_ctx, args| match args.len() {
            1 => round_half_to_even_default(&args[0], None),
            2 => round_half_to_even_default(&args[0], Some(&args[1])),
            _ => unreachable!("registry guarantees arity in range"),
        },
    );
    reg.register_ns_range(FNS, "sum", 1, Some(2), |_ctx, args| match args.len() {
        1 => sum_default(&args[0], None),
        2 => sum_default(&args[0], Some(&args[1])),
        _ => unreachable!("registry guarantees arity in range"),
    });
    reg.register_ns(FNS, "avg", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        let mut kind = NumericKind::Integer;
        let mut int_acc: i128 = 0;
        let mut dec_acc: f64 = 0.0;
        let mut use_int_acc = true;
        let mut count: i64 = 0;
        for it in &args[0] {
            let XdmItem::Atomic(a) = it else {
                return Err(Error::dynamic(
                    ErrorCode::XPTY0004,
                    "avg on non-atomic item",
                ));
            };
            if let Some((nk, num)) = classify_numeric(a)? {
                if nk == NumericKind::Double && num.is_nan() {
                    return Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(f64::NAN))]);
                }
                kind = kind.promote(nk);
                match nk {
                    NumericKind::Integer if use_int_acc => {
                        if let Some(i) = a_as_i128(a) {
                            if let Some(v) = int_acc.checked_add(i) {
                                int_acc = v;
                            } else {
                                use_int_acc = false;
                                dec_acc = int_acc as f64 + i as f64;
                                kind = kind.promote(NumericKind::Decimal);
                            }
                        }
                    }
                    _ => {
                        if use_int_acc {
                            dec_acc = int_acc as f64;
                            use_int_acc = false;
                        }
                        dec_acc += num;
                    }
                }
                count += 1;
            } else {
                return Err(Error::dynamic(
                    ErrorCode::XPTY0004,
                    "avg requires numeric values",
                ));
            }
        }
        if count == 0 {
            return Ok(vec![]);
        }
        let total = if use_int_acc && matches!(kind, NumericKind::Integer) {
            int_acc as f64
        } else {
            dec_acc
        };
        // Division: result type follows promotion rules; integer-only -> decimal division? XPath avg over integers returns decimal.
        let mean = total / (count as f64);
        let out = match kind {
            NumericKind::Integer | NumericKind::Decimal => XdmAtomicValue::Decimal(mean),
            NumericKind::Float => XdmAtomicValue::Float(mean as f32),
            NumericKind::Double => XdmAtomicValue::Double(mean),
        };
        Ok(vec![XdmItem::Atomic(out)])
    });

    // ===== Sequence family =====
    reg.register_ns(FNS, "empty", 1, |_ctx, args| {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(
            args[0].is_empty(),
        ))])
    });
    reg.register_ns(FNS, "exists", 1, |_ctx, args| {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(
            !args[0].is_empty(),
        ))])
    });
    reg.register_ns(FNS, "count", 1, |_ctx, args| {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
            args[0].len() as i64,
        ))])
    });
    // Cardinality helpers
    // exactly-one($arg) => returns the item, else FORG0005
    reg.register_ns(FNS, "exactly-one", 1, |_ctx, args| {
        if args[0].len() != 1 {
            return Err(Error::dynamic(
                ErrorCode::FORG0005,
                "exactly-one requires a sequence of length 1",
            ));
        }
        Ok(args[0].clone())
    });
    // one-or-more($arg) => returns the sequence, else FORG0004
    reg.register_ns(FNS, "one-or-more", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Err(Error::dynamic(
                ErrorCode::FORG0004,
                "one-or-more requires at least one item",
            ));
        }
        Ok(args[0].clone())
    });
    // zero-or-one($arg) => returns as-is, else FORG0004
    reg.register_ns(FNS, "zero-or-one", 1, |_ctx, args| {
        if args[0].len() > 1 {
            return Err(Error::dynamic(
                ErrorCode::FORG0004,
                "zero-or-one requires at most one item",
            ));
        }
        Ok(args[0].clone())
    });
    reg.register_ns(FNS, "reverse", 1, |_ctx, args| {
        let s: XdmSequence<N> = args[0].iter().cloned().rev().collect();
        Ok(s)
    });
    reg.register_ns_range(FNS, "subsequence", 2, Some(3), |_ctx, args| {
        let start_raw = to_number(&args[1])?;
        if args.len() == 2 {
            subsequence_default(&args[0], start_raw, None)
        } else {
            let len_raw = to_number(&args[2])?;
            subsequence_default(&args[0], start_raw, Some(len_raw))
        }
    });

    // distinct-values($seq as xs:anyAtomicType*) and 2-arg collation variant.
    // Spec-aligned semantics (subset):
    // - Non-atomic items raise XPTY0004.
    // - Numeric equality: different numeric types with same value collapse (incl. -0/+0).
    // - NaN values: all NaN collapse to a single NaN representative.
    // - String-like types honor optional collation (second argument) for equivalence; first representative kept.
    // - untypedAtomic treated as string lexical form.
    reg.register_ns_range(FNS, "distinct-values", 1, Some(2), |ctx, args| {
        if args.len() == 1 {
            distinct_values_impl(ctx, &args[0], None)
        } else {
            let uri = item_to_string(&args[1]);
            let k = crate::collation::resolve_collation(
                &ctx.dyn_ctx,
                ctx.default_collation.as_ref(),
                Some(&uri),
            )?;
            distinct_values_impl(ctx, &args[0], Some(k.as_trait()))
        }
    });

    // index-of($seq, $search[, $collation])
    reg.register_ns_range(FNS, "index-of", 2, Some(3), |ctx, args| {
        if args.len() == 2 {
            index_of_default(ctx, &args[0], &args[1], None)
        } else {
            let uri = item_to_string(&args[2]);
            index_of_default(
                ctx,
                &args[0],
                &args[1],
                if uri.is_empty() { None } else { Some(&uri) },
            )
        }
    });

    // insert-before($seq, $pos, $item)
    reg.register_ns(FNS, "insert-before", 3, |_ctx, args| {
        let mut out: XdmSequence<N> = Vec::new();
        let pos = to_number(&args[1])?.floor() as isize; // 1-based
        let insert_at = pos.max(1) as usize;
        let mut i = 1usize;
        for it in &args[0] {
            if i == insert_at {
                out.extend(args[2].iter().cloned());
            }
            out.push(it.clone());
            i += 1;
        }
        if insert_at > args[0].len() {
            out.extend(args[2].iter().cloned());
        }
        Ok(out)
    });

    // remove($seq, $pos)
    reg.register_ns(FNS, "remove", 2, |_ctx, args| {
        let mut out: XdmSequence<N> = Vec::new();
        let pos = to_number(&args[1])?.floor() as isize; // 1-based
        let remove_at = pos.max(1) as usize;
        for (i, it) in args[0].iter().enumerate() {
            if i + 1 != remove_at {
                out.push(it.clone());
            }
        }
        Ok(out)
    });

    // min/max (optional collation)
    reg.register_ns_range(FNS, "min", 1, Some(2), |ctx, args| {
        if args.len() == 1 {
            minmax_impl(ctx, &args[0], None, true)
        } else {
            let uri = item_to_string(&args[1]);
            let k = crate::collation::resolve_collation(
                &ctx.dyn_ctx,
                ctx.default_collation.as_ref(),
                Some(&uri),
            )?;
            minmax_impl(ctx, &args[0], Some(k.as_trait()), true)
        }
    });
    reg.register_ns_range(FNS, "max", 1, Some(2), |ctx, args| {
        if args.len() == 1 {
            minmax_impl(ctx, &args[0], None, false)
        } else {
            let uri = item_to_string(&args[1]);
            let k = crate::collation::resolve_collation(
                &ctx.dyn_ctx,
                ctx.default_collation.as_ref(),
                Some(&uri),
            )?;
            minmax_impl(ctx, &args[0], Some(k.as_trait()), false)
        }
    });

    // ===== Collation-related functions =====
    // compare($A, $B[, $collation]) => -1/0/1, empty if either is empty
    reg.register_ns_range(FNS, "compare", 2, Some(3), |ctx, args| {
        if args.len() == 2 {
            compare_default(ctx, &args[0], &args[1], None)
        } else {
            let uri = item_to_string(&args[2]);
            compare_default(ctx, &args[0], &args[1], Some(&uri))
        }
    });

    // codepoint-equal($A, $B) — empty if either is empty, uses codepoint collation only
    reg.register_ns(FNS, "codepoint-equal", 2, |ctx, args| {
        if args[0].is_empty() || args[1].is_empty() {
            return Ok(vec![]);
        }
        // Reuse atomic_equal_with_collation with codepoint collation; only first items considered.
        let coll = ctx
            .dyn_ctx
            .collations
            .get(crate::collation::CODEPOINT_URI)
            .expect("codepoint collation registered");
        let a_item = args[0].first().cloned();
        let b_item = args[1].first().cloned();
        let eq = if let (Some(XdmItem::Atomic(a)), Some(XdmItem::Atomic(b))) = (a_item, b_item) {
            atomic_equal_with_collation(&a, &b, Some(coll.as_ref()))?
        } else {
            // If nodes appear, fall back to their string values (current simplified behavior)
            let sa = item_to_string(&args[0]);
            let sb = item_to_string(&args[1]);
            sa == sb
        };
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(eq))])
    });

    // deep-equal($A, $B[, $collation]) as xs:boolean
    reg.register_ns_range(FNS, "deep-equal", 2, Some(3), |ctx, args| {
        if args.len() == 2 {
            deep_equal_default(ctx, &args[0], &args[1], None)
        } else {
            let uri = item_to_string(&args[2]);
            deep_equal_default(ctx, &args[0], &args[1], Some(&uri))
        }
    });

    // ===== Regex family =====
    // matches($input, $pattern[, $flags])
    reg.register_ns_range(FNS, "matches", 2, Some(3), |ctx, args| {
        if args.len() == 2 {
            matches_default(ctx, &args[0], &args[1], None)
        } else {
            let flags = item_to_string(&args[2]);
            matches_default(ctx, &args[0], &args[1], Some(&flags))
        }
    });

    // ===== Diagnostics =====
    // error() with 0..=3 arities — delegate to unified handler
    reg.register_ns_range(FNS, "error", 0, Some(3), |_ctx, args| error_default(args));

    // trace($value as item()*, $label as xs:string) as item()*
    // Implementation: currently pass-through without side-effects, optional hook in future.
    reg.register_ns(FNS, "trace", 2, |_ctx, args| Ok(args[0].clone()));

    // ===== Environment / Document / URI helpers (Batch A subset) =====
    // default-collation() as xs:string
    reg.register_ns(FNS, "default-collation", 0, |ctx, _args| {
        let uri = if let Some(c) = &ctx.default_collation {
            c.uri().to_string()
        } else if let Some(s) = &ctx.static_ctx.default_collation {
            s.clone()
        } else {
            crate::collation::CODEPOINT_URI.to_string()
        };
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(uri))])
    });
    // static-base-uri() as xs:anyURI?
    reg.register_ns(FNS, "static-base-uri", 0, |ctx, _args| {
        if let Some(b) = &ctx.static_ctx.base_uri {
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(b.clone()))])
        } else {
            Ok(vec![])
        }
    });
    // root($arg as node()?) as node()?
    reg.register_ns_range(FNS, "root", 0, Some(1), |ctx, args| {
        // Determine node argument or context item
        let node_opt = if args.is_empty() {
            ctx.dyn_ctx.context_item.clone()
        } else {
            args[0].get(0).cloned()
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
            _ => Err(Error::dynamic(ErrorCode::XPTY0004, "root() expects node()")),
        }
    });
    // base-uri($arg as node()?) as xs:anyURI?
    reg.register_ns_range(FNS, "base-uri", 0, Some(1), |ctx, args| {
        let node_opt = if args.is_empty() {
            ctx.dyn_ctx.context_item.clone()
        } else {
            args[0].get(0).cloned()
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
            _ => Err(Error::dynamic(
                ErrorCode::XPTY0004,
                "base-uri() expects node()",
            )),
        }
    });
    // document-uri($arg as node()?) as xs:anyURI?
    reg.register_ns_range(FNS, "document-uri", 0, Some(1), |ctx, args| {
        let node_opt = if args.is_empty() {
            ctx.dyn_ctx.context_item.clone()
        } else {
            args[0].get(0).cloned()
        };
        let Some(item) = node_opt else {
            return Ok(vec![]);
        };
        match item {
            XdmItem::Node(n) => {
                if matches!(n.kind(), crate::model::NodeKind::Document) {
                    if let Some(uri) = n.base_uri() {
                        return Ok(vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(uri))]);
                    }
                }
                Ok(vec![])
            }
            _ => Err(Error::dynamic(
                ErrorCode::XPTY0004,
                "document-uri() expects node()",
            )),
        }
    });
    // lang($test as xs:string?) as xs:boolean
    reg.register_ns(FNS, "lang", 1, |ctx, args| {
        // If arg empty -> false
        if args[0].is_empty() {
            return Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(false))]);
        }
        let test = item_to_string(&args[0]).to_ascii_lowercase();
        // target node = context item
        let Some(XdmItem::Node(mut n)) = ctx.dyn_ctx.context_item.clone() else {
            return Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(false))]);
        };
        // Walk up ancestors to find xml:lang
        let mut lang_val: Option<String> = None;
        loop {
            for a in n.attributes() {
                if let Some(q) = a.name() {
                    let is_xml_lang = q.local == "lang"
                        && (q.prefix.as_deref() == Some("xml")
                            || q.ns_uri.as_deref() == Some("http://www.w3.org/XML/1998/namespace"));
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
    });

    // Helper: simple ASCII NCName check (sufficient for tests)
    fn is_ncname_ascii(s: &str) -> bool {
        if s.is_empty() {
            return false;
        }
        let mut chars = s.chars();
        match chars.next().unwrap() {
            'A'..='Z' | 'a'..='z' | '_' => {}
            _ => return false,
        }
        for ch in chars {
            match ch {
                'A'..='Z' | 'a'..='z' | '_' | '0'..='9' | '-' | '.' => {}
                _ => return false,
            }
        }
        true
    }
    fn topmost_ancestor<N: crate::model::XdmNode + Clone>(mut n: N) -> N {
        while let Some(p) = n.parent() {
            n = p;
        }
        n
    }
    // element-with-id($arg as xs:string*[, $node as node()]) as element()*
    // id($arg as xs:string*[, $node as node()]) as element()*
    // Minimal non-schema-aware behavior: treat attributes named xml:id or unprefixed 'id' as ID values.
    let find_elements_with_id =
        |start_node_opt: Option<XdmItem<N>>, tokens: &std::collections::HashSet<String>| {
            let mut out: XdmSequence<N> = Vec::new();
            let Some(XdmItem::Node(start)) = start_node_opt else {
                return Ok(out);
            };
            let root = topmost_ancestor(start);
            // DFS preserving document order
            let mut stack: Vec<N> = vec![root.clone()];
            while let Some(node) = stack.pop() {
                // push children in reverse to visit in natural order
                let children = node.children();
                for c in children.iter().rev() {
                    stack.push(c.clone());
                }
                use crate::model::NodeKind;
                if matches!(node.kind(), NodeKind::Element) {
                    // Check attributes for xml:id or id
                    let mut has_match = false;
                    for a in node.attributes() {
                        if let Some(q) = a.name() {
                            let is_xml_id = q.local == "id"
                                && (q.prefix.as_deref() == Some("xml")
                                    || q.ns_uri.as_deref()
                                        == Some("http://www.w3.org/XML/1998/namespace"));
                            let is_plain_id =
                                q.local == "id" && q.prefix.is_none() && q.ns_uri.is_none();
                            if is_xml_id || is_plain_id {
                                let v = a.string_value();
                                if tokens.contains(&v) {
                                    has_match = true;
                                    break;
                                }
                            }
                        }
                    }
                    if has_match {
                        out.push(XdmItem::Node(node.clone()));
                    }
                }
            }
            Ok(out)
        };
    reg.register_ns_range(FNS, "id", 1, Some(2), move |ctx, args| {
        // build token set from whitespace-separated tokens in all strings
        let mut tokens: std::collections::HashSet<String> = std::collections::HashSet::new();
        for it in &args[0] {
            let s = match it {
                XdmItem::Atomic(a) => as_string(a),
                XdmItem::Node(n) => n.string_value(),
            };
            let collapsed = collapse_whitespace(&s);
            for t in collapsed.split(' ') {
                if !t.is_empty() && is_ncname_ascii(t) {
                    tokens.insert(t.to_string());
                }
            }
        }
        if tokens.is_empty() {
            return Ok(vec![]);
        }
        let start_node_opt = if args.len() == 2 && !args[1].is_empty() {
            Some(args[1][0].clone())
        } else {
            ctx.dyn_ctx.context_item.clone()
        };
        find_elements_with_id(start_node_opt, &tokens)
    });
    reg.register_ns_range(FNS, "element-with-id", 1, Some(2), move |ctx, args| {
        // same tokenization as id()
        let mut tokens: std::collections::HashSet<String> = std::collections::HashSet::new();
        for it in &args[0] {
            let s = match it {
                XdmItem::Atomic(a) => as_string(a),
                XdmItem::Node(n) => n.string_value(),
            };
            let collapsed = collapse_whitespace(&s);
            for t in collapsed.split(' ') {
                if !t.is_empty() && is_ncname_ascii(t) {
                    tokens.insert(t.to_string());
                }
            }
        }
        if tokens.is_empty() {
            return Ok(vec![]);
        }
        let start_node_opt = if args.len() == 2 && !args[1].is_empty() {
            Some(args[1][0].clone())
        } else {
            ctx.dyn_ctx.context_item.clone()
        };
        find_elements_with_id(start_node_opt, &tokens)
    });

    // idref($arg as xs:string*[, $node as node()]) as node()*
    // Minimal non-schema-aware behavior: treat all attributes as potential IDREF(S) holders.
    reg.register_ns_range(FNS, "idref", 1, Some(2), |ctx, args| {
        // Candidate IDs are the strings in $arg that are NCName
        let mut ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        for it in &args[0] {
            let s = match it {
                XdmItem::Atomic(a) => as_string(a),
                XdmItem::Node(n) => n.string_value(),
            };
            if is_ncname_ascii(&s) {
                ids.insert(s);
            }
        }
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let start_node_opt = if args.len() == 2 && !args[1].is_empty() {
            Some(args[1][0].clone())
        } else {
            ctx.dyn_ctx.context_item.clone()
        };
        let Some(XdmItem::Node(start)) = start_node_opt else {
            return Ok(vec![]);
        };
        let root = topmost_ancestor(start);
        let mut out: XdmSequence<N> = Vec::new();
        // DFS preserving document order
        let mut stack: Vec<N> = vec![root.clone()];
        use crate::model::NodeKind;
        while let Some(node) = stack.pop() {
            // push children in reverse to visit in order
            let children = node.children();
            for c in children.iter().rev() {
                stack.push(c.clone());
            }
            if matches!(node.kind(), NodeKind::Element) {
                for a in node.attributes() {
                    // Skip ID-bearing attributes (xml:id or unprefixed id) — only IDREF(S) intended here
                    if let Some(q) = a.name() {
                        let is_xml_id = q.local == "id"
                            && (q.prefix.as_deref() == Some("xml")
                                || q.ns_uri.as_deref()
                                    == Some("http://www.w3.org/XML/1998/namespace"));
                        let is_plain_id =
                            q.local == "id" && q.prefix.is_none() && q.ns_uri.is_none();
                        if is_xml_id || is_plain_id {
                            continue;
                        }
                    }
                    let v = a.string_value();
                    // tokenize on whitespace
                    let collapsed = collapse_whitespace(&v);
                    for t in collapsed.split(' ') {
                        if !t.is_empty() && ids.contains(t) {
                            out.push(XdmItem::Node(a.clone()));
                            break;
                        }
                    }
                }
            }
        }
        Ok(out)
    });

    // encode-for-uri($str)
    reg.register_ns(FNS, "encode-for-uri", 1, |_ctx, args| {
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
    });

    // nilled($arg as node()?) as xs:boolean?
    // Non-schema-aware: element() returns true iff it has xsi:nil="true" (or "1"); false otherwise.
    // Non-element or empty input returns empty sequence.
    reg.register_ns(FNS, "nilled", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        let item = &args[0][0];
        let XdmItem::Node(n) = item else {
            return Ok(vec![]);
        };
        use crate::model::NodeKind;
        if !matches!(n.kind(), NodeKind::Element) {
            return Ok(vec![]);
        }
        let mut is_nilled = false;
        for a in n.attributes() {
            if let Some(q) = a.name() {
                let is_xsi_nil = q.local == "nil"
                    && (q.prefix.as_deref() == Some("xsi")
                        || q.ns_uri.as_deref()
                            == Some("http://www.w3.org/2001/XMLSchema-instance"));
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
    });
    // iri-to-uri($str)
    reg.register_ns(FNS, "iri-to-uri", 1, |_ctx, args| {
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
    });
    // escape-html-uri($uri as xs:string?) as xs:string
    reg.register_ns(FNS, "escape-html-uri", 1, |_ctx, args| {
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
    });
    // resolve-uri($relative as xs:string?, $base as xs:string?) as xs:anyURI?
    reg.register_ns_range(FNS, "resolve-uri", 1, Some(2), |ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        let rel = item_to_string(&args[0]);
        // If rel is absolute (very rough check), return it
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
        // Join naive: strip after last '/' and append rel
        if !baseu.ends_with('/') {
            if let Some(idx) = baseu.rfind('/') {
                baseu.truncate(idx + 1);
            } else {
                baseu.push('/');
            }
        }
        let joined = format!("{}{}", baseu, rel);
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(joined))])
    });

    // normalize-unicode($arg as xs:string?, $form as xs:string = "NFC")
    reg.register_ns_range(FNS, "normalize-unicode", 1, Some(2), |_ctx, args| {
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
                return Err(Error::dynamic(
                    ErrorCode::FORG0001,
                    "invalid normalization form",
                ));
            }
        };
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(out))])
    });
    // doc-available($uri as xs:string?) as xs:boolean
    // Aligned with fn:doc semantics: returns true iff doc($uri) would succeed.
    reg.register_ns(FNS, "doc-available", 1, |ctx, args| {
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
            // No resolver configured → doc() would raise FODC0005 → false
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(false))])
        }
    });
    // doc($uri as xs:string?) as document-node()?
    reg.register_ns(FNS, "doc", 1, |ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        let uri = item_to_string(&args[0]);
        if let Some(nr) = &ctx.dyn_ctx.node_resolver {
            match nr.doc_node(&uri) {
                Ok(Some(n)) => return Ok(vec![XdmItem::Node(n)]),
                Ok(None) => {
                    return Err(Error::dynamic(
                        ErrorCode::FODC0005,
                        "document not available",
                    ));
                }
                Err(_e) => {
                    return Err(Error::dynamic(
                        ErrorCode::FODC0005,
                        "error retrieving document",
                    ));
                }
            }
        }
        // No node resolver configured → signal FODC0005
        Err(Error::dynamic(
            ErrorCode::FODC0005,
            "no node resolver configured for fn:doc",
        ))
    });
    // collection($uri as xs:string?) as node()*
    reg.register_ns_range(FNS, "collection", 0, Some(1), |ctx, args| {
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
    });

    // replace($input, $pattern, $replacement[, $flags])
    reg.register_ns_range(FNS, "replace", 3, Some(4), |ctx, args| {
        if args.len() == 3 {
            replace_default(ctx, &args[0], &args[1], &args[2], None)
        } else {
            let flags = item_to_string(&args[3]);
            replace_default(ctx, &args[0], &args[1], &args[2], Some(&flags))
        }
    });

    // tokenize($input, $pattern[, $flags])
    reg.register_ns_range(FNS, "tokenize", 2, Some(3), |ctx, args| {
        if args.len() == 2 {
            tokenize_default(ctx, &args[0], &args[1], None)
        } else {
            let flags = item_to_string(&args[2]);
            tokenize_default(ctx, &args[0], &args[1], Some(&flags))
        }
    });

    // unordered($arg as item()*) as item()* — identity order for now
    reg.register_ns(FNS, "unordered", 1, |_ctx, args| Ok(args[0].clone()));

    // Minimal constructor-like function (Task 11 subset): integer($arg)
    reg.register_ns(FNS, "integer", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        let s = item_to_string(&args[0]);
        let i: i64 = s
            .parse()
            .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid integer"))?;
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(i))])
    });

    // xs:* constructors are registered at the end to avoid borrow conflicts during registration
    // fn:dateTime($date, $time) and adjust-*-to-timezone are placed in the Date/Time family above
    reg.register_ns(FNS, "dateTime", 2, |_ctx, args| {
        if args[0].is_empty() || args[1].is_empty() {
            return Ok(vec![]);
        }
        // Reuse logic via string constructors
        // Combine via parsing helpers
        let (date, tz_date_opt) = match &args[0][0] {
            XdmItem::Atomic(XdmAtomicValue::Date { date, tz }) => (*date, *tz),
            XdmItem::Atomic(XdmAtomicValue::String(s))
            | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
                let (d, tzo) = parse_xs_date_local(s)
                    .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid xs:date"))?;
                (d, tzo)
            }
            _ => {
                return Err(Error::dynamic(
                    ErrorCode::XPTY0004,
                    "dateTime expects xs:date? and xs:time?",
                ));
            }
        };
        let (time, tz_time_opt) = match &args[1][0] {
            XdmItem::Atomic(XdmAtomicValue::Time { time, tz }) => (*time, *tz),
            XdmItem::Atomic(XdmAtomicValue::String(s))
            | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
                let (t, tzo) = crate::temporal::parse_time_lex(s)
                    .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid xs:time"))?;
                (t, tzo)
            }
            _ => {
                return Err(Error::dynamic(
                    ErrorCode::XPTY0004,
                    "dateTime expects xs:date? and xs:time?",
                ));
            }
        };
        let tz = match (tz_date_opt, tz_time_opt) {
            (Some(a), Some(b)) => {
                if a.local_minus_utc() == b.local_minus_utc() {
                    Some(a)
                } else {
                    return Err(Error::dynamic(ErrorCode::FORG0001, "conflicting timezones"));
                }
            }
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        let dt = crate::temporal::build_naive_datetime(date, time, tz);
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::DateTime(dt))])
    });
    reg.register_ns_range(FNS, "adjust-date-to-timezone", 1, Some(2), |ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        let tz_opt = if args.len() == 1 || args[1].is_empty() {
            Some(ctx.dyn_ctx.timezone_override.unwrap_or_else(|| {
                ctx.dyn_ctx
                    .now
                    .map(|n| *n.offset())
                    .unwrap_or_else(|| ChronoFixedOffset::east_opt(0).unwrap())
            }))
        } else {
            match &args[1][0] {
                XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => {
                    ChronoFixedOffset::east_opt(*secs as i32)
                        .ok_or_else(|| Error::dynamic(ErrorCode::FORG0001, "invalid timezone"))?
                }
                _ => {
                    return Err(Error::dynamic(
                        ErrorCode::XPTY0004,
                        "adjust-date-to-timezone expects xs:dayTimeDuration",
                    ));
                }
            }
            .into()
        };
        let (date, _tz) = match &args[0][0] {
            XdmItem::Atomic(XdmAtomicValue::Date { date, tz: _ }) => (*date, None),
            XdmItem::Atomic(XdmAtomicValue::String(s))
            | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => parse_xs_date_local(s)
                .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid xs:date"))?,
            _ => {
                return Err(Error::dynamic(
                    ErrorCode::XPTY0004,
                    "adjust-date-to-timezone expects xs:date?",
                ));
            }
        };
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Date {
            date,
            tz: tz_opt,
        })])
    });
    reg.register_ns_range(FNS, "adjust-time-to-timezone", 1, Some(2), |ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        let tz_opt = if args.len() == 1 || args[1].is_empty() {
            Some(ctx.dyn_ctx.timezone_override.unwrap_or_else(|| {
                ctx.dyn_ctx
                    .now
                    .map(|n| *n.offset())
                    .unwrap_or_else(|| ChronoFixedOffset::east_opt(0).unwrap())
            }))
        } else {
            match &args[1][0] {
                XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => {
                    ChronoFixedOffset::east_opt(*secs as i32)
                        .ok_or_else(|| Error::dynamic(ErrorCode::FORG0001, "invalid timezone"))?
                }
                _ => {
                    return Err(Error::dynamic(
                        ErrorCode::XPTY0004,
                        "adjust-time-to-timezone expects xs:dayTimeDuration",
                    ));
                }
            }
            .into()
        };
        let (time, _tz) = match &args[0][0] {
            XdmItem::Atomic(XdmAtomicValue::Time { time, tz: _ }) => (*time, None),
            XdmItem::Atomic(XdmAtomicValue::String(s))
            | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
                crate::temporal::parse_time_lex(s)
                    .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid xs:time"))?
            }
            _ => {
                return Err(Error::dynamic(
                    ErrorCode::XPTY0004,
                    "adjust-time-to-timezone expects xs:time?",
                ));
            }
        };
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Time {
            time,
            tz: tz_opt,
        })])
    });
    reg.register_ns_range(
        FNS,
        "adjust-dateTime-to-timezone",
        1,
        Some(2),
        |ctx, args| {
            if args[0].is_empty() {
                return Ok(vec![]);
            }
            let tz_opt = if args.len() == 1 || args[1].is_empty() {
                Some(ctx.dyn_ctx.timezone_override.unwrap_or_else(|| {
                    ctx.dyn_ctx
                        .now
                        .map(|n| *n.offset())
                        .unwrap_or_else(|| ChronoFixedOffset::east_opt(0).unwrap())
                }))
            } else {
                Some(match &args[1][0] {
                    XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => {
                        ChronoFixedOffset::east_opt(*secs as i32).ok_or_else(|| {
                            Error::dynamic(ErrorCode::FORG0001, "invalid timezone")
                        })?
                    }
                    _ => {
                        return Err(Error::dynamic(
                            ErrorCode::XPTY0004,
                            "adjust-dateTime-to-timezone expects xs:dayTimeDuration",
                        ));
                    }
                })
            };
            let dt = match &args[0][0] {
                XdmItem::Atomic(XdmAtomicValue::DateTime(dt)) => *dt,
                XdmItem::Atomic(XdmAtomicValue::String(s))
                | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
                    crate::temporal::parse_date_time_lex(s)
                        .map(|(d, t, tz)| crate::temporal::build_naive_datetime(d, t, tz))
                        .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid xs:dateTime"))?
                }
                _ => {
                    return Err(Error::dynamic(
                        ErrorCode::XPTY0004,
                        "adjust-dateTime-to-timezone expects xs:dateTime?",
                    ));
                }
            };
            let naive = dt.naive_utc();
            let res = match tz_opt {
                Some(ofs) => ofs.from_utc_datetime(&naive),
                None => ChronoFixedOffset::east_opt(0)
                    .unwrap()
                    .from_utc_datetime(&naive),
            };
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::DateTime(res))])
        },
    );

    // ===== Date/Time family (M8 subset) =====
    reg.register_ns(FNS, "current-dateTime", 0, |ctx, _args| {
        let dt = now_in_effective_tz(ctx);
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::DateTime(dt))])
    });
    reg.register_ns(FNS, "current-date", 0, |ctx, _args| {
        let dt = now_in_effective_tz(ctx);
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Date {
            date: dt.date_naive(),
            tz: Some(*dt.offset()),
        })])
    });
    reg.register_ns(FNS, "current-time", 0, |ctx, _args| {
        let dt = now_in_effective_tz(ctx);
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Time {
            time: dt.time(),
            tz: Some(*dt.offset()),
        })])
    });

    // implicit-timezone() as xs:dayTimeDuration
    reg.register_ns(FNS, "implicit-timezone", 0, |ctx, _args| {
        // Prefer explicit override; else use current now's offset; else UTC
        let offset_secs = if let Some(tz) = ctx.dyn_ctx.timezone_override {
            tz.local_minus_utc()
        } else if let Some(n) = ctx.dyn_ctx.now {
            n.offset().local_minus_utc()
        } else {
            0
        };
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(
            offset_secs as i64,
        ))])
    });

    // year-from-dateTime($arg as xs:dateTime?) as xs:integer?
    reg.register_ns(
        FNS,
        "year-from-dateTime",
        1,
        |_ctx, args| match get_datetime(&args[0])? {
            None => Ok(vec![]),
            Some(dt) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                dt.year() as i64
            ))]),
        },
    );
    // hours-from-dateTime($arg as xs:dateTime?) as xs:integer?
    reg.register_ns(
        FNS,
        "hours-from-dateTime",
        1,
        |_ctx, args| match get_datetime(&args[0])? {
            None => Ok(vec![]),
            Some(dt) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                dt.hour() as i64
            ))]),
        },
    );
    reg.register_ns(
        FNS,
        "minutes-from-dateTime",
        1,
        |_ctx, args| match get_datetime(&args[0])? {
            None => Ok(vec![]),
            Some(dt) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                dt.minute() as i64
            ))]),
        },
    );
    reg.register_ns(
        FNS,
        "seconds-from-dateTime",
        1,
        |_ctx, args| match get_datetime(&args[0])? {
            None => Ok(vec![]),
            Some(dt) => {
                let secs = dt.second() as f64 + (dt.nanosecond() as f64) / 1_000_000_000.0;
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Decimal(secs))])
            }
        },
    );
    // month-from-dateTime
    reg.register_ns(
        FNS,
        "month-from-dateTime",
        1,
        |_ctx, args| match get_datetime(&args[0])? {
            None => Ok(vec![]),
            Some(dt) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                dt.month() as i64
            ))]),
        },
    );
    // day-from-dateTime
    reg.register_ns(
        FNS,
        "day-from-dateTime",
        1,
        |_ctx, args| match get_datetime(&args[0])? {
            None => Ok(vec![]),
            Some(dt) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                dt.day() as i64
            ))]),
        },
    );

    // hours-from-time($arg as xs:time?) as xs:integer?
    reg.register_ns(FNS, "hours-from-time", 1, |_ctx, args| {
        match get_time(&args[0])? {
            None => Ok(vec![]),
            Some((time, _)) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                time.hour() as i64
            ))]),
        }
    });
    reg.register_ns(FNS, "minutes-from-time", 1, |_ctx, args| {
        match get_time(&args[0])? {
            None => Ok(vec![]),
            Some((time, _)) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                time.minute() as i64,
            ))]),
        }
    });
    reg.register_ns(FNS, "seconds-from-time", 1, |_ctx, args| {
        match get_time(&args[0])? {
            None => Ok(vec![]),
            Some((time, _)) => {
                let secs = time.second() as f64 + (time.nanosecond() as f64) / 1_000_000_000.0;
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Decimal(secs))])
            }
        }
    });

    // timezone-from-dateTime($arg as xs:dateTime?) as xs:dayTimeDuration?
    reg.register_ns(
        FNS,
        "timezone-from-dateTime",
        1,
        |_ctx, args| match get_datetime(&args[0])? {
            None => Ok(vec![]),
            Some(dt) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(
                dt.offset().local_minus_utc() as i64,
            ))]),
        },
    );
    // timezone-from-date($arg as xs:date?)
    reg.register_ns(FNS, "timezone-from-date", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        match &args[0][0] {
            XdmItem::Atomic(XdmAtomicValue::Date { tz, .. }) => {
                if let Some(off) = tz {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(
                        off.local_minus_utc() as i64,
                    ))])
                } else {
                    Ok(vec![])
                }
            }
            XdmItem::Atomic(XdmAtomicValue::String(s))
            | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
                if let Ok((_d, Some(off))) = parse_xs_date_local(s) {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(
                        off.local_minus_utc() as i64,
                    ))])
                } else {
                    Ok(vec![])
                }
            }
            XdmItem::Node(n) => {
                if let Ok((_d, Some(off))) = parse_xs_date_local(&n.string_value()) {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(
                        off.local_minus_utc() as i64,
                    ))])
                } else {
                    Ok(vec![])
                }
            }
            _ => Ok(vec![]),
        }
    });
    // timezone-from-time($arg as xs:time?)
    reg.register_ns(FNS, "timezone-from-time", 1, |_ctx, args| {
        match get_time(&args[0])? {
            None => Ok(vec![]),
            Some((_t, Some(off))) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(
                off.local_minus_utc() as i64,
            ))]),
            Some((_t, None)) => Ok(vec![]),
        }
    });

    // ===== Duration component accessors (10.5.1–10.5.6) =====
    // Helper to parse a duration-like lexical into either YMD months or DTD seconds
    fn parse_duration_lexical(s: &str) -> Result<(Option<i32>, Option<i64>), Error> {
        if let Ok(m) = parse_year_month_duration_months(s) {
            return Ok((Some(m), None));
        }
        if let Ok(sec) = parse_day_time_duration_secs(s) {
            return Ok((None, Some(sec)));
        }
        Err(Error::dynamic(ErrorCode::FORG0001, "invalid xs:duration"))
    }
    // years-from-duration($arg as xs:duration?) as xs:integer?
    reg.register_ns(FNS, "years-from-duration", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        match &args[0][0] {
            XdmItem::Atomic(XdmAtomicValue::YearMonthDuration(months)) => {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                    (*months / 12) as i64,
                ))])
            }
            XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(_)) => {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
            }
            XdmItem::Atomic(XdmAtomicValue::String(s))
            | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
                let (m_opt, s_opt) = parse_duration_lexical(s)?;
                if let Some(m) = m_opt {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                        (m / 12) as i64,
                    ))])
                } else if s_opt.is_some() {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
                } else {
                    Ok(vec![])
                }
            }
            XdmItem::Node(n) => {
                let (m_opt, s_opt) = parse_duration_lexical(&n.string_value())?;
                if let Some(m) = m_opt {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                        (m / 12) as i64,
                    ))])
                } else if s_opt.is_some() {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
                } else {
                    Ok(vec![])
                }
            }
            _ => Ok(vec![]),
        }
    });
    // months-from-duration($arg as xs:duration?) as xs:integer?
    reg.register_ns(FNS, "months-from-duration", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        match &args[0][0] {
            XdmItem::Atomic(XdmAtomicValue::YearMonthDuration(months)) => {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                    (*months % 12) as i64,
                ))])
            }
            XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(_)) => {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
            }
            XdmItem::Atomic(XdmAtomicValue::String(s))
            | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
                let (m_opt, s_opt) = parse_duration_lexical(s)?;
                if let Some(m) = m_opt {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                        (m % 12) as i64,
                    ))])
                } else if s_opt.is_some() {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
                } else {
                    Ok(vec![])
                }
            }
            XdmItem::Node(n) => {
                let (m_opt, s_opt) = parse_duration_lexical(&n.string_value())?;
                if let Some(m) = m_opt {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                        (m % 12) as i64,
                    ))])
                } else if s_opt.is_some() {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
                } else {
                    Ok(vec![])
                }
            }
            _ => Ok(vec![]),
        }
    });
    // days-from-duration($arg as xs:duration?) as xs:integer?
    reg.register_ns(FNS, "days-from-duration", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        match &args[0][0] {
            XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => Ok(vec![XdmItem::Atomic(
                XdmAtomicValue::Integer((*secs / (24 * 3600)) as i64),
            )]),
            XdmItem::Atomic(XdmAtomicValue::YearMonthDuration(_)) => {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
            }
            XdmItem::Atomic(XdmAtomicValue::String(s))
            | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
                let (m_opt, s_opt) = parse_duration_lexical(s)?;
                if let Some(sec) = s_opt {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                        (sec / (24 * 3600)) as i64,
                    ))])
                } else if m_opt.is_some() {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
                } else {
                    Ok(vec![])
                }
            }
            XdmItem::Node(n) => {
                let (m_opt, s_opt) = parse_duration_lexical(&n.string_value())?;
                if let Some(sec) = s_opt {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                        (sec / (24 * 3600)) as i64,
                    ))])
                } else if m_opt.is_some() {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
                } else {
                    Ok(vec![])
                }
            }
            _ => Ok(vec![]),
        }
    });
    // hours-from-duration($arg as xs:duration?) as xs:integer?
    reg.register_ns(FNS, "hours-from-duration", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        match &args[0][0] {
            XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => {
                let rem = *secs % (24 * 3600);
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                    (rem / 3600) as i64,
                ))])
            }
            XdmItem::Atomic(XdmAtomicValue::YearMonthDuration(_)) => {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
            }
            XdmItem::Atomic(XdmAtomicValue::String(s))
            | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
                let (m_opt, s_opt) = parse_duration_lexical(s)?;
                if let Some(sec) = s_opt {
                    let rem = sec % (24 * 3600);
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                        (rem / 3600) as i64,
                    ))])
                } else if m_opt.is_some() {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
                } else {
                    Ok(vec![])
                }
            }
            XdmItem::Node(n) => {
                let (m_opt, s_opt) = parse_duration_lexical(&n.string_value())?;
                if let Some(sec) = s_opt {
                    let rem = sec % (24 * 3600);
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                        (rem / 3600) as i64,
                    ))])
                } else if m_opt.is_some() {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
                } else {
                    Ok(vec![])
                }
            }
            _ => Ok(vec![]),
        }
    });
    // minutes-from-duration($arg as xs:duration?) as xs:integer?
    reg.register_ns(FNS, "minutes-from-duration", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        match &args[0][0] {
            XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => {
                let rem = *secs % 3600;
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                    (rem / 60) as i64,
                ))])
            }
            XdmItem::Atomic(XdmAtomicValue::YearMonthDuration(_)) => {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
            }
            XdmItem::Atomic(XdmAtomicValue::String(s))
            | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
                let (m_opt, s_opt) = parse_duration_lexical(s)?;
                if let Some(sec) = s_opt {
                    let rem = sec % 3600;
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                        (rem / 60) as i64,
                    ))])
                } else if m_opt.is_some() {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
                } else {
                    Ok(vec![])
                }
            }
            XdmItem::Node(n) => {
                let (m_opt, s_opt) = parse_duration_lexical(&n.string_value())?;
                if let Some(sec) = s_opt {
                    let rem = sec % 3600;
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                        (rem / 60) as i64,
                    ))])
                } else if m_opt.is_some() {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))])
                } else {
                    Ok(vec![])
                }
            }
            _ => Ok(vec![]),
        }
    });
    // seconds-from-duration($arg as xs:duration?) as xs:decimal?
    reg.register_ns(FNS, "seconds-from-duration", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        match &args[0][0] {
            XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs)) => {
                let rem = *secs % 60;
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Decimal(rem as f64))])
            }
            XdmItem::Atomic(XdmAtomicValue::YearMonthDuration(_)) => {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Decimal(0.0))])
            }
            XdmItem::Atomic(XdmAtomicValue::String(s))
            | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
                let (m_opt, s_opt) = parse_duration_lexical(s)?;
                if let Some(sec) = s_opt {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Decimal(
                        (sec % 60) as f64,
                    ))])
                } else if m_opt.is_some() {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Decimal(0.0))])
                } else {
                    Ok(vec![])
                }
            }
            XdmItem::Node(n) => {
                let (m_opt, s_opt) = parse_duration_lexical(&n.string_value())?;
                if let Some(sec) = s_opt {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Decimal(
                        (sec % 60) as f64,
                    ))])
                } else if m_opt.is_some() {
                    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Decimal(0.0))])
                } else {
                    Ok(vec![])
                }
            }
            _ => Ok(vec![]),
        }
    });

    // year/month/day from date (xs:date)
    reg.register_ns(FNS, "year-from-date", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        match &args[0][0] {
            XdmItem::Atomic(XdmAtomicValue::Date { date, .. }) => Ok(vec![XdmItem::Atomic(
                XdmAtomicValue::Integer(date.year() as i64),
            )]),
            XdmItem::Atomic(XdmAtomicValue::String(s))
            | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
                let (d, _) = parse_xs_date_local(s)
                    .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid xs:date"))?;
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                    d.year() as i64
                ))])
            }
            XdmItem::Node(n) => {
                let (d, _) = parse_xs_date_local(&n.string_value())
                    .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid xs:date"))?;
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                    d.year() as i64
                ))])
            }
            _ => Ok(vec![]),
        }
    });
    reg.register_ns(FNS, "month-from-date", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        match &args[0][0] {
            XdmItem::Atomic(XdmAtomicValue::Date { date, .. }) => Ok(vec![XdmItem::Atomic(
                XdmAtomicValue::Integer(date.month() as i64),
            )]),
            XdmItem::Atomic(XdmAtomicValue::String(s))
            | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
                let (d, _) = parse_xs_date_local(s)
                    .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid xs:date"))?;
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                    d.month() as i64
                ))])
            }
            XdmItem::Node(n) => {
                let (d, _) = parse_xs_date_local(&n.string_value())
                    .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid xs:date"))?;
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                    d.month() as i64
                ))])
            }
            _ => Ok(vec![]),
        }
    });
    reg.register_ns(FNS, "day-from-date", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        match &args[0][0] {
            XdmItem::Atomic(XdmAtomicValue::Date { date, .. }) => Ok(vec![XdmItem::Atomic(
                XdmAtomicValue::Integer(date.day() as i64),
            )]),
            XdmItem::Atomic(XdmAtomicValue::String(s))
            | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
                let (d, _) = parse_xs_date_local(s)
                    .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid xs:date"))?;
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                    d.day() as i64
                ))])
            }
            XdmItem::Node(n) => {
                let (d, _) = parse_xs_date_local(&n.string_value())
                    .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid xs:date"))?;
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
                    d.day() as i64
                ))])
            }
            _ => Ok(vec![]),
        }
    });

    // ===== XML Schema constructors (xs:*) =====
    // xs:string($arg) as xs:string?
    reg.register_ns(XS, "string", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        if args[0].len() > 1 {
            return Err(Error::dynamic(
                ErrorCode::FORG0006,
                "constructor expects at most one item",
            ));
        }
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(
            item_to_string(&args[0]),
        ))])
    });
    // NOTE: XPath 2.0 does not define an xs:untypedAtomic constructor, but test
    // suites (and some engines) accept it for convenience. Provide a permissive
    // implementation that mirrors other primitive constructors: empty sequence
    // -> empty result; single item -> atomize to string and wrap as untypedAtomic;
    // length > 1 -> FORG0006. Lexical form is unconstrained by design.
    reg.register_ns(XS, "untypedAtomic", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        if args[0].len() > 1 {
            return Err(Error::dynamic(
                ErrorCode::FORG0006,
                "constructor expects at most one item",
            ));
        }
        let s = item_to_string(&args[0]);
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s))])
    });
    // xs:boolean($arg) as xs:boolean?
    reg.register_ns(XS, "boolean", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        if args[0].len() > 1 {
            return Err(Error::dynamic(
                ErrorCode::FORG0006,
                "constructor expects at most one item",
            ));
        }
        // Atomize first item then apply constructor lexical rules: "true"/"1" => true, "false"/"0" => false
        let s = item_to_string(&args[0]);
        let v = match s.as_str() {
            "true" | "1" => true,
            "false" | "0" => false,
            _ => return Err(Error::dynamic(ErrorCode::FORG0001, "invalid xs:boolean")),
        };
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(v))])
    });
    // xs:integer($arg) as xs:integer?
    reg.register_ns(XS, "integer", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        if args[0].len() > 1 {
            return Err(Error::dynamic(
                ErrorCode::FORG0006,
                "constructor expects at most one item",
            ));
        }
        let s = item_to_string(&args[0]);
        let s_trim = s.trim();
        if s_trim.is_empty() {
            return Err(Error::dynamic(ErrorCode::FORG0001, "invalid xs:integer"));
        }
        // If decimal point or exponent present attempt f64 parse to distinguish fractional (FOCA0001)
        if s_trim.contains('.') || s_trim.contains('e') || s_trim.contains('E') {
            if let Ok(f) = s_trim.parse::<f64>() {
                if !f.is_finite() || f.fract() != 0.0 {
                    return Err(Error::dynamic(
                        ErrorCode::FOCA0001,
                        "fractional part in integer cast",
                    ));
                }
            }
        }
        let i: i64 = s_trim
            .parse()
            .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid xs:integer"))?;
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(i))])
    });

    // xs:decimal($arg) as xs:decimal?
    reg.register_ns(XS, "decimal", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        if args[0].len() > 1 {
            return Err(Error::dynamic(
                ErrorCode::FORG0006,
                "constructor expects at most one item",
            ));
        }
        let s = item_to_string(&args[0]).trim().to_string();
        // XML Schema decimal does not allow INF/NaN
        if s.eq_ignore_ascii_case("nan")
            || s.eq_ignore_ascii_case("inf")
            || s.eq_ignore_ascii_case("-inf")
        {
            return Err(Error::dynamic(ErrorCode::FORG0001, "invalid xs:decimal"));
        }
        let v: f64 = s
            .parse()
            .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid xs:decimal"))?;
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Decimal(v))])
    });
    // xs:double($arg) as xs:double?
    reg.register_ns(XS, "double", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        if args[0].len() > 1 {
            return Err(Error::dynamic(
                ErrorCode::FORG0006,
                "constructor expects at most one item",
            ));
        }
        let s = item_to_string(&args[0]).trim().to_string();
        let v = match s.as_str() {
            "NaN" => f64::NAN,
            "INF" => f64::INFINITY,
            "-INF" => f64::NEG_INFINITY,
            _ => s
                .parse()
                .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid xs:double"))?,
        };
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(v))])
    });
    // xs:float($arg) as xs:float?
    reg.register_ns(XS, "float", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        if args[0].len() > 1 {
            return Err(Error::dynamic(
                ErrorCode::FORG0006,
                "constructor expects at most one item",
            ));
        }
        let s = item_to_string(&args[0]).trim().to_string();
        let v = match s.as_str() {
            "NaN" => f32::NAN,
            "INF" => f32::INFINITY,
            "-INF" => f32::NEG_INFINITY,
            _ => s
                .parse()
                .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid xs:float"))?,
        };
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Float(v))])
    });

    // anyURI: whitespace facet = collapse
    reg.register_ns(XS, "anyURI", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        if args[0].len() > 1 {
            return Err(Error::dynamic(
                ErrorCode::FORG0006,
                "constructor expects at most one item",
            ));
        }
        let s = collapse_whitespace(&item_to_string(&args[0]));
        // Policy: allow empty anyURI after collapse
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(s))])
    });

    // QName: resolve prefix using static context namespaces (xml prefix is implicit)
    reg.register_ns(XS, "QName", 1, |ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        if args[0].len() > 1 {
            return Err(Error::dynamic(
                ErrorCode::FORG0006,
                "constructor expects at most one item",
            ));
        }
        let s = item_to_string(&args[0]);
        let (prefix_opt, local) = parse_qname_lexical(&s)
            .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid xs:QName"))?;
        let ns_uri = match prefix_opt.as_deref() {
            None => None,
            Some("xml") => Some("http://www.w3.org/XML/1998/namespace".to_string()),
            Some(p) => ctx.static_ctx.namespaces.by_prefix.get(p).cloned(),
        };
        if prefix_opt.is_some() && ns_uri.is_none() {
            return Err(Error::dynamic(
                ErrorCode::FORG0001,
                "unknown namespace prefix for QName",
            ));
        }
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::QName {
            ns_uri,
            prefix: prefix_opt,
            local,
        })])
    });

    // Binary types: base64Binary and hexBinary
    reg.register_ns(XS, "base64Binary", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        if args[0].len() > 1 {
            return Err(Error::dynamic(
                ErrorCode::FORG0006,
                "constructor expects at most one item",
            ));
        }
        let raw = item_to_string(&args[0]);
        let norm: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
        if base64::engine::general_purpose::STANDARD
            .decode(&norm)
            .is_err()
        {
            return Err(Error::dynamic(
                ErrorCode::FORG0001,
                "invalid xs:base64Binary",
            ));
        }
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Base64Binary(norm))])
    });
    reg.register_ns(XS, "hexBinary", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        if args[0].len() > 1 {
            return Err(Error::dynamic(
                ErrorCode::FORG0006,
                "constructor expects at most one item",
            ));
        }
        let raw = item_to_string(&args[0]);
        let norm: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
        if norm.len() % 2 != 0 || !norm.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(Error::dynamic(ErrorCode::FORG0001, "invalid xs:hexBinary"));
        }
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::HexBinary(norm))])
    });

    // Dates and times
    reg.register_ns(XS, "dateTime", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        if args[0].len() > 1 {
            return Err(Error::dynamic(
                ErrorCode::FORG0006,
                "constructor expects at most one item",
            ));
        }
        let s = item_to_string(&args[0]);
        match crate::temporal::parse_date_time_lex(&s) {
            Ok((d, t, tz)) => {
                let dt = crate::temporal::build_naive_datetime(d, t, tz);
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::DateTime(dt))])
            }
            Err(_) => Err(Error::dynamic(ErrorCode::FORG0001, "invalid xs:dateTime")),
        }
    });
    // (dateTime component extractors are registered earlier)
    reg.register_ns(XS, "date", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        if args[0].len() > 1 {
            return Err(Error::dynamic(
                ErrorCode::FORG0006,
                "constructor expects at most one item",
            ));
        }
        let s = item_to_string(&args[0]);
        match crate::temporal::parse_date_lex(&s) {
            Ok((d, tz)) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::Date { date: d, tz })]),
            Err(_) => Err(Error::dynamic(ErrorCode::FORG0001, "invalid xs:date")),
        }
    });
    reg.register_ns(XS, "time", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        if args[0].len() > 1 {
            return Err(Error::dynamic(
                ErrorCode::FORG0006,
                "constructor expects at most one item",
            ));
        }
        let s = item_to_string(&args[0]);
        match crate::temporal::parse_time_lex(&s) {
            Ok((t, tz)) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::Time { time: t, tz })]),
            Err(_) => Err(Error::dynamic(ErrorCode::FORG0001, "invalid xs:time")),
        }
    });

    // Durations
    reg.register_ns(XS, "dayTimeDuration", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        if args[0].len() > 1 {
            return Err(Error::dynamic(
                ErrorCode::FORG0006,
                "constructor expects at most one item",
            ));
        }
        let s = item_to_string(&args[0]);
        let secs = parse_day_time_duration_secs(&s)
            .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid xs:dayTimeDuration"))?;
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs))])
    });
    reg.register_ns(XS, "yearMonthDuration", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        if args[0].len() > 1 {
            return Err(Error::dynamic(
                ErrorCode::FORG0006,
                "constructor expects at most one item",
            ));
        }
        let s = item_to_string(&args[0]);
        let months = parse_year_month_duration_months(&s)
            .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid xs:yearMonthDuration"))?;
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::YearMonthDuration(
            months,
        ))])
    });

    // Integer-derived subtypes (range checking)
    reg.register_ns(XS, "long", 1, |_ctx, args| {
        int_subtype_i64(args, i64::MIN, i64::MAX, |v| XdmAtomicValue::Long(v))
    });
    reg.register_ns(XS, "int", 1, |_ctx, args| {
        int_subtype_i64(args, i32::MIN as i64, i32::MAX as i64, |v| {
            XdmAtomicValue::Int(v as i32)
        })
    });
    reg.register_ns(XS, "short", 1, |_ctx, args| {
        int_subtype_i64(args, i16::MIN as i64, i16::MAX as i64, |v| {
            XdmAtomicValue::Short(v as i16)
        })
    });
    reg.register_ns(XS, "byte", 1, |_ctx, args| {
        int_subtype_i64(args, i8::MIN as i64, i8::MAX as i64, |v| {
            XdmAtomicValue::Byte(v as i8)
        })
    });
    reg.register_ns(XS, "unsignedLong", 1, |_ctx, args| {
        uint_subtype_u128(args, 0, u64::MAX as u128, |v| {
            XdmAtomicValue::UnsignedLong(v as u64)
        })
    });
    reg.register_ns(XS, "unsignedInt", 1, |_ctx, args| {
        uint_subtype_u128(args, 0, u32::MAX as u128, |v| {
            XdmAtomicValue::UnsignedInt(v as u32)
        })
    });
    reg.register_ns(XS, "unsignedShort", 1, |_ctx, args| {
        uint_subtype_u128(args, 0, u16::MAX as u128, |v| {
            XdmAtomicValue::UnsignedShort(v as u16)
        })
    });
    reg.register_ns(XS, "unsignedByte", 1, |_ctx, args| {
        uint_subtype_u128(args, 0, u8::MAX as u128, |v| {
            XdmAtomicValue::UnsignedByte(v as u8)
        })
    });
    reg.register_ns(XS, "nonPositiveInteger", 1, |_ctx, args| {
        int_subtype_i64(args, i64::MIN, 0, |v| XdmAtomicValue::NonPositiveInteger(v))
    });
    reg.register_ns(XS, "negativeInteger", 1, |_ctx, args| {
        int_subtype_i64(args, i64::MIN, -1, |v| XdmAtomicValue::NegativeInteger(v))
    });
    reg.register_ns(XS, "nonNegativeInteger", 1, |_ctx, args| {
        uint_subtype_u128(args, 0, u64::MAX as u128, |v| {
            XdmAtomicValue::NonNegativeInteger(v as u64)
        })
    });
    reg.register_ns(XS, "positiveInteger", 1, |_ctx, args| {
        uint_subtype_u128(args, 1, u64::MAX as u128, |v| {
            XdmAtomicValue::PositiveInteger(v as u64)
        })
    });

    // String-derived types
    reg.register_ns(XS, "normalizedString", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        if args[0].len() > 1 {
            return Err(Error::dynamic(
                ErrorCode::FORG0006,
                "constructor expects at most one item",
            ));
        }
        let s = replace_whitespace(&item_to_string(&args[0]));
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::NormalizedString(s))])
    });
    reg.register_ns(XS, "token", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        if args[0].len() > 1 {
            return Err(Error::dynamic(
                ErrorCode::FORG0006,
                "constructor expects at most one item",
            ));
        }
        let s = collapse_whitespace(&item_to_string(&args[0]));
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Token(s))])
    });
    reg.register_ns(XS, "language", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        if args[0].len() > 1 {
            return Err(Error::dynamic(
                ErrorCode::FORG0006,
                "constructor expects at most one item",
            ));
        }
        let s = collapse_whitespace(&item_to_string(&args[0]));
        if !is_valid_language(&s) {
            return Err(Error::dynamic(ErrorCode::FORG0001, "invalid xs:language"));
        }
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Language(s))])
    });
    reg.register_ns(XS, "Name", 1, |_ctx, args| {
        str_name_like(args, true, true, |s| XdmAtomicValue::Name(s))
    });
    reg.register_ns(XS, "NCName", 1, |_ctx, args| {
        str_name_like(args, true, false, |s| XdmAtomicValue::NCName(s))
    });
    reg.register_ns(XS, "NMTOKEN", 1, |_ctx, args| {
        str_name_like(args, false, false, |s| XdmAtomicValue::NMTOKEN(s))
    });
    reg.register_ns(XS, "ID", 1, |_ctx, args| {
        str_name_like(args, true, false, |s| XdmAtomicValue::Id(s))
    });
    reg.register_ns(XS, "IDREF", 1, |_ctx, args| {
        str_name_like(args, true, false, |s| XdmAtomicValue::IdRef(s))
    });
    reg.register_ns(XS, "ENTITY", 1, |_ctx, args| {
        str_name_like(args, true, false, |s| XdmAtomicValue::Entity(s))
    });
    // NOTATION (QName lexical) — store lexical; minimal validation: QName-like
    reg.register_ns(XS, "NOTATION", 1, |_ctx, args| {
        if args[0].is_empty() {
            return Ok(vec![]);
        }
        if args[0].len() > 1 {
            return Err(Error::dynamic(
                ErrorCode::FORG0006,
                "constructor expects at most one item",
            ));
        }
        let s = item_to_string(&args[0]);
        if parse_qname_lexical(&s).is_err() {
            return Err(Error::dynamic(ErrorCode::FORG0001, "invalid xs:NOTATION"));
        }
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Notation(s))])
    });

    reg
}

fn item_to_string<N: crate::model::XdmNode>(seq: &XdmSequence<N>) -> String {
    if seq.is_empty() {
        return String::new();
    }
    match &seq[0] {
        XdmItem::Atomic(a) => as_string(a),
        XdmItem::Node(n) => n.string_value(),
    }
}

// Default implementation for normalize-space handling both 0- and 1-arity variants
fn normalize_space_default<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    arg_opt: Option<&XdmSequence<N>>,
) -> XdmSequence<N> {
    let s = match arg_opt {
        Some(seq) => item_to_string(seq),
        None => {
            if let Some(ci) = &ctx.dyn_ctx.context_item {
                match ci {
                    XdmItem::Atomic(a) => as_string(a),
                    XdmItem::Node(n) => n.string_value(),
                }
            } else {
                String::new()
            }
        }
    };
    let mut out = String::new();
    let mut in_space = true;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !in_space {
                out.push(' ');
                in_space = true;
            }
        } else {
            out.push(ch);
            in_space = false;
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    vec![XdmItem::Atomic(XdmAtomicValue::String(out))]
}

// Default implementation for data() 0/1-arity
fn data_default<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    arg_opt: Option<&XdmSequence<N>>,
) -> Result<XdmSequence<N>, Error> {
    if let Some(seq) = arg_opt {
        let mut out: XdmSequence<N> = Vec::with_capacity(seq.len());
        for it in seq {
            match it {
                XdmItem::Atomic(a) => out.push(XdmItem::Atomic(a.clone())),
                XdmItem::Node(n) => out.push(XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(
                    n.string_value(),
                ))),
            }
        }
        Ok(out)
    } else {
        if let Some(ci) = &ctx.dyn_ctx.context_item {
            match ci {
                XdmItem::Atomic(a) => Ok(vec![XdmItem::Atomic(a.clone())]),
                XdmItem::Node(n) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(
                    n.string_value(),
                ))]),
            }
        } else {
            Ok(Vec::new())
        }
    }
}

// Default implementation for number() 0/1-arity
fn number_default<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    arg_opt: Option<&XdmSequence<N>>,
) -> Result<XdmSequence<N>, Error> {
    let seq: XdmSequence<N> = if let Some(s) = arg_opt {
        s.clone()
    } else if let Some(ci) = &ctx.dyn_ctx.context_item {
        vec![ci.clone()]
    } else {
        vec![]
    };
    let n = to_number(&seq).unwrap_or(f64::NAN);
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(n))])
}

// Default implementation for string() 0/1-arity
fn string_default<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    arg_opt: Option<&XdmSequence<N>>,
) -> Result<XdmSequence<N>, Error> {
    let s = match arg_opt {
        Some(seq) => item_to_string(seq),
        None => {
            if let Some(ci) = &ctx.dyn_ctx.context_item {
                match ci {
                    XdmItem::Atomic(a) => as_string(a),
                    XdmItem::Node(n) => n.string_value(),
                }
            } else {
                String::new()
            }
        }
    };
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(s))])
}

// Default implementation for substring handling both 2- and 3-arity variants
fn substring_default(s: &str, start_raw: f64, len_raw_opt: Option<f64>) -> String {
    // NaN handling
    if start_raw.is_nan() {
        return String::new();
    }
    if let Some(len_raw) = len_raw_opt {
        if len_raw.is_nan() {
            return String::new();
        }
        if len_raw.is_infinite() && len_raw.is_sign_negative() {
            return String::new();
        }
        if len_raw <= 0.0 {
            return String::new();
        }
    }

    // +/- infinity on start
    if start_raw.is_infinite() {
        if start_raw.is_sign_positive() {
            return String::new();
        } else {
            // -INF: treat as starting well before string
            let chars: Vec<char> = s.chars().collect();
            if let Some(len_raw) = len_raw_opt {
                let len_rounded = round_half_to_even_f64(len_raw);
                if len_rounded <= 0.0 {
                    return String::new();
                }
                let total = chars.len() as isize;
                let first_pos: isize = 1;
                let mut last_pos: isize = first_pos + len_rounded as isize - 1;
                if first_pos > total {
                    return String::new();
                }
                if last_pos > total {
                    last_pos = total;
                }
                let from_index = 0usize;
                let to_index = last_pos.max(0) as usize;
                return chars[from_index..to_index].iter().collect();
            } else {
                // 2-arity: whole string
                return s.to_string();
            }
        }
    }

    // Common path
    let start_rounded = round_half_to_even_f64(start_raw);
    if let Some(len_raw) = len_raw_opt {
        let len_rounded = round_half_to_even_f64(len_raw);
        if len_rounded <= 0.0 {
            return String::new();
        }
        let chars: Vec<char> = s.chars().collect();
        let total = chars.len() as isize;
        let first_pos: isize = if start_rounded < 1.0 {
            1
        } else {
            start_rounded as isize
        };
        let mut last_pos: isize = first_pos + len_rounded as isize - 1;
        if first_pos > total {
            return String::new();
        }
        if last_pos > total {
            last_pos = total;
        }
        let from_index = (first_pos - 1).max(0) as usize;
        let to_index = last_pos.max(0) as usize; // inclusive 1-based -> exclusive
        chars[from_index..to_index].iter().collect()
    } else {
        // 2-arity: from start to end
        if start_rounded <= 1.0 {
            s.to_string()
        } else {
            let from_index: usize = (start_rounded as isize - 1).max(0) as usize;
            s.chars().skip(from_index).collect()
        }
    }
}

// Default implementation for node-name(1)
fn node_name_default<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    arg_opt: Option<&XdmSequence<N>>,
) -> Result<XdmSequence<N>, Error> {
    let Some(seq) = arg_opt else {
        return Ok(vec![]);
    };
    if seq.is_empty() {
        return Ok(vec![]);
    }
    match &seq[0] {
        XdmItem::Node(n) => {
            if let Some(q) = n.name() {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::QName {
                    ns_uri: q.ns_uri.clone(),
                    prefix: q.prefix.clone(),
                    local: q.local.clone(),
                })])
            } else {
                Ok(vec![])
            }
        }
        _ => Err(Error::dynamic(
            ErrorCode::XPTY0004,
            "node-name expects node()",
        )),
    }
}

// Default implementation for round($value[, $precision])
fn round_default<N: crate::model::XdmNode>(
    value_seq: &XdmSequence<N>,
    precision_seq_opt: Option<&XdmSequence<N>>,
) -> Result<XdmSequence<N>, Error> {
    match precision_seq_opt {
        None => {
            // Preserve existing semantics of 1-arity implementation using to_number (empty -> NaN)
            let n = to_number(value_seq).unwrap_or(f64::NAN);
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(n.round()))])
        }
        Some(pseq) => {
            if value_seq.is_empty() {
                return Ok(vec![]);
            }
            let n = to_number(value_seq).unwrap_or(f64::NAN);
            let p = to_number(pseq).unwrap_or(f64::NAN);
            let precision = if p.is_nan() { 0 } else { p.trunc() as i64 };
            let r = round_with_precision(n, precision);
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(r))])
        }
    }
}

// Default implementation for round-half-to-even($value[, $precision])
fn round_half_to_even_default<N: crate::model::XdmNode>(
    value_seq: &XdmSequence<N>,
    precision_seq_opt: Option<&XdmSequence<N>>,
) -> Result<XdmSequence<N>, Error> {
    match precision_seq_opt {
        None => {
            let n = to_number(value_seq).unwrap_or(f64::NAN);
            let r = round_half_to_even_with_precision(n, 0);
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(r))])
        }
        Some(pseq) => {
            if value_seq.is_empty() {
                return Ok(vec![]);
            }
            let n = to_number(value_seq).unwrap_or(f64::NAN);
            let p = to_number(pseq).unwrap_or(f64::NAN);
            let precision = if p.is_nan() { 0 } else { p.trunc() as i64 };
            let r = round_half_to_even_with_precision(n, precision);
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(r))])
        }
    }
}

// Default implementation for subsequence($seq,$start[,$len])
fn subsequence_default<N: crate::model::XdmNode + Clone>(
    seq: &XdmSequence<N>,
    start_raw: f64,
    len_raw_opt: Option<f64>,
) -> Result<XdmSequence<N>, Error> {
    if start_raw.is_nan() {
        return Ok(vec![]);
    }
    if start_raw.is_infinite() && start_raw.is_sign_positive() {
        return Ok(vec![]);
    }
    let start_rounded = round_half_to_even_f64(start_raw);
    if let Some(len_raw) = len_raw_opt {
        if len_raw.is_nan() || len_raw <= 0.0 {
            return Ok(vec![]);
        }
        let len_rounded = round_half_to_even_f64(len_raw);
        if len_rounded <= 0.0 {
            return Ok(vec![]);
        }
        let total = seq.len() as isize;
        let first_pos: isize = if start_rounded < 1.0 {
            1
        } else {
            start_rounded as isize
        };
        let last_pos = first_pos + len_rounded as isize - 1;
        if first_pos > total {
            return Ok(vec![]);
        }
        let last_pos = last_pos.min(total);
        let from_index = (first_pos - 1).max(0) as usize;
        let to_index_exclusive = last_pos as usize;
        Ok(seq
            .iter()
            .skip(from_index)
            .take(to_index_exclusive - from_index)
            .cloned()
            .collect())
    } else {
        if start_rounded <= 1.0 {
            return Ok(seq.clone());
        }
        let from_index = (start_rounded as isize - 1).max(0) as usize;
        Ok(seq.iter().skip(from_index).cloned().collect())
    }
}

// Default for deep-equal 2|3-arity
fn deep_equal_default<N: crate::model::XdmNode>(
    ctx: &CallCtx<N>,
    a: &XdmSequence<N>,
    b: &XdmSequence<N>,
    collation_uri: Option<&str>,
) -> Result<XdmSequence<N>, Error> {
    let k = crate::collation::resolve_collation(
        &ctx.dyn_ctx,
        ctx.default_collation.as_ref(),
        collation_uri.and_then(|u| if u.is_empty() { None } else { Some(u) }),
    )?;
    let b = deep_equal_with_collation(a, b, Some(k.as_trait()))?;
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))])
}

// Defaults for regex family: matches/replace/tokenize
fn matches_default<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    input: &XdmSequence<N>,
    pattern: &XdmSequence<N>,
    flags_opt: Option<&str>,
) -> Result<XdmSequence<N>, Error> {
    let inp = item_to_string(input);
    let pat = item_to_string(pattern);
    let flags = flags_opt.unwrap_or("");
    let b = regex_matches(ctx, &inp, &pat, flags)?;
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))])
}

fn replace_default<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    input: &XdmSequence<N>,
    pattern: &XdmSequence<N>,
    repl: &XdmSequence<N>,
    flags_opt: Option<&str>,
) -> Result<XdmSequence<N>, Error> {
    let inp = item_to_string(input);
    let pat = item_to_string(pattern);
    let rep = item_to_string(repl);
    let flags = flags_opt.unwrap_or("");
    let s = regex_replace(ctx, &inp, &pat, &rep, flags)?;
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(s))])
}

fn tokenize_default<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    input: &XdmSequence<N>,
    pattern: &XdmSequence<N>,
    flags_opt: Option<&str>,
) -> Result<XdmSequence<N>, Error> {
    let inp = item_to_string(input);
    let pat = item_to_string(pattern);
    let flags = flags_opt.unwrap_or("");
    let parts = regex_tokenize(ctx, &inp, &pat, flags)?;
    Ok(parts
        .into_iter()
        .map(|s| XdmItem::Atomic(XdmAtomicValue::String(s)))
        .collect())
}

// Default for sum($seq[, $zero])
fn sum_default<N: crate::model::XdmNode>(
    seq: &XdmSequence<N>,
    zero_opt: Option<&XdmSequence<N>>,
) -> Result<XdmSequence<N>, Error> {
    if seq.is_empty() {
        if let Some(z) = zero_opt {
            if z.is_empty() {
                return Err(Error::dynamic(
                    ErrorCode::FORG0001,
                    "sum seed required when first arg empty",
                ));
            } else {
                return Ok(z.clone());
            }
        }
        // when no seed provided, but empty seq: per existing behavior in 1-arity path we returned 0 for empty? Original 1-arity returned 0 as xs:integer.
        return Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))]);
    }
    let mut kind = NumericKind::Integer; // narrowest so far
    let mut int_acc: i128 = 0; // integer accumulation
    let mut dec_acc: f64 = 0.0; // promoted accumulation
    let mut use_int_acc = true;
    for it in seq {
        let XdmItem::Atomic(a) = it else {
            return Err(Error::dynamic(
                ErrorCode::XPTY0004,
                "sum on non-atomic item",
            ));
        };
        if let Some((nk, num)) = classify_numeric(a)? {
            if nk == NumericKind::Double && num.is_nan() {
                return Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(f64::NAN))]);
            }
            kind = kind.promote(nk);
            match nk {
                NumericKind::Integer if use_int_acc => {
                    if let Some(i) = a_as_i128(a) {
                        if let Some(v) = int_acc.checked_add(i) {
                            int_acc = v;
                        } else {
                            use_int_acc = false;
                            dec_acc = int_acc as f64 + i as f64;
                            kind = kind.promote(NumericKind::Decimal);
                        }
                    }
                }
                _ => {
                    if use_int_acc {
                        dec_acc = int_acc as f64;
                        use_int_acc = false;
                    }
                    dec_acc += num;
                }
            }
        } else {
            return Err(Error::dynamic(
                ErrorCode::XPTY0004,
                "sum requires numeric values",
            ));
        }
    }
    let out = if use_int_acc && matches!(kind, NumericKind::Integer) {
        XdmAtomicValue::Integer(int_acc as i64)
    } else {
        match kind {
            NumericKind::Integer => XdmAtomicValue::Integer(int_acc as i64),
            NumericKind::Decimal => XdmAtomicValue::Decimal(dec_acc),
            NumericKind::Float => XdmAtomicValue::Float(dec_acc as f32),
            NumericKind::Double => XdmAtomicValue::Double(dec_acc),
        }
    };
    Ok(vec![XdmItem::Atomic(out)])
}

// Default implementation for name() 0/1-arity
fn name_default<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    arg_opt: Option<&XdmSequence<N>>,
) -> XdmSequence<N> {
    use crate::model::NodeKind;
    let s = if let Some(seq) = arg_opt {
        if seq.is_empty() {
            String::new()
        } else {
            match &seq[0] {
                XdmItem::Node(n) => n
                    .name()
                    .map(|q| {
                        if matches!(n.kind(), NodeKind::Namespace) {
                            q.local
                        } else if let Some(p) = q.prefix {
                            format!("{}:{}", p, q.local)
                        } else {
                            q.local
                        }
                    })
                    .unwrap_or_default(),
                _ => return vec![XdmItem::Atomic(XdmAtomicValue::String(String::new()))],
            }
        }
    } else {
        if let Some(ci) = &ctx.dyn_ctx.context_item {
            match ci {
                XdmItem::Node(n) => n
                    .name()
                    .map(|q| {
                        if matches!(n.kind(), NodeKind::Namespace) {
                            q.local
                        } else if let Some(p) = q.prefix {
                            format!("{}:{}", p, q.local)
                        } else {
                            q.local
                        }
                    })
                    .unwrap_or_default(),
                _ => String::new(),
            }
        } else {
            String::new()
        }
    };
    vec![XdmItem::Atomic(XdmAtomicValue::String(s))]
}

// Default implementation for local-name() 0/1-arity
fn local_name_default<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    arg_opt: Option<&XdmSequence<N>>,
) -> XdmSequence<N> {
    let s = if let Some(seq) = arg_opt {
        if seq.is_empty() {
            String::new()
        } else {
            match &seq[0] {
                XdmItem::Node(n) => n.name().map(|q| q.local).unwrap_or_default(),
                _ => return vec![XdmItem::Atomic(XdmAtomicValue::String(String::new()))],
            }
        }
    } else {
        if let Some(ci) = &ctx.dyn_ctx.context_item {
            match ci {
                XdmItem::Node(n) => n.name().map(|q| q.local).unwrap_or_default(),
                _ => String::new(),
            }
        } else {
            String::new()
        }
    };
    vec![XdmItem::Atomic(XdmAtomicValue::String(s))]
}

// Default implementation for namespace-uri() 0/1-arity
fn namespace_uri_default<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    arg_opt: Option<&XdmSequence<N>>,
) -> Result<XdmSequence<N>, Error> {
    use crate::model::NodeKind;
    if let Some(seq) = arg_opt {
        if seq.is_empty() {
            return Ok(vec![]);
        }
        match &seq[0] {
            XdmItem::Node(n) => {
                let uri = if matches!(n.kind(), NodeKind::Namespace) {
                    String::new()
                } else {
                    n.name().and_then(|q| q.ns_uri).unwrap_or_default()
                };
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(uri))])
            }
            _ => Err(Error::dynamic(
                ErrorCode::XPTY0004,
                "namespace-uri expects node()",
            )),
        }
    } else {
        let s = if let Some(ci) = &ctx.dyn_ctx.context_item {
            match ci {
                XdmItem::Node(n) => {
                    if matches!(n.kind(), NodeKind::Namespace) {
                        String::new()
                    } else {
                        n.name().and_then(|q| q.ns_uri).unwrap_or_default()
                    }
                }
                _ => String::new(),
            }
        } else {
            String::new()
        };
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(s))])
    }
}

// Default implementation for compare($A,$B[,$collation])
fn compare_default<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    a: &XdmSequence<N>,
    b: &XdmSequence<N>,
    collation_uri: Option<&str>,
) -> Result<XdmSequence<N>, Error> {
    if a.is_empty() || b.is_empty() {
        return Ok(vec![]);
    }
    let sa = item_to_string(a);
    let sb = item_to_string(b);
    let uri_opt = collation_uri.and_then(|u| if u.is_empty() { None } else { Some(u) });
    let k =
        crate::collation::resolve_collation(&ctx.dyn_ctx, ctx.default_collation.as_ref(), uri_opt)?;
    let c = k.as_trait();
    let ord = c.compare(&sa, &sb);
    let v = match ord {
        core::cmp::Ordering::Less => -1,
        core::cmp::Ordering::Equal => 0,
        core::cmp::Ordering::Greater => 1,
    };
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(v))])
}

// Default implementation for index-of($seq,$search[,$collation])
fn index_of_default<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    seq: &XdmSequence<N>,
    search: &XdmSequence<N>,
    collation_uri: Option<&str>,
) -> Result<XdmSequence<N>, Error> {
    use crate::eq::{EqKey, build_eq_key};
    let mut out: XdmSequence<N> = Vec::new();
    let uri_opt = collation_uri.and_then(|u| if u.is_empty() { None } else { Some(u) });
    let coll_kind =
        crate::collation::resolve_collation(&ctx.dyn_ctx, ctx.default_collation.as_ref(), uri_opt)?;
    let coll: Option<&dyn crate::collation::Collation> = Some(coll_kind.as_trait());
    let needle_opt = search.first();
    // Precompute key for atomic needle; if NaN early return empty.
    let needle_key = if let Some(XdmItem::Atomic(a)) = needle_opt {
        match build_eq_key::<crate::simple_node::SimpleNode>(&XdmItem::Atomic(a.clone()), coll) {
            Ok(k) => {
                if matches!(k, EqKey::NaN) {
                    return Ok(out);
                }
                Some(k)
            }
            Err(_) => None,
        }
    } else {
        None
    };
    for (i, it) in seq.iter().enumerate() {
        let eq = match (it, needle_opt) {
            (XdmItem::Atomic(a), Some(XdmItem::Atomic(_))) => {
                if let Some(ref nk) = needle_key {
                    match build_eq_key::<crate::simple_node::SimpleNode>(
                        &XdmItem::Atomic(a.clone()),
                        coll,
                    ) {
                        Ok(k) => k == *nk,
                        Err(_) => false,
                    }
                } else {
                    false
                }
            }
            // Node vs node: fallback to string-value equality (legacy simplification)
            (XdmItem::Node(n), Some(XdmItem::Node(m))) => n.string_value() == m.string_value(),
            // Mixed node/atomic: compare string values; honor collation if provided
            (XdmItem::Node(n), Some(XdmItem::Atomic(b))) => {
                if let Some(c) = coll {
                    c.key(&n.string_value()) == c.key(&as_string(b))
                } else {
                    n.string_value() == as_string(b)
                }
            }
            (XdmItem::Atomic(a), Some(XdmItem::Node(n))) => {
                if let Some(c) = coll {
                    c.key(&as_string(a)) == c.key(&n.string_value())
                } else {
                    as_string(a) == n.string_value()
                }
            }
            _ => false,
        };
        if eq {
            out.push(XdmItem::Atomic(XdmAtomicValue::Integer(i as i64 + 1)));
        }
    }
    Ok(out)
}

// Unified handler for fn:error() 0-3 arities
fn error_default<N: crate::model::XdmNode>(
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    match args.len() {
        0 => Err(Error::dynamic(ErrorCode::FOER0000, "fn:error()")),
        1 => {
            let code = item_to_string(&args[0]);
            if code.is_empty() {
                Err(Error::dynamic(ErrorCode::FOER0000, "fn:error"))
            } else {
                Err(Error::dynamic_err(&code, "fn:error"))
            }
        }
        2 => {
            let code = item_to_string(&args[0]);
            let desc = item_to_string(&args[1]);
            let msg = if desc.is_empty() {
                "fn:error".to_string()
            } else {
                desc
            };
            if code.is_empty() {
                Err(Error::dynamic(ErrorCode::FOER0000, msg))
            } else {
                Err(Error::dynamic_err(&code, msg))
            }
        }
        _ => {
            // 3 or more: third arg (data) ignored for now
            let code = item_to_string(&args[0]);
            let desc = item_to_string(&args[1]);
            let msg = if desc.is_empty() {
                "fn:error".to_string()
            } else {
                desc
            };
            if code.is_empty() {
                Err(Error::dynamic(ErrorCode::FOER0000, msg))
            } else {
                Err(Error::dynamic_err(&code, msg))
            }
        }
    }
}

fn as_string(a: &XdmAtomicValue) -> String {
    match a {
        XdmAtomicValue::String(s) => s.clone(),
        XdmAtomicValue::UntypedAtomic(s) => s.clone(),
        XdmAtomicValue::AnyUri(u) => u.clone(),
        XdmAtomicValue::Boolean(b) => {
            if *b {
                "true".into()
            } else {
                "false".into()
            }
        }
        XdmAtomicValue::Integer(i) => i.to_string(),
        // Numeric subtypes fallback to their numeric representation
        XdmAtomicValue::Long(i) => i.to_string(),
        XdmAtomicValue::Int(i) => i.to_string(),
        XdmAtomicValue::Short(i) => i.to_string(),
        XdmAtomicValue::Byte(i) => i.to_string(),
        XdmAtomicValue::UnsignedLong(i) => i.to_string(),
        XdmAtomicValue::UnsignedInt(i) => i.to_string(),
        XdmAtomicValue::UnsignedShort(i) => i.to_string(),
        XdmAtomicValue::UnsignedByte(i) => i.to_string(),
        XdmAtomicValue::NonPositiveInteger(i) => i.to_string(),
        XdmAtomicValue::NegativeInteger(i) => i.to_string(),
        XdmAtomicValue::NonNegativeInteger(i) => i.to_string(),
        XdmAtomicValue::PositiveInteger(i) => i.to_string(),
        XdmAtomicValue::Double(d) => d.to_string(),
        XdmAtomicValue::Float(f) => f.to_string(),
        XdmAtomicValue::Decimal(d) => d.to_string(),
        XdmAtomicValue::QName { prefix, local, .. } => {
            if let Some(p) = prefix {
                format!("{}:{}", p, local)
            } else {
                local.clone()
            }
        }
        XdmAtomicValue::DateTime(dt) => dt.format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
        XdmAtomicValue::Date { date, tz } => {
            if let Some(off) = tz {
                format!("{}{}", date.format("%Y-%m-%d"), fmt_offset_local(off))
            } else {
                date.format("%Y-%m-%d").to_string()
            }
        }
        XdmAtomicValue::Time { time, tz } => {
            if let Some(off) = tz {
                format!("{}{}", time.format("%H:%M:%S"), fmt_offset_local(off))
            } else {
                time.format("%H:%M:%S").to_string()
            }
        }
        XdmAtomicValue::YearMonthDuration(months) => format_year_month_duration_local(*months),
        XdmAtomicValue::DayTimeDuration(secs) => format_day_time_duration_local(*secs),
        // Binary & lexical string-derived types: return stored lexical form
        XdmAtomicValue::Base64Binary(s)
        | XdmAtomicValue::HexBinary(s)
        | XdmAtomicValue::NormalizedString(s)
        | XdmAtomicValue::Token(s)
        | XdmAtomicValue::Language(s)
        | XdmAtomicValue::Name(s)
        | XdmAtomicValue::NCName(s)
        | XdmAtomicValue::NMTOKEN(s)
        | XdmAtomicValue::Id(s)
        | XdmAtomicValue::IdRef(s)
        | XdmAtomicValue::Entity(s)
        | XdmAtomicValue::Notation(s) => s.clone(),
        // g* date fragments: simple ISO-ish formatting
        XdmAtomicValue::GYear { year, tz } => format!(
            "{:04}{}",
            year,
            tz.map(|o| fmt_offset_local(&o)).unwrap_or_default()
        ),
        XdmAtomicValue::GYearMonth { year, month, tz } => format!(
            "{:04}-{:02}{}",
            year,
            month,
            tz.map(|o| fmt_offset_local(&o)).unwrap_or_default()
        ),
        XdmAtomicValue::GMonth { month, tz } => format!(
            "--{:02}{}",
            month,
            tz.map(|o| fmt_offset_local(&o)).unwrap_or_default()
        ),
        XdmAtomicValue::GMonthDay { month, day, tz } => format!(
            "--{:02}-{:02}{}",
            month,
            day,
            tz.map(|o| fmt_offset_local(&o)).unwrap_or_default()
        ),
        XdmAtomicValue::GDay { day, tz } => format!(
            "---{:02}{}",
            day,
            tz.map(|o| fmt_offset_local(&o)).unwrap_or_default()
        ),
    }
}

fn to_number<N: crate::model::XdmNode>(seq: &XdmSequence<N>) -> Result<f64, Error> {
    if seq.is_empty() {
        return Ok(f64::NAN);
    }
    if seq.len() != 1 {
        return Err(Error::dynamic(ErrorCode::FORG0006, "expects single item"));
    }
    match &seq[0] {
        XdmItem::Atomic(a) => to_number_atomic(a),
        XdmItem::Node(n) => n
            .string_value()
            .parse::<f64>()
            .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid number")),
    }
}

fn to_number_atomic(a: &XdmAtomicValue) -> Result<f64, Error> {
    match a {
        XdmAtomicValue::Integer(i) => Ok(*i as f64),
        XdmAtomicValue::Long(i) => Ok(*i as f64),
        XdmAtomicValue::Int(i) => Ok(*i as f64),
        XdmAtomicValue::Short(i) => Ok(*i as f64),
        XdmAtomicValue::Byte(i) => Ok(*i as f64),
        XdmAtomicValue::UnsignedLong(i) => Ok(*i as f64),
        XdmAtomicValue::UnsignedInt(i) => Ok(*i as f64),
        XdmAtomicValue::UnsignedShort(i) => Ok(*i as f64),
        XdmAtomicValue::UnsignedByte(i) => Ok(*i as f64),
        XdmAtomicValue::NonPositiveInteger(i) => Ok(*i as f64),
        XdmAtomicValue::NegativeInteger(i) => Ok(*i as f64),
        XdmAtomicValue::NonNegativeInteger(i) => Ok(*i as f64),
        XdmAtomicValue::PositiveInteger(i) => Ok(*i as f64),
        XdmAtomicValue::Double(d) => Ok(*d),
        XdmAtomicValue::Float(f) => Ok(*f as f64),
        XdmAtomicValue::Decimal(d) => Ok(*d),
        XdmAtomicValue::UntypedAtomic(s)
        | XdmAtomicValue::String(s)
        | XdmAtomicValue::AnyUri(s) => s
            .parse::<f64>()
            .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid number")),
        XdmAtomicValue::Boolean(b) => Ok(if *b { 1.0 } else { 0.0 }),
        XdmAtomicValue::QName { .. } => Err(Error::dynamic(
            ErrorCode::XPTY0004,
            "cannot cast QName to number",
        )),
        XdmAtomicValue::DateTime(_)
        | XdmAtomicValue::Date { .. }
        | XdmAtomicValue::Time { .. }
        | XdmAtomicValue::YearMonthDuration(_)
        | XdmAtomicValue::DayTimeDuration(_)
        | XdmAtomicValue::Base64Binary(_)
        | XdmAtomicValue::HexBinary(_)
        | XdmAtomicValue::GYear { .. }
        | XdmAtomicValue::GYearMonth { .. }
        | XdmAtomicValue::GMonth { .. }
        | XdmAtomicValue::GMonthDay { .. }
        | XdmAtomicValue::GDay { .. }
        | XdmAtomicValue::NormalizedString(_)
        | XdmAtomicValue::Token(_)
        | XdmAtomicValue::Language(_)
        | XdmAtomicValue::Name(_)
        | XdmAtomicValue::NCName(_)
        | XdmAtomicValue::NMTOKEN(_)
        | XdmAtomicValue::Id(_)
        | XdmAtomicValue::IdRef(_)
        | XdmAtomicValue::Entity(_)
        | XdmAtomicValue::Notation(_) => Err(Error::dynamic(
            ErrorCode::XPTY0004,
            "cannot cast value to number",
        )),
    }
}

fn num_unary<N: crate::model::XdmNode>(
    args: &[XdmSequence<N>],
    f: impl Fn(f64) -> f64,
) -> XdmSequence<N> {
    let n = to_number(&args[0]).unwrap_or(f64::NAN);
    vec![XdmItem::Atomic(XdmAtomicValue::Double(f(n)))]
}

// Precision rounding helpers for multi-arg numeric functions
// Implements XPath 2.0 semantics (simplified: treat value as xs:double, precision as integer; NaN/INF propagate)
fn round_with_precision(value: f64, precision: i64) -> f64 {
    if value.is_nan() || value.is_infinite() {
        return value;
    }
    // Optimization: if precision outside reasonable range, short-circuit
    if precision >= 0 {
        if precision > 15 {
            // beyond double mantissa; value unchanged
            return value;
        }
        let factor = 10_f64.powi(precision as i32);
        (value * factor).round() / factor
    } else {
        let negp = (-precision) as i32;
        if negp > 15 {
            // rounds to 0 (or +/-) at large magnitude beyond precision; emulate by returning 0 with sign
            return 0.0 * value.signum();
        }
        let factor = 10_f64.powi(negp);
        (value / factor).round() * factor
    }
}

fn round_half_to_even_with_precision(value: f64, precision: i64) -> f64 {
    if value.is_nan() || value.is_infinite() {
        return value;
    }
    if precision == 0 {
        // replicate single-arg banker rounding
        let t = value.trunc();
        let frac = value - t;
        if frac.abs() != 0.5 {
            return value.round();
        }
        let ti = t as i64;
        if ti % 2 == 0 { t } else { t + value.signum() }
    } else if precision > 0 {
        if precision > 15 {
            return value;
        }
        let factor = 10_f64.powi(precision as i32);
        let scaled = value * factor;
        banker_round(scaled) / factor
    } else {
        // precision < 0
        let negp = (-precision) as i32;
        if negp > 15 {
            return 0.0 * value.signum();
        }
        let factor = 10_f64.powi(negp);
        banker_round(value / factor) * factor
    }
}

fn banker_round(x: f64) -> f64 {
    let t = x.trunc();
    let frac = x - t;
    // Floating point scaling can yield values like 234.49999999997 for an
    // expected half (.5). Treat near-half within epsilon as exact .5 so the
    // even rule applies (ensures 2.345 -> 2.34 for precision=2 case).
    let frac_abs = frac.abs();
    const HALF: f64 = 0.5;
    const EPS: f64 = 1e-9;
    if (frac_abs - HALF).abs() > EPS {
        return x.round();
    }
    let ti = t as i64;
    if ti % 2 == 0 { t } else { t + x.signum() }
}

// (removed contains_with_collation/starts_with_with_collation/ends_with_with_collation after refactor)

#[doc(hidden)]
pub fn deep_equal_with_collation<N: crate::model::XdmNode>(
    a: &XdmSequence<N>,
    b: &XdmSequence<N>,
    coll: Option<&dyn crate::collation::Collation>,
) -> Result<bool, Error> {
    if a.len() != b.len() {
        return Ok(false);
    }
    for (ia, ib) in a.iter().zip(b.iter()) {
        let eq = match (ia, ib) {
            (XdmItem::Atomic(aa), XdmItem::Atomic(bb)) => {
                atomic_equal_with_collation(aa, bb, coll)?
            }
            (XdmItem::Node(na), XdmItem::Node(nb)) => node_deep_equal(na, nb, coll)?,
            _ => false,
        };
        if !eq {
            return Ok(false);
        }
    }
    Ok(true)
}

fn node_deep_equal<N: crate::model::XdmNode>(
    a: &N,
    b: &N,
    coll: Option<&dyn crate::collation::Collation>,
) -> Result<bool, Error> {
    use crate::model::NodeKind;
    // Kind must match
    if a.kind() != b.kind() {
        return Ok(false);
    }
    // Name (if present) must match (namespace + local)
    if a.name() != b.name() {
        return Ok(false);
    }
    match a.kind() {
        NodeKind::Text
        | NodeKind::Comment
        | NodeKind::ProcessingInstruction
        | NodeKind::Attribute
        | NodeKind::Namespace => {
            let mut sa = a.string_value();
            let mut sb = b.string_value();
            if let Some(c) = coll {
                sa = c.key(&sa);
                sb = c.key(&sb);
            }
            Ok(sa == sb)
        }
        NodeKind::Element | NodeKind::Document => {
            // Attributes unordered
            let mut attrs_a: Vec<(Option<String>, String, String)> = a
                .attributes()
                .into_iter()
                .map(|at| {
                    let name = at.name();
                    let ns = name.as_ref().and_then(|q| q.ns_uri.clone());
                    let local = name.as_ref().map(|q| q.local.clone()).unwrap_or_default();
                    let mut val = at.string_value();
                    if let Some(c) = coll {
                        val = c.key(&val);
                    }
                    (ns, local, val)
                })
                .collect();
            let mut attrs_b: Vec<(Option<String>, String, String)> = b
                .attributes()
                .into_iter()
                .map(|at| {
                    let name = at.name();
                    let ns = name.as_ref().and_then(|q| q.ns_uri.clone());
                    let local = name.as_ref().map(|q| q.local.clone()).unwrap_or_default();
                    let mut val = at.string_value();
                    if let Some(c) = coll {
                        val = c.key(&val);
                    }
                    (ns, local, val)
                })
                .collect();
            attrs_a.sort();
            attrs_b.sort();
            if attrs_a != attrs_b {
                return Ok(false);
            }
            // Namespace nodes unordered (exclude reserved xml prefix if present). Treat as (prefix, uri) pairs.
            let mut ns_a: Vec<(String, String)> = a
                .namespaces()
                .into_iter()
                .filter_map(|ns| {
                    let name = ns.name()?; // prefix stored both in prefix/local
                    if name.prefix.as_deref() == Some("xml") {
                        return None;
                    }
                    Some((name.prefix.unwrap_or_default(), ns.string_value()))
                })
                .collect();
            let mut ns_b: Vec<(String, String)> = b
                .namespaces()
                .into_iter()
                .filter_map(|ns| {
                    let name = ns.name()?;
                    if name.prefix.as_deref() == Some("xml") {
                        return None;
                    }
                    Some((name.prefix.unwrap_or_default(), ns.string_value()))
                })
                .collect();
            ns_a.sort();
            ns_b.sort();
            if ns_a != ns_b {
                return Ok(false);
            }
            // Children ordered
            let ca = a.children();
            let cb = b.children();
            if ca.len() != cb.len() {
                return Ok(false);
            }
            for (child_a, child_b) in ca.iter().zip(cb.iter()) {
                if !node_deep_equal(child_a, child_b, coll)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
    }
}

// distinct-values core implementation shared by 1- and 2-arg variants.
fn distinct_values_impl<N: crate::model::XdmNode>(
    _ctx: &CallCtx<N>,
    seq: &XdmSequence<N>,
    coll: Option<&dyn crate::collation::Collation>,
) -> Result<XdmSequence<N>, Error> {
    use crate::eq::{EqKey, build_eq_key};
    use std::collections::HashSet;
    let mut seen: HashSet<EqKey> = HashSet::new();
    let mut out: XdmSequence<N> = Vec::new();
    for it in seq {
        match it {
            XdmItem::Node(_) => {
                return Err(Error::dynamic(
                    ErrorCode::XPTY0004,
                    "distinct-values on non-atomic item",
                ));
            }
            XdmItem::Atomic(a) => {
                let tmp: XdmItem<N> = XdmItem::Atomic(a.clone());
                let key = build_eq_key(&tmp, coll)?;
                if seen.insert(key) {
                    out.push(tmp);
                }
            }
        }
    }
    Ok(out)
}

fn atomic_equal_with_collation(
    a: &XdmAtomicValue,
    b: &XdmAtomicValue,
    coll: Option<&dyn crate::collation::Collation>,
) -> Result<bool, Error> {
    use crate::eq::build_eq_key;
    // Helper generic to appease type inference for XdmNode parameter.
    fn key_for<'x, N: crate::model::XdmNode>(
        v: &'x XdmAtomicValue,
        coll: Option<&dyn crate::collation::Collation>,
    ) -> Result<crate::eq::EqKey, Error> {
        let item: XdmItem<N> = XdmItem::Atomic(v.clone());
        build_eq_key(&item, coll)
    }
    // We don't know the node type here (deep-equal may compare atomics only) – use SimpleNode phantom by leveraging that EqKey building ignores node internals for atomic case.
    // SAFETY: build_eq_key only pattern matches &XdmItem and for Atomic branch does not access node-specific APIs.
    type AnyNode = crate::simple_node::SimpleNode;
    let ka = key_for::<AnyNode>(a, coll)?;
    let kb = key_for::<AnyNode>(b, coll)?;
    Ok(ka == kb)
}

// ===== Helpers (M7 Regex) =====
fn get_regex_provider<N>(ctx: &CallCtx<N>) -> std::sync::Arc<dyn crate::runtime::RegexProvider> {
    if let Some(p) = &ctx.regex {
        p.clone()
    } else {
        std::sync::Arc::new(crate::runtime::FancyRegexProvider)
    }
}

fn regex_matches<N>(
    ctx: &CallCtx<N>,
    input: &str,
    pattern: &str,
    flags: &str,
) -> Result<bool, Error> {
    let provider = get_regex_provider(ctx);
    let normalized = validate_regex_flags(flags)?;
    reject_backref_in_char_class(pattern)?;
    provider.matches(pattern, &normalized, input)
}

fn regex_replace<N>(
    ctx: &CallCtx<N>,
    input: &str,
    pattern: &str,
    repl: &str,
    flags: &str,
) -> Result<String, Error> {
    let provider = get_regex_provider(ctx);
    let normalized = validate_regex_flags(flags)?;
    reject_backref_in_char_class(pattern)?;
    provider.replace(pattern, &normalized, input, repl)
}

fn regex_tokenize<N>(
    ctx: &CallCtx<N>,
    input: &str,
    pattern: &str,
    flags: &str,
) -> Result<Vec<String>, Error> {
    let provider = get_regex_provider(ctx);
    let normalized = validate_regex_flags(flags)?;
    reject_backref_in_char_class(pattern)?;
    provider.tokenize(pattern, &normalized, input)
}

// Validate XPath 2.0 regex flags: i (case-insensitive), m (multiline), s (dotall), x (free-spacing)
// Return normalized string with duplicates removed preserving input order of first occurrences.
fn validate_regex_flags(flags: &str) -> Result<String, Error> {
    if flags.is_empty() {
        return Ok(String::new());
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut out = String::new();
    for ch in flags.chars() {
        match ch {
            'i' | 'm' | 's' | 'x' => {
                if seen.insert(ch) {
                    out.push(ch);
                }
            }
            _ => {
                return Err(Error::dynamic(
                    crate::runtime::ErrorCode::FORX0001,
                    format!("unsupported regex flag: {ch}"),
                ));
            }
        }
    }
    Ok(out)
}

// Quick scan to reject illegal backreferences inside character classes like [$1] which XPath 2.0 forbids.
// We don't implement a full parser here; a conservative heuristic is sufficient: if we see a '[' ... ']' region
// and a '$' followed by a digit within it, we raise FORX0002.
fn reject_backref_in_char_class(pattern: &str) -> Result<(), Error> {
    let bytes = pattern.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            // enter class; handle nested \] and escaped \[
            i += 1;
            while i < bytes.len() && bytes[i] != b']' {
                if bytes[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if bytes[i] == b'$'
                    && i + 1 < bytes.len()
                    && (bytes[i + 1] as char).is_ascii_digit()
                {
                    return Err(Error::dynamic(
                        crate::runtime::ErrorCode::FORX0002,
                        "backreference not allowed in character class",
                    ));
                }
                i += 1;
            }
        }
        i += 1;
    }
    Ok(())
}

// Banker (half-to-even) rounding for f64 values matching fn:round-half-to-even semantics used by substring/subsequence.
fn round_half_to_even_f64(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let ax = x.abs();
    let floor = ax.floor();
    let frac = ax - floor;
    const EPS: f64 = 1e-12; // tolerant epsilon for .5 detection
    let rounded_abs = if (frac - 0.5).abs() < EPS {
        // tie -> even
        if ((floor as i64) & 1) == 0 {
            floor
        } else {
            floor + 1.0
        }
    } else if frac < 0.5 {
        floor
    } else {
        floor + 1.0
    };
    if x.is_sign_negative() {
        -rounded_abs
    } else {
        rounded_abs
    }
}

fn minmax_impl<N: crate::model::XdmNode>(
    ctx: &CallCtx<N>,
    seq: &XdmSequence<N>,
    coll: Option<&dyn crate::collation::Collation>,
    is_min: bool,
) -> Result<XdmSequence<N>, Error> {
    if seq.is_empty() {
        return Ok(vec![]);
    }
    // numeric if all numeric, else string using collation (default or provided)
    let mut all_num = true;
    let mut acc_num = if is_min {
        f64::INFINITY
    } else {
        f64::NEG_INFINITY
    };
    for it in seq {
        match it {
            XdmItem::Atomic(a) => match to_number_atomic(a) {
                Ok(n) => {
                    if n.is_nan() {
                        return Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(f64::NAN))]);
                    }
                    if is_min {
                        acc_num = acc_num.min(n)
                    } else {
                        acc_num = acc_num.max(n)
                    }
                }
                Err(_) => {
                    all_num = false;
                    break;
                }
            },
            _ => {
                all_num = false;
                break;
            }
        }
    }
    if all_num {
        // Re-run with detailed kind inference to decide result type & value (acc_num already min/max as f64)
        let mut kind = NumericKind::Integer;
        for it in seq {
            if let XdmItem::Atomic(a) = it {
                if let Some((nk, num)) = classify_numeric(a)? {
                    if nk == NumericKind::Double && num.is_nan() {
                        return Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(f64::NAN))]);
                    }
                    if nk == NumericKind::Float && num.is_nan() {
                        return Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(f64::NAN))]);
                    }
                    kind = kind.promote(nk);
                }
            }
        }
        let out = match kind {
            NumericKind::Integer => XdmAtomicValue::Integer(acc_num as i64),
            NumericKind::Decimal => XdmAtomicValue::Decimal(acc_num),
            NumericKind::Float => XdmAtomicValue::Float(acc_num as f32),
            NumericKind::Double => XdmAtomicValue::Double(acc_num),
        };
        return Ok(vec![XdmItem::Atomic(out)]);
    }
    // String branch
    // Ensure owned Arc lives while function executes (store optionally)
    let mut owned_coll: Option<std::sync::Arc<dyn crate::collation::Collation>> = None;
    let effective_coll: Option<&dyn crate::collation::Collation> = if let Some(c) = coll {
        Some(c)
    } else {
        let k = crate::collation::resolve_collation(
            &ctx.dyn_ctx,
            ctx.default_collation.as_ref(),
            None,
        )?;
        match k {
            crate::collation::CollationKind::Codepoint(a)
            | crate::collation::CollationKind::Other(a) => {
                owned_coll = Some(a);
            }
        }
        owned_coll.as_deref()
    };
    let mut iter = seq.iter();
    let first = match iter.next() {
        Some(XdmItem::Atomic(a)) => as_string(a),
        Some(XdmItem::Node(n)) => n.string_value(),
        None => String::new(), // unreachable due to non-empty
    };
    if let Some(c) = effective_coll {
        let mut best_orig = first.clone();
        let mut best_key = c.key(&first);
        for it in iter {
            let s = match it {
                XdmItem::Atomic(a) => as_string(a),
                XdmItem::Node(n) => n.string_value(),
            };
            let k = c.key(&s);
            let ord = k.cmp(&best_key);
            if (is_min && ord == core::cmp::Ordering::Less)
                || (!is_min && ord == core::cmp::Ordering::Greater)
            {
                best_key = k;
                best_orig = s;
            }
        }
        let _arc_hold = &owned_coll; // keep alive
        return Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(best_orig))]);
    } else {
        let best = iter.fold(first, |acc, it| {
            let s = match it {
                XdmItem::Atomic(a) => as_string(a),
                XdmItem::Node(n) => n.string_value(),
            };
            let ord = s.cmp(&acc);
            if is_min {
                if ord == core::cmp::Ordering::Less {
                    s
                } else {
                    acc
                }
            } else if ord == core::cmp::Ordering::Greater {
                s
            } else {
                acc
            }
        });
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(best))])
    }
}

// ===== Aggregate numeric typing helpers (WP-A) =====
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum NumericKind {
    Integer,
    Decimal,
    Float,
    Double,
}

impl NumericKind {
    fn promote(self, other: NumericKind) -> NumericKind {
        use NumericKind::*;
        match (self, other) {
            (Double, _) | (_, Double) => Double,
            (Float, _) | (_, Float) => {
                if matches!(self, Double) || matches!(other, Double) {
                    Double
                } else {
                    Float
                }
            }
            (Decimal, _) | (_, Decimal) => match (self, other) {
                (Integer, Decimal) | (Decimal, Integer) | (Decimal, Decimal) => Decimal,
                (Decimal, Float) | (Float, Decimal) => Float,
                (Decimal, Double) | (Double, Decimal) => Double,
                _ => Decimal,
            },
            (Integer, Integer) => Integer,
        }
    }
}

fn classify_numeric(a: &XdmAtomicValue) -> Result<Option<(NumericKind, f64)>, Error> {
    use XdmAtomicValue::*;
    Ok(match a {
        Integer(i) => Some((NumericKind::Integer, *i as f64)),
        Long(i) => Some((NumericKind::Integer, *i as f64)),
        Int(i) => Some((NumericKind::Integer, *i as f64)),
        Short(i) => Some((NumericKind::Integer, *i as f64)),
        Byte(i) => Some((NumericKind::Integer, *i as f64)),
        UnsignedLong(i) => Some((NumericKind::Integer, *i as f64)),
        UnsignedInt(i) => Some((NumericKind::Integer, *i as f64)),
        UnsignedShort(i) => Some((NumericKind::Integer, *i as f64)),
        UnsignedByte(i) => Some((NumericKind::Integer, *i as f64)),
        NonPositiveInteger(i) => Some((NumericKind::Integer, *i as f64)),
        NegativeInteger(i) => Some((NumericKind::Integer, *i as f64)),
        NonNegativeInteger(i) => Some((NumericKind::Integer, *i as f64)),
        PositiveInteger(i) => Some((NumericKind::Integer, *i as f64)),
        Decimal(d) => Some((NumericKind::Decimal, *d)),
        Float(f) => Some((NumericKind::Float, *f as f64)),
        Double(d) => Some((NumericKind::Double, *d)),
        UntypedAtomic(s) | String(s) | AnyUri(s) => {
            // Attempt numeric cast; if fails treat as non-numeric (caller will error)
            if let Ok(parsed) = s.parse::<f64>() {
                Some((NumericKind::Double, parsed))
            } else {
                None
            }
        }
        Boolean(b) => Some((NumericKind::Integer, if *b { 1.0 } else { 0.0 })),
        _ => None,
    })
}

fn a_as_i128(a: &XdmAtomicValue) -> Option<i128> {
    use XdmAtomicValue::*;
    Some(match a {
        Integer(i) => *i as i128,
        Long(i) => *i as i128,
        Int(i) => *i as i128,
        Short(i) => *i as i128,
        Byte(i) => *i as i128,
        UnsignedLong(i) => *i as i128,
        UnsignedInt(i) => *i as i128,
        UnsignedShort(i) => *i as i128,
        UnsignedByte(i) => *i as i128,
        NonPositiveInteger(i) => *i as i128,
        NegativeInteger(i) => *i as i128,
        NonNegativeInteger(i) => *i as i128,
        PositiveInteger(i) => *i as i128,
        Boolean(b) => {
            if *b {
                1
            } else {
                0
            }
        }
        _ => return None,
    })
}

fn now_in_effective_tz<N>(ctx: &CallCtx<N>) -> chrono::DateTime<chrono::FixedOffset> {
    // Base instant: context-provided 'now' or system time in UTC
    let base = if let Some(n) = ctx.dyn_ctx.now {
        n
    } else {
        // Use local offset if available; fallback to UTC+00:00
        let utc = chrono::Utc::now();
        let fixed = chrono::FixedOffset::east_opt(0).unwrap();
        utc.with_timezone(&fixed)
    };
    if let Some(tz) = ctx.dyn_ctx.timezone_override {
        base.with_timezone(&tz)
    } else {
        base
    }
}

// ===== Helpers for M8b component functions =====
fn parse_offset(tz: &str) -> Option<ChronoFixedOffset> {
    if tz.len() != 6 {
        return None;
    }
    let sign = &tz[0..1];
    let hours: i32 = tz[1..3].parse().ok()?;
    let mins: i32 = tz[4..6].parse().ok()?;
    let total = hours * 3600 + mins * 60;
    let secs = if sign == "-" { -total } else { total };
    chrono::FixedOffset::east_opt(secs)
}

fn get_datetime<N: crate::model::XdmNode>(
    seq: &XdmSequence<N>,
) -> Result<Option<ChronoDateTime<ChronoFixedOffset>>, Error> {
    if seq.is_empty() {
        return Ok(None);
    }
    match &seq[0] {
        XdmItem::Atomic(XdmAtomicValue::DateTime(dt)) => Ok(Some(*dt)),
        XdmItem::Atomic(XdmAtomicValue::String(s))
        | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            ChronoDateTime::parse_from_rfc3339(s)
                .map(Some)
                .map_err(|_| Error::dynamic_err("err:FORG0001", "invalid xs:dateTime"))
        }
        XdmItem::Node(n) => ChronoDateTime::parse_from_rfc3339(&n.string_value())
            .map(Some)
            .map_err(|_| Error::dynamic_err("err:FORG0001", "invalid xs:dateTime")),
        _ => Err(Error::dynamic_err("err:XPTY0004", "not a dateTime")),
    }
}

fn get_time<N: crate::model::XdmNode>(
    seq: &XdmSequence<N>,
) -> Result<Option<(NaiveTime, Option<ChronoFixedOffset>)>, Error> {
    if seq.is_empty() {
        return Ok(None);
    }
    match &seq[0] {
        XdmItem::Atomic(XdmAtomicValue::Time { time, tz }) => Ok(Some((*time, *tz))),
        XdmItem::Atomic(XdmAtomicValue::String(s))
        | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => crate::temporal::parse_time_lex(s)
            .map(Some)
            .map_err(|_| Error::dynamic_err("err:FORG0001", "invalid xs:time")),
        XdmItem::Node(n) => crate::temporal::parse_time_lex(&n.string_value())
            .map(Some)
            .map_err(|_| Error::dynamic_err("err:FORG0001", "invalid xs:time")),
        _ => Err(Error::dynamic_err("err:XPTY0004", "not a time")),
    }
}

fn format_year_month_duration_local(months: i32) -> String {
    if months == 0 {
        return "P0M".to_string();
    }
    let neg = months < 0;
    let mut m = months.abs();
    let y = m / 12;
    m %= 12;
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    out.push('P');
    if y != 0 {
        out.push_str(&format!("{}Y", y));
    }
    if m != 0 {
        out.push_str(&format!("{}M", m));
    }
    if y == 0 && m == 0 {
        out.push('0');
        out.push('M');
    }
    out
}

fn format_day_time_duration_local(total_secs: i64) -> String {
    if total_secs == 0 {
        return "PT0S".to_string();
    }
    let neg = total_secs < 0;
    let mut s = total_secs.abs();
    let days = s / (24 * 3600);
    s %= 24 * 3600;
    let hours = s / 3600;
    s %= 3600;
    let mins = s / 60;
    s %= 60;
    let secs = s;
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    out.push('P');
    if days != 0 {
        out.push_str(&format!("{}D", days));
    }
    if hours != 0 || mins != 0 || secs != 0 {
        out.push('T');
    }
    if hours != 0 {
        out.push_str(&format!("{}H", hours));
    }
    if mins != 0 {
        out.push_str(&format!("{}M", mins));
    }
    if secs != 0 {
        out.push_str(&format!("{}S", secs));
    }
    out
}

fn fmt_offset_local(off: &ChronoFixedOffset) -> String {
    let secs = off.local_minus_utc();
    let sign = if secs < 0 { '-' } else { '+' };
    let mut s = secs.abs();
    let hours = s / 3600;
    s %= 3600;
    let mins = s / 60;
    format!("{}{:02}:{:02}", sign, hours, mins)
}

fn parse_xs_date_local(s: &str) -> Result<(NaiveDate, Option<ChronoFixedOffset>), ()> {
    if s.ends_with('Z') || s.ends_with('z') {
        let d = &s[..s.len() - 1];
        let date = NaiveDate::parse_from_str(d, "%Y-%m-%d").map_err(|_| ())?;
        return Ok((date, Some(ChronoFixedOffset::east_opt(0).ok_or(())?)));
    }
    if let Some(pos) = s.rfind(['+', '-'])
        && pos >= 10
    {
        let (d, tzs) = s.split_at(pos);
        let date = NaiveDate::parse_from_str(d, "%Y-%m-%d").map_err(|_| ())?;
        let off = parse_offset(tzs).ok_or(())?;
        return Ok((date, Some(off)));
    }
    let date = NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| ())?;
    Ok((date, None))
}

// ===== Additional Helpers for Task 11 =====

fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_space = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !in_space {
                out.push(' ');
                in_space = true;
            }
        } else {
            out.push(ch);
            in_space = false;
        }
    }
    if out.starts_with(' ') {
        out.remove(0);
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

fn replace_whitespace(s: &str) -> String {
    s.chars()
        .map(|c| {
            if matches!(c, '\u{0009}' | '\u{000A}' | '\u{000D}') {
                ' '
            } else {
                c
            }
        })
        .collect()
}

fn parse_qname_lexical(s: &str) -> Result<(Option<String>, String), ()> {
    if s.is_empty() {
        return Err(());
    }
    if let Some(pos) = s.find(':') {
        let (p, l) = s.split_at(pos);
        let local = &l[1..];
        if !is_valid_ncname(p) || !is_valid_ncname(local) {
            return Err(());
        }
        Ok((Some(p.to_string()), local.to_string()))
    } else {
        if !is_valid_ncname(s) {
            return Err(());
        }
        Ok((None, s.to_string()))
    }
}

fn is_valid_ncname(s: &str) -> bool {
    // ASCII-only approximation: [_A-Za-z] [_A-Za-z0-9.-]* (no colon)
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if first == ':' || !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }
    for ch in chars {
        if ch == ':' || !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.') {
            return false;
        }
    }
    true
}

fn is_valid_language(s: &str) -> bool {
    // Simple BCP47-ish: 1-8 alpha, then (- 1-8 alnum) repeated
    let mut parts = s.split('-');
    if let Some(first) = parts.next() {
        if !(1..=8).contains(&first.len()) || !first.chars().all(|c| c.is_ascii_alphabetic()) {
            return false;
        }
    } else {
        return false;
    }
    for p in parts {
        if !(1..=8).contains(&p.len()) || !p.chars().all(|c| c.is_ascii_alphanumeric()) {
            return false;
        }
    }
    true
}

fn parse_year_month_duration_months(s: &str) -> Result<i32, ()> {
    // Pattern: -?P(\d+Y)?(\d+M)? with at least one present
    let s = s.trim();
    if s.is_empty() {
        return Err(());
    }
    let neg = s.starts_with('-');
    let body = if neg { &s[1..] } else { s };
    if !body.starts_with('P') {
        return Err(());
    }
    let mut years: i32 = 0;
    let mut months: i32 = 0;
    let mut cur = &body[1..];
    let mut consumed_any = false;
    while !cur.is_empty() {
        // find next number
        let mut i = 0;
        while i < cur.len() && cur.as_bytes()[i].is_ascii_digit() {
            i += 1;
        }
        if i == 0 {
            break;
        }
        let n: i32 = cur[..i].parse().map_err(|_| ())?;
        cur = &cur[i..];
        if cur.starts_with('Y') {
            years = n;
            cur = &cur[1..];
            consumed_any = true;
        } else if cur.starts_with('M') {
            months = n;
            cur = &cur[1..];
            consumed_any = true;
        } else {
            return Err(());
        }
    }
    if !consumed_any || !cur.is_empty() {
        return Err(());
    }
    let total = years
        .checked_mul(12)
        .ok_or(())?
        .checked_add(months)
        .ok_or(())?;
    Ok(if neg { -total } else { total })
}

fn parse_day_time_duration_secs(s: &str) -> Result<i64, ()> {
    // Pattern: -?P(\d+D)?(T(\d+H)?(\d+M)?(\d+(\.\d+)?S)?)?
    let s = s.trim();
    if s.is_empty() {
        return Err(());
    }
    let neg = s.starts_with('-');
    let body = if neg { &s[1..] } else { s };
    if !body.starts_with('P') {
        return Err(());
    }
    let mut cur = &body[1..];
    let mut days: i64 = 0;
    let mut hours: i64 = 0;
    let mut mins: i64 = 0;
    let mut secs: f64 = 0.0;
    // days
    if !cur.is_empty() {
        let mut i = 0;
        while i < cur.len() && cur.as_bytes()[i].is_ascii_digit() {
            i += 1;
        }
        if i > 0 && cur[i..].starts_with('D') {
            days = cur[..i].parse().map_err(|_| ())?;
            cur = &cur[i + 1..];
        }
    }
    if cur.starts_with('T') {
        cur = &cur[1..];
        // hours
        if !cur.is_empty() {
            let mut i = 0;
            while i < cur.len() && cur.as_bytes()[i].is_ascii_digit() {
                i += 1;
            }
            if i > 0 && cur[i..].starts_with('H') {
                hours = cur[..i].parse().map_err(|_| ())?;
                cur = &cur[i + 1..];
            }
        }
        // minutes
        if !cur.is_empty() {
            let mut i = 0;
            while i < cur.len() && cur.as_bytes()[i].is_ascii_digit() {
                i += 1;
            }
            if i > 0 && cur[i..].starts_with('M') {
                mins = cur[..i].parse().map_err(|_| ())?;
                cur = &cur[i + 1..];
            }
        }
        // seconds (allow fractional)
        if !cur.is_empty() {
            let mut i = 0;
            while i < cur.len() && (cur.as_bytes()[i].is_ascii_digit() || cur.as_bytes()[i] == b'.')
            {
                i += 1;
            }
            if i > 0 && cur[i..].starts_with('S') {
                secs = cur[..i].parse().map_err(|_| ())?;
                cur = &cur[i + 1..];
            }
        }
    }
    if !cur.is_empty() {
        return Err(());
    }
    let mut total = days
        .checked_mul(24 * 3600)
        .ok_or(())?
        .checked_add(hours.checked_mul(3600).ok_or(())?)
        .ok_or(())?
        .checked_add(mins.checked_mul(60).ok_or(())?)
        .ok_or(())? as f64
        + secs;
    if neg {
        total = -total;
    }
    Ok(total.trunc() as i64)
}

fn int_subtype_i64<N: crate::model::XdmNode>(
    args: &[XdmSequence<N>],
    min: i64,
    max: i64,
    mk: impl Fn(i64) -> XdmAtomicValue,
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::dynamic(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = item_to_string(&args[0]).trim().to_string();
    let v: i64 = s
        .parse()
        .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid integer"))?;
    if v < min || v > max {
        return Err(Error::dynamic(ErrorCode::FORG0001, "out of range"));
    }
    Ok(vec![XdmItem::Atomic(mk(v))])
}

fn uint_subtype_u128<N: crate::model::XdmNode>(
    args: &[XdmSequence<N>],
    min: u128,
    max: u128,
    mk: impl Fn(u128) -> XdmAtomicValue,
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::dynamic(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = item_to_string(&args[0]).trim().to_string();
    if s.starts_with('-') {
        return Err(Error::dynamic(ErrorCode::FORG0001, "negative not allowed"));
    }
    let v: u128 = s
        .parse()
        .map_err(|_| Error::dynamic(ErrorCode::FORG0001, "invalid unsigned integer"))?;
    if v < min || v > max {
        return Err(Error::dynamic(ErrorCode::FORG0001, "out of range"));
    }
    Ok(vec![XdmItem::Atomic(mk(v))])
}

fn str_name_like<N: crate::model::XdmNode>(
    args: &[XdmSequence<N>],
    require_start: bool,
    allow_colon: bool,
    mk: impl Fn(String) -> XdmAtomicValue,
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::dynamic(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = collapse_whitespace(&item_to_string(&args[0]));
    // Simplified validation
    if require_start {
        let mut chars = s.chars();
        if let Some(first) = chars.next() {
            if !(first == '_' || first.is_ascii_alphabetic() || (allow_colon && first == ':')) {
                return Err(Error::dynamic(ErrorCode::FORG0001, "invalid Name"));
            }
            for ch in chars {
                if !(ch.is_ascii_alphanumeric()
                    || ch == '_'
                    || ch == '-'
                    || ch == '.'
                    || (allow_colon && ch == ':'))
                {
                    return Err(Error::dynamic(ErrorCode::FORG0001, "invalid Name"));
                }
            }
        } else {
            return Err(Error::dynamic(ErrorCode::FORG0001, "invalid Name"));
        }
    }
    Ok(vec![XdmItem::Atomic(mk(s))])
}
