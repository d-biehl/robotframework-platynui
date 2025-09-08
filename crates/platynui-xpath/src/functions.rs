//! Function families per XQuery and XPath Functions and Operators.
//! Minimal default registry with a few core functions to bootstrap evaluator tests.

use crate::runtime::{CallCtx, Error, FunctionRegistry};
use crate::xdm::{ExpandedName, XdmAtomicValue, XdmItem, XdmSequence};

const FNS: &str = "http://www.w3.org/2005/xpath-functions";

fn ebv<N>(seq: &XdmSequence<N>) -> Result<bool, Error> {
    match seq.len() {
        0 => Ok(false),
        1 => match &seq[0] {
            XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => Ok(*b),
            XdmItem::Atomic(XdmAtomicValue::String(s)) => Ok(!s.is_empty()),
            XdmItem::Atomic(XdmAtomicValue::Integer(i)) => Ok(*i != 0),
            XdmItem::Atomic(XdmAtomicValue::Double(d)) => Ok(*d != 0.0 && !d.is_nan()),
            XdmItem::Atomic(XdmAtomicValue::Float(f)) => Ok(*f != 0.0 && !f.is_nan()),
            XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => Ok(!s.is_empty()),
            XdmItem::Atomic(_) => Err(Error::dynamic_err(
                "err:FORG0006",
                "EBV for this atomic type not supported yet",
            )),
            XdmItem::Node(_) => Ok(true),
        },
        _ => Err(Error::dynamic_err(
            "err:FORG0006",
            "EBV of sequence with more than one item",
        )),
    }
}

type FnSig<N> = fn(&CallCtx<N>, &[XdmSequence<N>]) -> Result<XdmSequence<N>, Error>;

pub fn default_function_registry<N: 'static + Send + Sync + crate::model::XdmNode + Clone>()
-> FunctionRegistry<N> {
    let mut reg = FunctionRegistry::new();
    // helper to register under default namespace
    let mut add = |local: &str, arity: usize, f: FnSig<N>| {
        let fun = std::sync::Arc::new(move |ctx: &CallCtx<N>, args: &[XdmSequence<N>]| f(ctx, args));
        reg.register(
            ExpandedName {
                ns_uri: Some(FNS.to_string()),
                local: local.to_string(),
            },
            arity,
            fun.clone(),
        );
        reg.register(
            ExpandedName {
                ns_uri: None,
                local: local.to_string(),
            },
            arity,
            fun,
        );
    };

    // ===== Core booleans =====
    add("true", 0, |_ctx, _args| {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(true))])
    });
    add("false", 0, |_ctx, _args| {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(false))])
    });
    add("not", 1, |_ctx, args| {
        let b = ebv(&args[0])?;
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(!b))])
    });

    // ===== String family =====
    add("string", 1, |_ctx, args| {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(
            item_to_string(&args[0]),
        ))])
    });
    add("string-length", 1, |_ctx, args| {
        let s = item_to_string(&args[0]);
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
            s.chars().count() as i64,
        ))])
    });
    for ar in 2..=5 {
        add("concat", ar, |_ctx, args| {
            let mut out = String::new();
            for a in args {
                out.push_str(&item_to_string(a));
            }
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(out))])
        });
    }
    // contains($arg1, $arg2) — collation-aware (default collation)
    add("contains", 2, |ctx, args| {
        let s = item_to_string(&args[0]);
        let sub = item_to_string(&args[1]);
        let coll = resolve_default_collation_fn(ctx);
        let b = contains_with_collation(&s, &sub, coll.as_deref());
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))])
    });
    // contains($arg1, $arg2, $collation)
    add("contains", 3, |ctx, args| {
        let s = item_to_string(&args[0]);
        let sub = item_to_string(&args[1]);
        let uri = item_to_string(&args[2]);
        let coll = resolve_named_collation_fn(ctx, &uri)?;
        let b = contains_with_collation(&s, &sub, Some(coll.as_ref()));
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))])
    });
    add("starts-with", 2, |ctx, args| {
        let s = item_to_string(&args[0]);
        let sub = item_to_string(&args[1]);
        let coll = resolve_default_collation_fn(ctx);
        let b = starts_with_with_collation(&s, &sub, coll.as_deref());
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))])
    });
    add("starts-with", 3, |ctx, args| {
        let s = item_to_string(&args[0]);
        let sub = item_to_string(&args[1]);
        let uri = item_to_string(&args[2]);
        let coll = resolve_named_collation_fn(ctx, &uri)?;
        let b = starts_with_with_collation(&s, &sub, Some(coll.as_ref()));
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))])
    });
    add("ends-with", 2, |ctx, args| {
        let s = item_to_string(&args[0]);
        let sub = item_to_string(&args[1]);
        let coll = resolve_default_collation_fn(ctx);
        let b = ends_with_with_collation(&s, &sub, coll.as_deref());
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))])
    });
    add("ends-with", 3, |ctx, args| {
        let s = item_to_string(&args[0]);
        let sub = item_to_string(&args[1]);
        let uri = item_to_string(&args[2]);
        let coll = resolve_named_collation_fn(ctx, &uri)?;
        let b = ends_with_with_collation(&s, &sub, Some(coll.as_ref()));
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))])
    });
    add("substring", 2, |_ctx, args| {
        let s = item_to_string(&args[0]);
        let start = to_number(&args[1])?; // 1-based
        let from = (start.floor() as isize - 1).max(0) as usize;
        let out: String = s.chars().skip(from).collect();
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(out))])
    });
    add("substring", 3, |_ctx, args| {
        let s = item_to_string(&args[0]);
        let start = to_number(&args[1])?;
        let len = to_number(&args[2])?;
        let from = (start.floor() as isize - 1).max(0) as usize;
        let take = len.floor().max(0.0) as usize;
        let out: String = s.chars().skip(from).take(take).collect();
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(out))])
    });

    // substring-before/after
    add("substring-before", 2, |_ctx, args| {
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
    add("substring-after", 2, |_ctx, args| {
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

    // normalize-space (1-arg variant)
    add("normalize-space", 1, |_ctx, args| {
        let s = item_to_string(&args[0]);
        let mut out = String::new();
        let mut in_space = true; // leading spaces skipped
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
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(out))])
    });

    // translate($s, $map, $trans)
    add("translate", 3, |_ctx, args| {
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
    add("lower-case", 1, |_ctx, args| {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(
            item_to_string(&args[0]).to_lowercase(),
        ))])
    });
    add("upper-case", 1, |_ctx, args| {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(
            item_to_string(&args[0]).to_uppercase(),
        ))])
    });

    // string-join($seq, $sep)
    add("string-join", 2, |_ctx, args| {
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

    // ===== Numeric family =====
    add("abs", 1, |_ctx, args| Ok(num_unary(args, |n| n.abs())));
    add("floor", 1, |_ctx, args| Ok(num_unary(args, |n| n.floor())));
    add("ceiling", 1, |_ctx, args| Ok(num_unary(args, |n| n.ceil())));
    add("round", 1, |_ctx, args| Ok(num_unary(args, |n| n.round())));
    add("sum", 1, |_ctx, args| {
        let mut total = 0.0;
        for it in &args[0] {
            if let XdmItem::Atomic(a) = it {
                total += to_number_atomic(a)?;
            }
        }
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(total))])
    });
    add("avg", 1, |_ctx, args| {
        let mut total = 0.0;
        let mut c = 0.0;
        for it in &args[0] {
            if let XdmItem::Atomic(a) = it {
                total += to_number_atomic(a)?;
                c += 1.0;
            }
        }
        if c == 0.0 {
            Ok(vec![])
        } else {
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(total / c))])
        }
    });

    // ===== Sequence family =====
    add("empty", 1, |_ctx, args| {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(
            args[0].is_empty(),
        ))])
    });
    add("exists", 1, |_ctx, args| {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(
            !args[0].is_empty(),
        ))])
    });
    add("count", 1, |_ctx, args| {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(
            args[0].len() as i64,
        ))])
    });
    add("reverse", 1, |_ctx, args| {
        let s: XdmSequence<N> = args[0].iter().cloned().rev().collect();
        Ok(s)
    });
    add("subsequence", 2, |_ctx, args| {
        let s = &args[0];
        let start = to_number(&args[1])?;
        let from = (start.floor() as isize - 1).max(0) as usize;
        Ok(s.iter().skip(from).cloned().collect())
    });
    add("subsequence", 3, |_ctx, args| {
        let s = &args[0];
        let start = to_number(&args[1])?;
        let len = to_number(&args[2])?;
        let from = (start.floor() as isize - 1).max(0) as usize;
        let take = len.floor().max(0.0) as usize;
        Ok(s.iter().skip(from).take(take).cloned().collect())
    });

    // distinct-values($seq as xs:anyAtomicType*)
    add("distinct-values", 1, |_ctx, args| {
        use std::collections::HashSet;
        let mut seen: HashSet<String> = HashSet::new();
        let mut out: XdmSequence<N> = Vec::new();
        for it in &args[0] {
            let (key, push_item) = match it {
                XdmItem::Atomic(a) => {
                    let s = as_string(a);
                    (s.clone(), XdmItem::Atomic(XdmAtomicValue::String(s)))
                }
                XdmItem::Node(n) => {
                    let s = n.string_value();
                    (s.clone(), XdmItem::Atomic(XdmAtomicValue::String(s)))
                }
            };
            if seen.insert(key) {
                out.push(push_item);
            }
        }
        Ok(out)
    });

    // index-of($seq, $search)
    add("index-of", 2, |_ctx, args| {
        let mut out: XdmSequence<N> = Vec::new();
        // Use string compare if either side is non-numeric; else numeric equality
        for (i, it) in args[0].iter().enumerate() {
            let eq = match (it, args[1].first()) {
                (XdmItem::Atomic(a), Some(XdmItem::Atomic(b))) => {
                    // numeric if possible, else string
                    match (to_number_atomic(a), to_number_atomic(b)) {
                        (Ok(na), Ok(nb)) => na == nb,
                        _ => as_string(a) == as_string(b),
                    }
                }
                (XdmItem::Node(n), Some(XdmItem::Node(m))) => n.string_value() == m.string_value(),
                (XdmItem::Node(n), Some(XdmItem::Atomic(b))) => n.string_value() == as_string(b),
                (XdmItem::Atomic(a), Some(XdmItem::Node(n))) => as_string(a) == n.string_value(),
                _ => false,
            };
            if eq {
                out.push(XdmItem::Atomic(XdmAtomicValue::Integer(i as i64 + 1)));
            }
        }
        Ok(out)
    });

    // insert-before($seq, $pos, $item)
    add("insert-before", 3, |_ctx, args| {
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
    add("remove", 2, |_ctx, args| {
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

    // min/max
    add("min", 1, |ctx, args| minmax_impl(ctx, &args[0], None, true));
    add("min", 2, |ctx, args| {
        let uri = item_to_string(&args[1]);
        let coll = Some(resolve_named_collation_fn(ctx, &uri)?);
        minmax_impl(ctx, &args[0], coll.as_ref().map(|a| a.as_ref()), true)
    });
    add("max", 1, |ctx, args| minmax_impl(ctx, &args[0], None, false));
    add("max", 2, |ctx, args| {
        let uri = item_to_string(&args[1]);
        let coll = Some(resolve_named_collation_fn(ctx, &uri)?);
        minmax_impl(ctx, &args[0], coll.as_ref().map(|a| a.as_ref()), false)
    });

    // ===== Collation-related functions =====
    // compare($A, $B) => -1/0/1, empty if either is empty
    add("compare", 2, |ctx, args| {
        if args[0].is_empty() || args[1].is_empty() {
            return Ok(vec![]);
        }
        let a = item_to_string(&args[0]);
        let b = item_to_string(&args[1]);
        let coll = resolve_default_collation_fn(ctx);
        let ord = if let Some(c) = coll.as_deref() { c.compare(&a, &b) } else { a.cmp(&b) };
        let v = match ord { core::cmp::Ordering::Less => -1, core::cmp::Ordering::Equal => 0, core::cmp::Ordering::Greater => 1 };
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(v))])
    });
    // compare($A, $B, $collation)
    add("compare", 3, |ctx, args| {
        if args[0].is_empty() || args[1].is_empty() {
            return Ok(vec![]);
        }
        let a = item_to_string(&args[0]);
        let b = item_to_string(&args[1]);
        let uri = item_to_string(&args[2]);
        let coll = resolve_named_collation_fn(ctx, &uri)?;
        let ord = coll.compare(&a, &b);
        let v = match ord { core::cmp::Ordering::Less => -1, core::cmp::Ordering::Equal => 0, core::cmp::Ordering::Greater => 1 };
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(v))])
    });

    // codepoint-equal($A, $B) — empty if either is empty, uses codepoint collation only
    add("codepoint-equal", 2, |ctx, args| {
        if args[0].is_empty() || args[1].is_empty() {
            return Ok(vec![]);
        }
        let a = item_to_string(&args[0]);
        let b = item_to_string(&args[1]);
        // codepoint collation is always registered
        let coll = ctx
            .dyn_ctx
            .collations
            .get("http://www.w3.org/2005/xpath-functions/collation/codepoint")
            .expect("codepoint collation registered");
        let eq = coll.compare(&a, &b) == core::cmp::Ordering::Equal;
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(eq))])
    });

    // deep-equal($A as item()*, $B as item()*) as xs:boolean
    add("deep-equal", 2, |ctx, args| {
        let coll = resolve_default_collation_fn(ctx);
        let b = deep_equal_with_collation(&args[0], &args[1], coll.as_deref())?;
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))])
    });
    // deep-equal($A, $B, $collation as xs:string)
    add("deep-equal", 3, |ctx, args| {
        let uri = item_to_string(&args[2]);
        let coll = resolve_named_collation_fn(ctx, &uri)?;
        let b = deep_equal_with_collation(&args[0], &args[1], Some(coll.as_ref()))?;
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))])
    });

    // ===== Regex family =====
    // matches($input, $pattern)
    add("matches", 2, |ctx, args| {
        let input = item_to_string(&args[0]);
        let pattern = item_to_string(&args[1]);
        let flags = "";
        let b = regex_matches(ctx, &input, &pattern, flags)?;
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))])
    });
    // matches($input, $pattern, $flags)
    add("matches", 3, |ctx, args| {
        let input = item_to_string(&args[0]);
        let pattern = item_to_string(&args[1]);
        let flags = item_to_string(&args[2]);
        let b = regex_matches(ctx, &input, &pattern, &flags)?;
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))])
    });

    // replace($input, $pattern, $replacement)
    add("replace", 3, |ctx, args| {
        let input = item_to_string(&args[0]);
        let pattern = item_to_string(&args[1]);
        let repl = item_to_string(&args[2]);
        let flags = "";
        let s = regex_replace(ctx, &input, &pattern, &repl, flags)?;
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(s))])
    });
    // replace($input, $pattern, $replacement, $flags)
    add("replace", 4, |ctx, args| {
        let input = item_to_string(&args[0]);
        let pattern = item_to_string(&args[1]);
        let repl = item_to_string(&args[2]);
        let flags = item_to_string(&args[3]);
        let s = regex_replace(ctx, &input, &pattern, &repl, &flags)?;
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(s))])
    });

    // tokenize($input, $pattern)
    add("tokenize", 2, |ctx, args| {
        let input = item_to_string(&args[0]);
        let pattern = item_to_string(&args[1]);
        let flags = "";
        let parts = regex_tokenize(ctx, &input, &pattern, flags)?;
        Ok(parts
            .into_iter()
            .map(|s| XdmItem::Atomic(XdmAtomicValue::String(s)))
            .collect())
    });
    // tokenize($input, $pattern, $flags)
    add("tokenize", 3, |ctx, args| {
        let input = item_to_string(&args[0]);
        let pattern = item_to_string(&args[1]);
        let flags = item_to_string(&args[2]);
        let parts = regex_tokenize(ctx, &input, &pattern, &flags)?;
        Ok(parts
            .into_iter()
            .map(|s| XdmItem::Atomic(XdmAtomicValue::String(s)))
            .collect())
    });

    // ===== Date/Time family (M8 subset) =====
    add("current-dateTime", 0, |ctx, _args| {
        let dt = now_in_effective_tz(ctx);
        let s = dt.format("%Y-%m-%dT%H:%M:%S%:z").to_string();
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(s))])
    });
    add("current-date", 0, |ctx, _args| {
        let dt = now_in_effective_tz(ctx);
        let s = dt.format("%Y-%m-%d%:z").to_string();
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(s))])
    });
    add("current-time", 0, |ctx, _args| {
        let dt = now_in_effective_tz(ctx);
        let s = dt.format("%H:%M:%S%:z").to_string();
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(s))])
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
    }
}

fn to_number<N: crate::model::XdmNode>(seq: &XdmSequence<N>) -> Result<f64, Error> {
    if seq.is_empty() {
        return Ok(f64::NAN);
    }
    if seq.len() != 1 {
        return Err(Error::dynamic_err("err:FORG0006", "expects single item"));
    }
    match &seq[0] {
        XdmItem::Atomic(a) => to_number_atomic(a),
        XdmItem::Node(n) => n
            .string_value()
            .parse::<f64>()
            .map_err(|_| Error::dynamic_err("err:FORG0001", "invalid number")),
    }
}

fn to_number_atomic(a: &XdmAtomicValue) -> Result<f64, Error> {
    match a {
        XdmAtomicValue::Integer(i) => Ok(*i as f64),
        XdmAtomicValue::Double(d) => Ok(*d),
        XdmAtomicValue::Float(f) => Ok(*f as f64),
        XdmAtomicValue::Decimal(d) => Ok(*d),
        XdmAtomicValue::UntypedAtomic(s)
        | XdmAtomicValue::String(s)
        | XdmAtomicValue::AnyUri(s) => s
            .parse::<f64>()
            .map_err(|_| Error::dynamic_err("err:FORG0001", "invalid number")),
        XdmAtomicValue::Boolean(b) => Ok(if *b { 1.0 } else { 0.0 }),
        XdmAtomicValue::QName { .. } => Err(Error::dynamic_err(
            "err:XPTY0004",
            "cannot cast QName to number",
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

// ===== Helpers (M7 Collations) =====
fn resolve_default_collation_fn<N>(ctx: &CallCtx<N>) -> Option<std::sync::Arc<dyn crate::runtime::Collation>> {
    if let Some(c) = &ctx.default_collation {
        Some(c.clone())
    } else {
        ctx.dyn_ctx
            .collations
            .get("http://www.w3.org/2005/xpath-functions/collation/codepoint")
    }
}

fn resolve_named_collation_fn<N>(
    ctx: &CallCtx<N>,
    uri: &str,
) -> Result<std::sync::Arc<dyn crate::runtime::Collation>, Error> {
    if let Some(c) = ctx.dyn_ctx.collations.get(uri) {
        Ok(c)
    } else {
        Err(Error::dynamic_err("err:FOCH0002", format!("unknown collation URI: {}", uri)))
    }
}

fn contains_with_collation(
    s: &str,
    sub: &str,
    coll: Option<&dyn crate::runtime::Collation>,
) -> bool {
    if let Some(c) = coll {
        let ks = c.key(s);
        let ksub = c.key(sub);
        ks.contains(&ksub)
    } else {
        s.contains(sub)
    }
}

fn starts_with_with_collation(
    s: &str,
    sub: &str,
    coll: Option<&dyn crate::runtime::Collation>,
) -> bool {
    if let Some(c) = coll {
        let ks = c.key(s);
        let ksub = c.key(sub);
        ks.starts_with(&ksub)
    } else {
        s.starts_with(sub)
    }
}

fn ends_with_with_collation(
    s: &str,
    sub: &str,
    coll: Option<&dyn crate::runtime::Collation>,
) -> bool {
    if let Some(c) = coll {
        let ks = c.key(s);
        let ksub = c.key(sub);
        ks.ends_with(&ksub)
    } else {
        s.ends_with(sub)
    }
}

fn deep_equal_with_collation<N: crate::model::XdmNode>(
    a: &XdmSequence<N>,
    b: &XdmSequence<N>,
    coll: Option<&dyn crate::runtime::Collation>,
) -> Result<bool, Error> {
    if a.len() != b.len() {
        return Ok(false);
    }
    for (ia, ib) in a.iter().zip(b.iter()) {
        let eq = match (ia, ib) {
            (XdmItem::Atomic(aa), XdmItem::Atomic(bb)) => atomic_equal_with_collation(aa, bb, coll)?,
            (XdmItem::Node(na), XdmItem::Node(nb)) => {
                // Simplified node branch: compare string-values under collation
                let sa = na.string_value();
                let sb = nb.string_value();
                if let Some(c) = coll { c.compare(&sa, &sb) == core::cmp::Ordering::Equal } else { sa == sb }
            }
            _ => false,
        };
        if !eq { return Ok(false); }
    }
    Ok(true)
}

fn atomic_equal_with_collation(
    a: &XdmAtomicValue,
    b: &XdmAtomicValue,
    coll: Option<&dyn crate::runtime::Collation>,
) -> Result<bool, Error> {
    use XdmAtomicValue::*;
    // Prefer numeric equality if both sides are numeric primitives
    let is_num = |v: &XdmAtomicValue| matches!(v, Integer(_) | Double(_) | Float(_) | Decimal(_));
    if is_num(a) && is_num(b) {
        let na = to_number_atomic(a)?;
        let nb = to_number_atomic(b)?;
        return Ok(na == nb);
    }
    // Otherwise, compare by string using collation
    let sa = as_string(a);
    let sb = as_string(b);
    if let Some(c) = coll {
        Ok(c.compare(&sa, &sb) == core::cmp::Ordering::Equal)
    } else {
        Ok(sa == sb)
    }
}

// ===== Helpers (M7 Regex) =====
fn get_regex_provider<N>(
    ctx: &CallCtx<N>,
) -> std::sync::Arc<dyn crate::runtime::RegexProvider> {
    if let Some(p) = &ctx.regex {
        p.clone()
    } else {
        std::sync::Arc::new(crate::runtime::RustRegexProvider)
    }
}

fn regex_matches<N>(
    ctx: &CallCtx<N>,
    input: &str,
    pattern: &str,
    flags: &str,
) -> Result<bool, Error> {
    let provider = get_regex_provider(ctx);
    provider.matches(pattern, flags, input)
}

fn regex_replace<N>(
    ctx: &CallCtx<N>,
    input: &str,
    pattern: &str,
    repl: &str,
    flags: &str,
) -> Result<String, Error> {
    let provider = get_regex_provider(ctx);
    provider.replace(pattern, flags, input, repl)
}

fn regex_tokenize<N>(
    ctx: &CallCtx<N>,
    input: &str,
    pattern: &str,
    flags: &str,
) -> Result<Vec<String>, Error> {
    let provider = get_regex_provider(ctx);
    provider.tokenize(pattern, flags, input)
}

fn minmax_impl<N: crate::model::XdmNode>(
    ctx: &CallCtx<N>,
    seq: &XdmSequence<N>,
    coll: Option<&dyn crate::runtime::Collation>,
    is_min: bool,
) -> Result<XdmSequence<N>, Error> {
    if seq.is_empty() {
        return Ok(vec![]);
    }
    // numeric if all numeric, else string using collation (default or provided)
    let mut all_num = true;
    let mut acc_num = if is_min { f64::INFINITY } else { f64::NEG_INFINITY };
    for it in seq {
        match it {
            XdmItem::Atomic(a) => match to_number_atomic(a) {
                Ok(n) => {
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
        return Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(acc_num))]);
    }
    // String branch
    let default_coll = resolve_default_collation_fn(ctx);
    let effective_coll: Option<&dyn crate::runtime::Collation> = if let Some(c) = coll {
        Some(c)
    } else {
        default_coll.as_deref()
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
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(best_orig))])
    } else {
        let best = iter.fold(first, |acc, it| {
            let s = match it {
                XdmItem::Atomic(a) => as_string(a),
                XdmItem::Node(n) => n.string_value(),
            };
            let ord = s.cmp(&acc);
            if is_min {
                if ord == core::cmp::Ordering::Less { s } else { acc }
            } else {
                if ord == core::cmp::Ordering::Greater { s } else { acc }
            }
        });
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(best))])
    }
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
